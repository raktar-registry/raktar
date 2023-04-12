mod api;
pub mod app_state;
pub mod db;
pub mod metadata;
pub mod storage;

use crate::api::index::{
    get_info_for_long_name_crate, get_info_for_short_name_crate, get_info_for_three_letter_crate,
};
use crate::api::publish::publish_crate;
use crate::app_state::AppState;
use crate::storage::S3Storage;
use aws_sdk_dynamodb::Client;
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use lambda_web::run_hyper_on_lambda;
use serde::Serialize;

#[derive(Serialize)]
struct Config {
    dl: String,
    api: String,
}

async fn get_config_json() -> (StatusCode, Json<Config>) {
    let response = Config {
        dl: "https://23g9zd8v1b.execute-api.eu-west-1.amazonaws.com/api/v1/crates".to_string(),
        api: "https://23g9zd8v1b.execute-api.eu-west-1.amazonaws.com".to_string(),
    };

    (StatusCode::OK, Json(response))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().json().init();

    let aws_config = aws_config::from_env().load().await;
    let db_client = Client::new(&aws_config);
    let storage = S3Storage::new().await;
    let app_state = AppState { db_client, storage };

    let app = Router::new()
        .route("/config.json", get(get_config_json))
        .route("/api/v1/crates/new", put(publish_crate))
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
        .with_state(app_state);

    run_app(app).await
}

#[cfg(feature = "local")]
async fn run_app(app: Router) {
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3025));
    tracing::info!("listening on http://{}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("service to start successfully");
}

#[cfg(not(feature = "local"))]
async fn run_app(app: Router) {
    run_hyper_on_lambda(app)
        .await
        .expect("app to run on Lambda successfully")
}
