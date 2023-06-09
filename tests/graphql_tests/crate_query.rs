use async_graphql::{value, Request, Variables};
use axum::body::Bytes;
use raktar::auth::AuthenticatedUser;
use raktar::cargo_api::publish::publish_crate;
use raktar::graphql::schema::{build_schema, RaktarSchema};
use raktar::repository::DynRepository;
use raktar::storage::DynCrateStorage;
use serde::Deserialize;
use std::sync::Arc;

use crate::common::graphql::build_request;
use crate::common::memory_storage::MemoryStorage;
use crate::common::setup::build_repository;

#[tokio::test]
async fn test_crate_query_with_head_version_works() {
    let repository = Arc::new(build_repository().await) as DynRepository;
    let schema = build_schema(repository.clone());
    let storage = Arc::new(MemoryStorage::default()) as DynCrateStorage;
    let user = AuthenticatedUser { id: 1 };

    // publish version 0.1.1
    let data = Bytes::from_static(CRATE_BYTES_V1);
    publish_crate(user.clone(), storage.clone(), repository.clone(), data)
        .await
        .expect("publish to succeed");

    // head state is 0.1.1, assert that the query works and reflects this
    let crate_version = get_crate_version(&schema, "testcrate_1").await;
    assert_eq!(crate_version.version, "0.1.1");

    // publish version 0.1.2
    let data = Bytes::from_static(CRATE_BYTES_V2);
    publish_crate(user.clone(), storage.clone(), repository.clone(), data)
        .await
        .expect("publish to succeed");

    // the query should now return 0.1.2
    let crate_version = get_crate_version(&schema, "testcrate_1").await;
    assert_eq!(crate_version.version, "0.1.2");
    assert_eq!(crate_version.krate.versions, vec!["0.1.1", "0.1.2"])
}

#[tokio::test]
async fn test_crate_query_returns_null_when_crate_is_missing() {
    let repository = Arc::new(build_repository().await) as DynRepository;
    let schema = build_schema(repository.clone());

    let request = build_crate_request(1, "missing_crate", None);
    let response = schema.execute(request).await;

    assert_eq!(response.errors.len(), 0);
    let data = response.data.into_json().unwrap();
    let crate_version = data.as_object().unwrap().get("crateVersion").unwrap();

    assert!(crate_version.as_null().is_some());
}

#[tokio::test]
async fn test_crate_query_returns_null_when_crate_version_is_missing() {
    let repository = Arc::new(build_repository().await) as DynRepository;
    let schema = build_schema(repository.clone());
    let storage = Arc::new(MemoryStorage::default()) as DynCrateStorage;
    let user = AuthenticatedUser { id: 1 };

    // publish version 0.1.1
    let data = Bytes::from_static(CRATE_BYTES_V1);
    publish_crate(user.clone(), storage.clone(), repository.clone(), data)
        .await
        .expect("publish to succeed");

    // head state is 0.1.1, assert that the query works and reflects this
    let crate_version = get_crate_version(&schema, "testcrate_1").await;
    assert_eq!(crate_version.version, "0.1.1");

    // version 0.2.0 does not exist, so the query should return null
    let request = build_crate_request(1, "testcrate_1", Some("0.2.0"));
    let response = schema.execute(request).await;

    assert_eq!(response.errors.len(), 0);
    let data = response.data.into_json().unwrap();
    let crate_version = data.as_object().unwrap().get("crateVersion").unwrap();

    assert!(crate_version.as_null().is_some());
}

async fn get_crate_version(schema: &RaktarSchema, name: &str) -> CrateVersion {
    let request = build_crate_request(1, name, None);
    let response = schema.execute(request).await;

    assert_eq!(response.errors.len(), 0);
    let data = response.data.into_json().unwrap();
    let parsed_response: CrateVersionResponse = serde_json::from_value(data).unwrap();

    parsed_response.crate_version
}

#[derive(Debug, Deserialize)]
struct CrateVersionResponse {
    #[serde(rename = "crateVersion")]
    crate_version: CrateVersion,
}

#[derive(Debug, Deserialize)]
struct CrateVersion {
    version: String,
    #[serde(rename = "crate")]
    krate: Crate,
}

#[derive(Debug, Deserialize)]
struct Crate {
    versions: Vec<String>,
}

