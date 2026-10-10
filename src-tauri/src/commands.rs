//! Commandes appelables depuis l'interface (via `invoke` côté TypeScript).
//!
//! Elles ne font que relayer vers les modules de domaine ; chacune est déclarée dans
//! `build.rs` et autorisée nommément dans `capabilities/default.json`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::State;

use crate::environment::Channel;
use crate::inbox::{self, Cursor, Destination, InboxError, InboxFilter, InboxItem, InboxPage, Scope};
use crate::{AppEnvironment, Database};

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

/// Exécute `action` avec la connexion, accès un par un.
fn with_db<T>(
    db: &Database,
    action: impl FnOnce(&rusqlite::Connection) -> Result<T, InboxError>,
) -> Result<T, InboxError> {
    let conn = db.0.lock().map_err(|_| InboxError::Unavailable)?;
    let result = action(&conn);
    if let Err(InboxError::Storage(cause)) = &result {
        // Cause technique pour le diagnostic (terminal de développement uniquement).
        // Requêtes paramétrées : elle ne contient jamais le texte saisi.
        eprintln!("[M'Organiser] erreur SQLite : {cause}");
    }
    result
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// Commandes `async` : exécutées hors du fil principal, la fenêtre reste fluide.

#[tauri::command]
pub async fn list_destinations(db: State<'_, Database>) -> Result<Vec<Destination>, InboxError> {
    with_db(&db, inbox::list_destinations)
}

#[tauri::command]
pub async fn list_inbox_items(
    db: State<'_, Database>,
    filter: InboxFilter,
    limit: u32,
    before: Option<Cursor>,
) -> Result<InboxPage, InboxError> {
    with_db(&db, |conn| {
        inbox::list_items(conn, &filter, Scope::Active, before.as_ref(), limit)
    })
}

#[tauri::command]
pub async fn list_trashed_items(
    db: State<'_, Database>,
    limit: u32,
    before: Option<Cursor>,
) -> Result<InboxPage, InboxError> {
    with_db(&db, |conn| {
        inbox::list_items(conn, &InboxFilter::All, Scope::Trash, before.as_ref(), limit)
    })
}

#[tauri::command]
pub async fn get_inbox_item(db: State<'_, Database>, id: String) -> Result<InboxItem, InboxError> {
    with_db(&db, |conn| inbox::get_item(conn, &id))
}

#[tauri::command]
pub async fn update_inbox_item(
    db: State<'_, Database>,
    id: String,
    content: String,
    destination_id: Option<String>,
    expected_updated_at: i64,
) -> Result<InboxItem, InboxError> {
    with_db(&db, |conn| {
        inbox::update_item(
            conn,
            &id,
            &content,
            destination_id.as_deref(),
            expected_updated_at,
            now_ms(),
        )
    })
}

#[tauri::command]
pub async fn trash_inbox_item(db: State<'_, Database>, id: String) -> Result<InboxItem, InboxError> {
    with_db(&db, |conn| inbox::trash_item(conn, &id, now_ms()))
}

#[tauri::command]
pub async fn restore_inbox_item(db: State<'_, Database>, id: String) -> Result<InboxItem, InboxError> {
    with_db(&db, |conn| inbox::restore_item(conn, &id))
}

#[tauri::command]
pub async fn create_inbox_item(
    db: State<'_, Database>,
    content: String,
    destination_id: Option<String>,
) -> Result<InboxItem, InboxError> {
    with_db(&db, |conn| {
        inbox::create_item(conn, &content, destination_id.as_deref(), now_ms())
    })
}
