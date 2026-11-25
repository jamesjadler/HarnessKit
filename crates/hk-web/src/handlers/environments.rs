use crate::{
    router::{ApiError, blocking},
    state::WebState,
};
use axum::{Json, extract::State};
use hk_core::environments::{self, TextSnapshot};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct ReadText {
    pub path: String,
    pub environment: Option<String>,
    pub provider: Option<String>,
}
#[derive(Deserialize)]
pub struct SaveText {
    pub path: String,
    pub expected: String,
    pub content: String,
    pub environment: Option<String>,
    pub provider: Option<String>,
}
pub async fn read_comparison_file(
    State(state): State<WebState>,
    Json(req): Json<ReadText>,
) -> Result<Json<TextSnapshot>, ApiError> {
    blocking(move || {
        if let Some(env) = req.environment {
            return environments::environment_text(
                &state.home,
                &env,
                req.provider.as_deref().unwrap_or(""),
                &req.path,
                None,
            );
        }
        let roots = environments::local_roots(&state.store.lock(), &state.home)?;
        environments::read_text(req.path.as_ref(), &roots)
    })
    .await
}
pub async fn save_comparison_file(
    State(state): State<WebState>,
    Json(req): Json<SaveText>,
) -> Result<Json<TextSnapshot>, ApiError> {
    blocking(move || {
        let store = hk_core::store::Store::open(&state.home.join(".harnesskit/metadata.db"))?;
        if let Some(snapshot) = environments::managed_files::comparison_save(
            &state.home,
            &state.adapters,
            &store,
            req.environment.as_deref(),
            &req.path,
            &req.expected,
            &req.content,
        )? {
            return Ok(snapshot);
        }
        if let Some(env) = req.environment {
            return environments::environment_text(
                &state.home,
                &env,
                req.provider.as_deref().unwrap_or(""),
                &req.path,
                Some((&req.expected, &req.content)),
            );
        }
        let roots = environments::local_roots(&state.store.lock(), &state.home)?;
        environments::save_text(
            req.path.as_ref(),
            &req.expected,
            &req.content,
            &roots,
            &state.home.join(".harnesskit/comparison-history"),
        )
    })
    .await
}

pub async fn environment_command(
    State(state): State<WebState>,
    Json(body): Json<super::ReqBody<hk_core::environments::service::Request>>,
) -> Result<Json<hk_core::environments::registry::Overview>, ApiError> {
    blocking(move || {
        let store = hk_core::store::Store::open(&state.home.join(".harnesskit/metadata.db"))?;
        hk_core::environments::service::execute(&state.home, &state.adapters, &store, body.req)
    })
    .await
}

pub async fn discover_environments()
-> Result<Json<Vec<hk_core::environments::transport::Candidate>>, ApiError> {
    blocking(move || Ok(hk_core::environments::transport::discover())).await
}
pub async fn environment_extension(
    State(state): State<WebState>,
    Json(req): Json<hk_core::environments::inventory::ActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    blocking(move || hk_core::environments::inventory::action(&state.home, req)).await
}

pub async fn managed_file_command(
    State(state): State<WebState>,
    Json(body): Json<super::ReqBody<hk_core::environments::managed_files::Request>>,
) -> Result<Json<hk_core::environments::managed_files::Response>, ApiError> {
    blocking(move || {
        let store = hk_core::store::Store::open(&state.home.join(".harnesskit/metadata.db"))?;
        hk_core::environments::managed_files::execute(
            &state.home,
            &state.adapters,
            &store,
            body.req,
        )
    })
    .await
}
