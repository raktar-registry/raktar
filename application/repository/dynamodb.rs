use anyhow::anyhow;
use aws_sdk_dynamodb::operation::put_item::PutItemError;
use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
use aws_sdk_dynamodb::operation::update_item::UpdateItemError;
use aws_sdk_dynamodb::types::{AttributeValue, Put, ReturnValue, TransactWriteItem};
use aws_sdk_dynamodb::Client;
use semver::Version;
use serde_dynamo::aws_sdk_dynamodb_0_25::{from_items, to_item};
use serde_dynamo::from_item;
use std::collections::HashMap;
use thiserror::__private::AsDynError;
use tracing::{error, info};

use crate::error::{internal_error, AppError, AppResult};
use crate::models::crate_details::CrateDetails;
use crate::models::index::PackageInfo;
use crate::models::user::User;
use crate::repository::Repository;

static CRATES_PARTITION_KEY: &str = "CRATES";

#[derive(Clone)]
pub struct DynamoDBRepository {
    db_client: Client,
    table_name: String,
}

impl DynamoDBRepository {
    pub(crate) fn new(db_client: Client) -> Self {
        Self {
            db_client,
            table_name: std::env::var("TABLE_NAME").unwrap(),
        }
    }

    fn get_package_key(crate_name: &str) -> AttributeValue {
        AttributeValue::S(format!("CRT#{}", crate_name))
    }

    fn get_package_version_key(version: &Version) -> AttributeValue {
        AttributeValue::S(format!("V#{}", version))
    }

    fn get_crate_info_key(&self, crate_name: String) -> Option<HashMap<String, AttributeValue>> {
        let mut key = HashMap::new();
        key.insert(
            "pk".to_string(),
            AttributeValue::S(CRATES_PARTITION_KEY.to_string()),
        );
        key.insert("sk".to_string(), AttributeValue::S(crate_name));
        Some(key)
    }

    async fn get_crate_details(&self, crate_name: &str) -> AppResult<Option<CrateDetails>> {
        let res = self
            .db_client
            .get_item()
            .table_name(&self.table_name)
            .set_key(self.get_crate_info_key(crate_name.to_string()))
            .send()
            .await
            .map_err(|e| {
                let err = e.as_dyn_error();
                error!(err, crate_name, "unexpected error in getting crate details");
                internal_error()
            })?;

        let details = if let Some(item) = res.item {
            let crate_info: CrateDetails = from_item(item).map_err(|_| {
                error!(crate_name, "failed to parse crate info");
                internal_error()
            })?;

            Some(crate_info)
        } else {
            None
        };

        Ok(details)
    }

    async fn put_new_package(
        &self,
        crate_name: &str,
        version: &Version,
        package_info: PackageInfo,
    ) -> AppResult<()> {
        let details = CrateDetails {
            // TODO: this should be the user's ID once auth is in place
            owners: vec![0],
        };
        let item = to_item(details).unwrap();
        let put_item = Put::builder()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .item("pk", AttributeValue::S(CRATES_PARTITION_KEY.to_string()))
            .item("sk", AttributeValue::S(crate_name.to_string()))
            .condition_expression("attribute_not_exists(sk)")
            .build();
        let put_details_item = TransactWriteItem::builder().put(put_item).build();

        // TODO: fix unwrap
        let pk = DynamoDBRepository::get_package_key(&package_info.name);
        let sk = DynamoDBRepository::get_package_version_key(&package_info.vers);
        let item = to_item(package_info).unwrap();
        let put = Put::builder()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .item("pk", pk)
            .item("sk", sk)
            .build();
        let put_item = TransactWriteItem::builder().put(put).build();

        match self
            .db_client
            .transact_write_items()
            .transact_items(put_details_item)
            .transact_items(put_item)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    crate_name = crate_name,
                    version = version.to_string(),
                    "persisted package info"
                );
                Ok(())
            }
            Err(e) => Err(match e.into_service_error() {
                TransactWriteItemsError::TransactionCanceledException(_) => {
                    // TODO: how should we handle this? retry? fail?
                    anyhow::anyhow!("write conflict on new crate").into()
                }
                _ => anyhow::anyhow!("unexpected error in persisting crate").into(),
            }),
        }
    }

    async fn put_new_package_version(
        &self,
        crate_name: &str,
        version: &Version,
        package_info: PackageInfo,
    ) -> AppResult<()> {
        let pk = DynamoDBRepository::get_package_key(&package_info.name);
        let sk = DynamoDBRepository::get_package_version_key(&package_info.vers);

        let item = to_item(package_info).unwrap();
        match self
            .db_client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .item("pk", pk)
            .item("sk", sk)
            .condition_expression("attribute_not_exists(sk)")
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    crate_name = crate_name,
                    version = version.to_string(),
                    "persisted package info"
                );
                Ok(())
            }
            Err(err) => {
                let err = match err.into_service_error() {
                    PutItemError::ConditionalCheckFailedException(_) => {
                        AppError::DuplicateCrateVersion {
                            crate_name: crate_name.to_string(),
                            version: version.clone(),
                        }
                    }
                    _ => {
                        error!("failed to store package info");
                        anyhow::anyhow!("unexpected error in persisting crate").into()
                    }
                };

                Err(err)
            }
        }
    }
}

