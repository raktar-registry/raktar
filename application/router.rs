use crate::auth::token_authenticator;
use crate::cargo_api::config::get_config_json;
use crate::cargo_api::download::download_crate;
use crate::cargo_api::index::{
    get_info_for_long_name_crate, get_info_for_short_name_crate, get_info_for_three_letter_crate,
};
use crate::cargo_api::me::redirect_for_token;
use crate::cargo_api::owners::{add_owners, list_owners};
use crate::cargo_api::publish::publish_crate_handler;
use crate::cargo_api::unyank::unyank;
use crate::cargo_api::yank::yank;
use crate::graphql::handler::{graphiql, graphql_handler};
use crate::graphql::schema::build_schema;
use crate::repository::DynRepository;
use crate::storage::DynCrateStorage;
use axum::routing::{delete, get, put, Router};
use axum::Extension;

pub type AppState = (DynRepository, DynCrateStorage);

pub fn build_router(repository: DynRepository, storage: DynCrateStorage) -> Router {
    let core_router = build_core_router(repository.clone());
    let graphql_router = build_graphql_router(repository.clone());
    let state = (repository, storage);

    Router::new()
        .route("/config.json", get(get_config_json))
        .route("/me", get(redirect_for_token))
        .nest("/", core_router)
        .nest("/gql", graphql_router)
        .with_state(state)
}

fn build_core_router(repository: DynRepository) -> Router<AppState> {
    Router::new()
        .route("/api/v1/crates/new", put(publish_crate_handler))
        .route(
            "/api/v1/crates/:crate_name/owners",
            get(list_owners).put(add_owners),
        )
        .route("/api/v1/crates/:crate_name/:version/yank", delete(yank))
        .route("/api/v1/crates/:crate_name/:version/unyank", put(unyank))
        .route(
            "/api/v1/crates/:crate_name/:version/download",
            get(download_crate),
        )
        .route("/1/:crate_name", get(get_info_for_short_name_crate))
        .route("/2/:crate_name", get(get_info_for_short_name_crate))
        .route(
            "/3/:first_letter/:crate_name",
            get(get_info_for_three_letter_crate),
        )
        .route(
            "/:first_two/:second_two/:crate_name",
            get(get_info_for_long_name_crate),
        )
        .layer(axum::middleware::from_fn_with_state(
            repository,
            token_authenticator,
        ))
}

fn build_graphql_router(repository: DynRepository) -> Router<AppState> {
    let schema = build_schema(repository);
    Router::new()
        .route("/", get(graphiql).post(graphql_handler))
        .layer(Extension(schema))
}
