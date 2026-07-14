use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AppBuildDto {
    pub version: String,
    pub sha: String,
}

#[tauri::command]
pub fn app_build() -> AppBuildDto {
    AppBuildDto {
        version: env!("CARGO_PKG_VERSION").to_string(),
        sha: env!("COWORK_BUILD_SHA").to_string(),
    }
}
