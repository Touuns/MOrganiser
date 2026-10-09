//! Commandes appelables depuis l'interface (via `invoke` côté TypeScript).

use serde::Serialize;
use tauri::State;

use crate::environment::Channel;
use crate::AppEnvironment;

/// Informations de diagnostic affichées dans la fenêtre. Lecture seule.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    channel: Channel,
    version: String,
    data_dir: String,
}

#[tauri::command]
pub fn app_info(app: tauri::AppHandle, env: State<'_, AppEnvironment>) -> AppInfo {
    AppInfo {
        channel: env.channel,
        version: app.package_info().version.to_string(),
        data_dir: env.data_dir.display().to_string(),
    }
}
