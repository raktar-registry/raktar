use std::str::FromStr;

use axum::extract::{Path, State};
use axum::Json;
use semver::Version;
use serde::Serialize;

use crate::error::AppResult;
use crate::router::AppState;

#[derive(Serialize)]
pub struct Response {
    ok: bool,
}

pub async fn unyank(
    Path((crate_name, version)): Path<(String, String)>,
    State((repository, _)): State<AppState>,
) -> AppResult<Json<Response>> {
    let vers = Version::from_str(&version).expect("version to be valid");
    repository.set_yanked(&crate_name, &vers, false).await?;

    let response = Json(Response { ok: true });
    Ok(response)
}
