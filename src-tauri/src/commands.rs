//! Commandes appelables depuis l'interface (via `invoke` côté TypeScript).
//!
//! Elles ne font que relayer vers les modules de domaine ; chacune est déclarée dans
//! `build.rs` et autorisée nommément dans `capabilities/default.json`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::State;

use crate::environment::Channel;
use crate::inbox::{self, Cursor, Destination, InboxError, InboxFilter, InboxItem, InboxPage, Scope};
use crate::tasks::{self, Conversion, Task, TaskError, TaskPage};
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

/// Erreurs de domaine que `with_db` sait produire ou journaliser.
trait DomainError {
    fn unavailable() -> Self;
    fn storage_cause(&self) -> Option<&rusqlite::Error>;
}

impl DomainError for InboxError {
    fn unavailable() -> Self {
        InboxError::Unavailable
    }
    fn storage_cause(&self) -> Option<&rusqlite::Error> {
        match self {
            InboxError::Storage(cause) => Some(cause),
            _ => None,
        }
    }
}

impl DomainError for TaskError {
    fn unavailable() -> Self {
        TaskError::Unavailable
    }
    fn storage_cause(&self) -> Option<&rusqlite::Error> {
        match self {
            TaskError::Storage(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Exécute `action` avec la connexion, accès un par un.
fn with_db<T, E: DomainError>(
    db: &Database,
    action: impl FnOnce(&rusqlite::Connection) -> Result<T, E>,
) -> Result<T, E> {
    let conn = db.0.lock().map_err(|_| E::unavailable())?;
    let result = action(&conn);
    if let Some(cause) = result.as_ref().err().and_then(DomainError::storage_cause) {
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

// --- Conversion des captures en tâches (002-A) ---

/// Transforme une capture en tâche. `title` : `None` utilise le titre proposé.
#[tauri::command]
pub async fn convert_inbox_item_to_task(
    db: State<'_, Database>,
    id: String,
    expected_updated_at: i64,
    title: Option<String>,
) -> Result<Conversion, TaskError> {
    with_db(&db, |conn| {
        tasks::convert_item(conn, &id, expected_updated_at, title.as_deref(), now_ms())
    })
}

/// Annule la conversion : la capture revient dans la boîte, la tâche est conservée.
#[tauri::command]
pub async fn cancel_task_conversion(db: State<'_, Database>, id: String) -> Result<Conversion, TaskError> {
    with_db(&db, |conn| tasks::cancel_conversion(conn, &id, now_ms()))
}

/// Titre proposé pour la conversion d'une capture (lecture seule).
#[tauri::command]
pub async fn suggest_task_title(db: State<'_, Database>, id: String) -> Result<String, TaskError> {
    with_db(&db, |conn| tasks::suggest_title_for_item(conn, &id))
}

#[tauri::command]
pub async fn list_tasks(
    db: State<'_, Database>,
    limit: u32,
    before: Option<Cursor>,
) -> Result<TaskPage, TaskError> {
    with_db(&db, |conn| tasks::list_tasks(conn, before.as_ref(), limit))
}

#[tauri::command]
pub async fn get_task(db: State<'_, Database>, id: String) -> Result<Task, TaskError> {
    with_db(&db, |conn| tasks::get_task(conn, &id))
}

/// Captures transformées en tâche (« Traitées »), de la plus récemment traitée à la plus ancienne.
#[tauri::command]
pub async fn list_converted_items(
    db: State<'_, Database>,
    limit: u32,
    before: Option<Cursor>,
) -> Result<InboxPage, InboxError> {
    with_db(&db, |conn| {
        inbox::list_items(conn, &InboxFilter::All, Scope::Converted, before.as_ref(), limit)
    })
}
