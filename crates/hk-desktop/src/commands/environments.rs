use super::AppState;
use hk_core::{
    HkError,
    environments::{self, TextSnapshot},
};
use tauri::State;
#[tauri::command]
pub fn read_comparison_file(
    state: State<AppState>,
    path: String,
    environment: Option<String>,
    provider: Option<String>,
) -> Result<TextSnapshot, HkError> {
    let home = hk_core::home_dir();
    if let Some(env) = environment {
        return environments::environment_text(
            &home,
            &env,
            provider.as_deref().unwrap_or(""),
            &path,
            None,
        );
    }
    let roots = environments::local_roots(&state.store.lock(), &home)?;
    environments::read_text(path.as_ref(), &roots)
}
#[tauri::command]
pub fn save_comparison_file(
    state: State<AppState>,
    path: String,
    expected: String,
    content: String,
    environment: Option<String>,
    provider: Option<String>,
) -> Result<TextSnapshot, HkError> {
    let home = hk_core::home_dir();
    let store = hk_core::store::Store::open(&home.join(".harnesskit/metadata.db"))?;
    if let Some(snapshot) = environments::managed_files::comparison_save(
        &home,
        &state.adapters,
        &store,
        environment.as_deref(),
        &path,
        &expected,
        &content,
    )? {
        return Ok(snapshot);
    }

    if let Some(env) = environment {
        return environments::environment_text(
            &home,
            &env,
            provider.as_deref().unwrap_or(""),
            &path,
            Some((&expected, &content)),
        );
    }
    let roots = environments::local_roots(&state.store.lock(), &home)?;
    environments::save_text(
        path.as_ref(),
        &expected,
        &content,
        &roots,
        &home.join(".harnesskit/comparison-history"),
    )
}

#[tauri::command]
pub fn environment_command(
    state: State<AppState>,
    req: hk_core::environments::service::Request,
) -> Result<hk_core::environments::registry::Overview, HkError> {
    let home = hk_core::home_dir();
    let store = hk_core::store::Store::open(&home.join(".harnesskit/metadata.db"))?;
    hk_core::environments::service::execute(&home, &state.adapters, &store, req)
}

#[tauri::command]
pub fn discover_environments() -> Vec<hk_core::environments::transport::Candidate> {
    hk_core::environments::transport::discover()
}
#[tauri::command]
pub fn environment_extension(
    member: String,
    action: String,
    expected: Option<String>,
    enabled: Option<bool>,
) -> Result<serde_json::Value, HkError> {
    hk_core::environments::inventory::action(
        &hk_core::home_dir(),
        hk_core::environments::inventory::ActionRequest {
            member,
            action,
            expected,
            enabled,
        },
    )
}

#[tauri::command]
pub fn managed_file_command(
    state: State<AppState>,
    req: hk_core::environments::managed_files::Request,
) -> Result<hk_core::environments::managed_files::Response, HkError> {
    let home = hk_core::home_dir();
    let store = hk_core::store::Store::open(&home.join(".harnesskit/metadata.db"))?;
    hk_core::environments::managed_files::execute(&home, &state.adapters, &store, req)
}