fn build_crate_request(user_id: u32, name: &str, version: Option<&str>) -> Request {
    let query = r#"
    query CrateVersion($name: String!, $version: String) {
      crateVersion(name: $name, version: $version) {
        id
        name
        description
        version
        readme
        repository
        crate {
          versions
        }
      }
    }
    "#;

    let variables = match version {
        None => Variables::from_value(value!({ "name": name })),
        Some(v) => Variables::from_value(value!({ "name": name, "version": v })),
    };

    build_request(query, user_id).variables(variables)
}

static CRATE_BYTES_V1: &[u8; 1374] = b":\x02\0\0{\"name\":\"testcrate_1\",\"vers\":\"0.1.1\",\"deps\":[{\"optional\":false,\"default_features\":true,\"name\":\"serde\",\"features\":[\"derive\"],\"version_req\":\"^1.0.150\",\"target\":null,\"kind\":\"normal\",\"registry\":\"https://github.com/rust-lang/crates.io-index\"}],\"features\":{},\"authors\":[],\"description\":\"A private crate for testing purposes.\",\"documentation\":null,\"homepage\":null,\"readme\":\"# Test Crate 1\\n\\nA crate for testing Raktar.\\n\",\"readme_file\":\"README.md\",\"keywords\":[],\"categories\":[],\"license\":null,\"license_file\":null,\"repository\":null,\"badges\":{},\"links\":null,\"rust_version\":null}\x1c\x03\0\0\x1f\x8b\x08\x08\0\0\0\0\x02\xfftestcrate_1-0.1.1.crate\0\xedX\xdfo\xda0\x10\xe6\xd9\x7f\xc5)}i%\x9a&\xfc\x94:\xf5!\x03\xb6!\xb5\xabD\x99\xa6\xaab\xabILb\xe1\xc4\xc8v\xcaX\xd5\xff}\x97@K\xa1h}\x18EC\xcd\xf7\x12\xc7\xb1\xef\xce\x97\xfb>;1L\x1b_Q\xc3~\xba\xc7\x8e\xed\xda\xeeI\x8b\xaaP\xdaF\xc6\xa2\xb4%8\x88F\xad\xb6\xb1\x1f\xe1Vj\xeec;\xbb\xc5\xb6\x8b}\xf5\x92S\xda\x01Rm\xa8\x02(\xbdS\x1c@\xffK\xf7\n>u\xcf;\x80W\xef[\xff\xf2\xc2\xebw[\xde\xf9\xf95|\xee|\xed\xf4\xbc~\xa7\r\x1f\xaf\xa1\xe5\xf5>_\x92\x03r\0\xdf#\x96@:\x11\x92\x06<\t!/\x1f\rF\x82\x89\x18(\x16rm\xd4\x0c\xf2:\x82)\x17\x02h\x8a\xe5D\r\xf7\xa9\x1034`%R\xc5T\xf0\xdf\xcc\x82e\xb9\xc1\x88\x0b\xb43\x92\nb\xfa\x8b\xe3\0\xf0e<\xc1yC.\xb8\xc9&N\xb9\x89\0\x8d\xc0\x1dS\x9a\xcbD\x83\x1c-\x1c\xd1$\xc0'Zb\0S\xc5\r\x83[\x9c\x19\xddB\xc0&,\tX\xe2s\xa6\xd1\x82\x91\xcb\x08\x0f\x99\x1d\xda\xe5E\xfc6\x97G+\x83\xed|\xad\xdd\x11\xccd\nTe+\x9b\xaf\xd7D\\\xe7\xb1\xc2\x90\x01\x9df\x8fLDM\xbez\xa9x\xc8\x13\x8c|\xb9\xac<l\x0cY\xf01\x133\x10R\x8e\xb3\xf0g\x10\xf0\xd1\x88)\x96\x188\xcc\x82\x8fS?\x82X\xce\x1di\x99\xd0\xa1`G\x18\x04\\1\xf6\xcc\x9c\x9d\xb9\xc8\x93\xb4\xe2\xcf\x97\x89AS\x185\xb9\x99P\x7fLC6 ,\xe0\x06\xb3\x04g`U\x9c\x8ak\x91\x84\xc6,\xbb3K\xd6[d\x91\xcb\xac?W\0\x8b\x04L\xfb\x8aO\x1e\xe7z0Q\xfc\x0eG\xcfS5w\x8e\x16\xb2dLR5\x91\x1a\xb3e\x91,?s\xf3\xbd\x8e\xd7\xbe\xe8\xd8q`a4+9\xd5L\x05\x18\xd83\x97?\\\x1b\xbd\xd6\x1d\x8b\x8c\x185\xa9\xc2\n8\x83\x1b+`\xe8\x92Y\x03R*\xf0\x960\x7f\xd1\xff\xbc\xd4\xb6\xa8\xff\x0b\x81_\xbf:\xd5J\xa3\xe4:\xb5\x86\xeb`\x95V\xb3~\xb7\xd6\xac\xba\xfb\xa5\xff\xeb\x8b\xdb\x13,\xc5\xe2M\xb5\xe1\x85\x12m\x16\x8bU\xb5\x18\x90\\.p\xd0=lR\x8c2l\x94\x0cx(D\xe3\x1f\xf8\xff\xf4>\xb6\xe6\xe35\xfe;\xf5\xda:\xff\xabU\xb7Y\xf0\x7f7\xe7?\xac\0h\xe5\xe4u\t\xf16\xf0\xb8G\xc7\x98#\xbb`\xd5\xbb\xe0\xbfBF\x1c\x1b)\x85\x1fQ\x9el\xe3C\xf0U\xfeW\x9b\xeb\xfc\xaf7\xddb\xff\xdf\xc9\xfe\xff\xf4\xa6\x07\x04/I\xc2D\xb6\xc7&<\x8c\x8c\x98Y\x05A\xde\x1d\xff\xb5\xf2O\x04\x1f\xdaJ\xefj\xffw\x1b/\xf7\xff\xe6\xbe\xfd\xff\xd9S\xfe\xa7\x9aA~\xca>=\xbdo3l\xf1\xfc\xbfL\x19\xae\x1e\x9b\x0f\x1f\x089\xb8\x99\x1f\xae\x0f\xdbl\x98\x86e\xd8<\xf2\x08\x0f\xecF\xa5\xbe\xc9\xcf\x14mj(\xdc\x13@\x04\xd8<\x85Tg\xc3Iq:/P\xa0@\x81\xff\x01\x7f\0\xc4\xbd\n+\0\x1a\0\0";
static CRATE_BYTES_V2: &[u8; 1374] = b":\x02\0\0{\"name\":\"testcrate_1\",\"vers\":\"0.1.2\",\"deps\":[{\"optional\":false,\"default_features\":true,\"name\":\"serde\",\"features\":[\"derive\"],\"version_req\":\"^1.0.150\",\"target\":null,\"kind\":\"normal\",\"registry\":\"https://github.com/rust-lang/crates.io-index\"}],\"features\":{},\"authors\":[],\"description\":\"A private crate for testing purposes.\",\"documentation\":null,\"homepage\":null,\"readme\":\"# Test Crate 1\\n\\nA crate for testing Raktar.\\n\",\"readme_file\":\"README.md\",\"keywords\":[],\"categories\":[],\"license\":null,\"license_file\":null,\"repository\":null,\"badges\":{},\"links\":null,\"rust_version\":null}\x1c\x03\0\0\x1f\x8b\x08\x08\0\0\0\0\x02\xfftestcrate_1-0.1.2.crate\0\xedXQo\xda0\x10\xe6\xd9\xbf\xe2\x14^Z\x89\xa6\tP\x90:\xf5!\x03\xb6!\xb5\xabD\x99\xa6\xaab\xabILb\xe1\xc4\xc8v\xcaX\xd5\xff\xbeK\xa0\xa5P\xb4>\x8c\xa2\xa1\xe6{\x89\xe3\xd8w\xe7\xcb}\x9f\x9d\x18\xa6\x8d\xaf\xa8a?\xdd#\xc7v\xed\xeaq\x8b\xaaP\xdaF\xc6\xa2\xb4%8\x88F\xbd\xbe\xb1\x1f\xe1V\xeb\xeec;\xbb\xc5\xb6\x8b}\x8d\x92S\xda\x01Rm\xa8\x02(\xbdS\x94\xa1\xff\xa5{\x05\x9f\xba\xe7\x1d\xc0\xab\xf7\xad\x7fy\xe1\xf5\xbb-\xef\xfc\xfc\x1a>w\xbevz^\xbf\xd3\x86\x8f\xd7\xd0\xf2z\x9f/I\x99\x94\xe1{\xc4\x12H'B\xd2\x80'!\xe4\xe5\xa3\xc1H0\x11\x03\xc5B\xae\x8d\x9aA^G0\xe5B\0M\xb1\x9c\xa8\xe1>\x15b\x86\x06\xacD\xaa\x98\n\xfe\x9bY\xb0,7\x18q\x81vFRAL\x7fq\x1c\0\xbe\x8c'8o\xc8\x057\xd9\xc4)7\x11\xa0\x11\xb8cJs\x99h\x90\xa3\x85#\x9a\x04\xf8DK\x0c`\xaa\xb8ap\x8b3\xa3[\x08\xd8\x84%\x01K|\xce4Z0r\x19\xe1\x01\xb3C\xbb\xb2\x88\xdf\xe6\xf2pe\xb0\x9d\xaf\xb5;\x82\x99L\x81\xaale\xf3\xf5\x9a\x88\xeb<V\x182\xa0\xd3\xec\x91\x89\xa8\xc9W/\x15\x0fy\x82\x91/\x97\x95\x87\x8d!\x0b>fb\x06B\xcaq\x16\xfe\x0c\x02>\x1a1\xc5\x12\x03\x07Y\xf0q\xeaG\x10\xcb\xb9#-\x13:\x14\xec\x10\x83\x80+\xc6\x9e\x99\xb33\x17y\x92V\xfc\xf921h\n\xa3&7\x13\xea\x8fi\xc8\x06\x84\x05\xdc`\x96\xe0\x0c\xac\xaaSu-\x92\xd0\x98ewf\xc9z\x8b,r\x99\xf5\xe7\n`\x91\x80i_\xf1\xc9\xe3\\\x0f&\x8a\xdf\xe1\xe8y\xaa\xe6\xce\xd1B\x96\x8cI\xaa&Rc\xb6,\x92\xe5gn\xbe\xd7\xf1\xda\x17\x1d;\x0e,\x8cf%\xa7\x9a\xa9\0\x03{\xe6\xf2\x87k\xa3\xd7\x13\xc7\"#FM\xaa\xb0\x02\xce\xe0\xc6\n\x18\xbad\xd6\x80\x94\n\xbc%\xcc_\xf4?/\xb5-\xea\xffB\xe0\xd7\xafN\xad\xda(\xb9N\xbd\xe1:X\xa5\xb5\xac\xdf\xad7k\xd5\xfd\xd2\xff\xf5\xc5\xed\t\x96b\xf1\xa6\xda\xf0B\x896\x8b\xc5\xaaZ\x0cH.\x178\xe8\x1e6)F\x056J\x06<\x14\xa2\xf1\x0f\xfc\x7fz\x1f[\xf3\xf1\x1a\xff\x9d\x93\xfa:\xffk\xb5\xaaS\xf0\x7f7\xe7?\xac\0h\xe5\xe4u\t\xf16\xf0\xb8G\xc7\x98#\xbb`\xd5\xbb\xe0\xbfBF\x1c\x19)\x85\x1fQ\x9el\xe3C\xf0U\xfe\xd7\x9a\xeb\xfc?i\xba\xc5\xfe\xbf\x93\xfd\xff\xe9M\x0f\x08^\x92\x84\x89l\x8fMx\x18\x191\xb3\n\x82\xbc;\xfek\xe5\x1f\x0b>\xb4\x95\xde\xd5\xfe\xef6^\xee\xff\xcd}\xfb\xff\xb3\xa7\xfcO5\x83\xfc\x94}zz\xdff\xd8\xe2\xf9\x7f\x99\n\\=6\x1f>\x10R\xbe\x99\x1f\xae\x0f\xdal\x98\x86\x15\xd8<\xf2\x10\x0f\xecF\xa5\xbe\xc9\xcf\x14mj(\xdc\x13@\x04\xd8<\x85Tg\xc3Iq:/P\xa0@\x81\xff\x01\x7f\0\xe6\x93\r)\0\x1a\0\0";