#[async_trait::async_trait]
impl Repository for DynamoDBRepository {
    async fn get_package_info(&self, crate_name: &str) -> AppResult<String> {
        let result = self
            .db_client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("pk = :pk and begins_with(sk, :prefix)")
            .expression_attribute_values(":pk", DynamoDBRepository::get_package_key(crate_name))
            .expression_attribute_values(":prefix", AttributeValue::S("V#".to_string()))
            .send()
            .await
            .map_err(|err| {
                let error = format!("{:?}", err.into_service_error());
                error!(error, crate_name, "failed to query package info");
                anyhow!("internal server error")
            })?;

        match result.items() {
            None => Err(AppError::NonExistentPackageInfo(crate_name.to_string())),
            Some(items) => {
                let infos = from_items::<PackageInfo>(items.to_vec()).map_err(|_| {
                    error!(
                        crate_name,
                        "failed to parse DynamoDB package info items for crate"
                    );
                    anyhow!("internal server error")
                })?;
                Ok(infos
                    .into_iter()
                    .map(|info| serde_json::to_string(&info).unwrap())
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
        }
    }

    async fn store_package_info(
        &self,
        crate_name: &str,
        version: &Version,
        package_info: PackageInfo,
    ) -> AppResult<()> {
        match self.get_crate_details(crate_name).await? {
            // the crate does not exist yet, write the crate details and the new version at once
            None => {
                self.put_new_package(crate_name, version, package_info)
                    .await
            }
            // crate details already exist, write the new version
            Some(_) => {
                self.put_new_package_version(crate_name, version, package_info)
                    .await
            }
        }
    }

    async fn set_yanked(&self, crate_name: &str, version: &Version, yanked: bool) -> AppResult<()> {
        let pk = DynamoDBRepository::get_package_key(crate_name);
        let sk = DynamoDBRepository::get_package_version_key(version);

        self.db_client
            .update_item()
            .table_name(&self.table_name)
            .key("pk", pk)
            .key("sk", sk)
            .update_expression("SET yanked = :y")
            .condition_expression("attribute_exists(sk)")
            .expression_attribute_values(":y", AttributeValue::Bool(yanked))
            .send()
            .await
            .map_err(|err| match err.into_service_error() {
                UpdateItemError::ConditionalCheckFailedException(_) => {
                    AppError::NonExistentCrateVersion {
                        crate_name: crate_name.to_string(),
                        version: version.clone(),
                    }
                }
                _ => {
                    // TODO: add more information for the failure
                    error!("failed to yank package");
                    anyhow!("internal server error").into()
                }
            })?;

        Ok(())
    }

    async fn list_owners(&self, crate_name: &str) -> AppResult<Vec<User>> {
        match self.get_crate_details(crate_name).await? {
            None => Err(AppError::NonExistentPackageInfo(crate_name.to_string())),
            Some(crate_details) => {
                let users = crate_details
                    .owners
                    .into_iter()
                    .map(|id| User {
                        // TODO: map login properly
                        id,
                        login: "dummy".to_string(),
                        name: None,
                    })
                    .collect();
                Ok(users)
            }
        }
    }

    async fn add_owners(&self, crate_name: &str, user_ids: Vec<String>) -> AppResult<()> {
        match self
            .db_client
            .update_item()
            .table_name(&self.table_name)
            .set_key(self.get_crate_info_key(crate_name.to_string()))
            .update_expression("ADD #owners = :new_owners")
            .expression_attribute_names("#owners", "owners".to_string())
            .expression_attribute_values(":new_owners", AttributeValue::Ss(user_ids))
            .return_values(ReturnValue::UpdatedOld)
            .send()
            .await
        {
            Ok(_) => Ok(()),
            Err(_err) => Err(anyhow::anyhow!("internal server error").into()),
        }
    }
}