//! Tâches (brique 002-A) : tâche minimale, conversion d'une capture en tâche, annulation.
//!
//! Invariants (garantis par des transactions `IMMEDIATE`, des gardes dans les `UPDATE` et
//! des index/déclencheurs SQL — voir `migrations/0002_tasks.sql`) :
//! 1. une capture a au plus UNE tâche active ;
//! 2. une capture est « convertie » si et seulement si `converted_task_id` désigne une tâche
//!    active dont `origin_inbox_item_id` est cette capture ;
//! 3. rien n'est jamais supprimé : annuler = `deleted_at` sur la tâche, la capture revient
//!    avec son texte, sa destination et son `updated_at` d'origine ;
//! 4. une tâche modifiée depuis sa création (`updated_at` > `created_at`, ou statut autre
//!    que « à faire ») ne peut plus être annulée : la conversion ne masque pas un travail fait.
//!
//! Le texte saisi n'intervient jamais dans le SQL autrement que comme paramètre.

use std::fmt;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Serialize, Serializer};

use crate::inbox::{self, Cursor, InboxError, InboxItem};

/// Longueur maximale d'un titre, en caractères (même règle dans le schéma SQL).
pub const MAX_TITLE_CHARS: usize = 120;

/// Statuts décidés au cahier des charges. 002-A n'écrit que `Todo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Waiting,
    Blocked,
    Done,
}

impl TaskStatus {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "todo" => TaskStatus::Todo,
            "in_progress" => TaskStatus::InProgress,
            "waiting" => TaskStatus::Waiting,
            "blocked" => TaskStatus::Blocked,
            "done" => TaskStatus::Done,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    /// Copie intégrale du texte de la capture au moment de la conversion.
    pub details: String,
    pub status: TaskStatus,
    pub destination_id: Option<String>,
    /// Capture d'origine ; `None` pour une future tâche créée directement.
    pub origin_inbox_item_id: Option<String>,
    pub created_at: i64,
    /// Égal à `created_at` tant que la tâche n'a pas été modifiée.
    pub updated_at: i64,
    /// Renseigné pour une tâche annulée ou mise à la corbeille.
    pub deleted_at: Option<i64>,
}

/// Résultat d'une conversion ou d'une annulation : la tâche et la capture dans leur nouvel état.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Conversion {
    pub task: Task,
    pub item: InboxItem,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPage {
    /// De la plus récente à la plus ancienne (création).
    pub items: Vec<Task>,
    pub total: u32,
    pub next_cursor: Option<Cursor>,
}

const COLUMNS: &str =
    "id, title, details, status, destination_id, origin_inbox_item_id, created_at, updated_at, deleted_at";

fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let status: String = row.get(3)?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        details: row.get(2)?,
        // Le CHECK du schéma garantit une valeur connue ; `Todo` ne sert que de repli.
        status: TaskStatus::parse(&status).unwrap_or(TaskStatus::Todo),
        destination_id: row.get(4)?,
        origin_inbox_item_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
    })
}

/// Lit une tâche, annulée comprise.
pub fn get_task(conn: &Connection, id: &str) -> Result<Task, TaskError> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM tasks WHERE id = ?1"),
        params![id],
        map_task,
    )
    .optional()?
    .ok_or(TaskError::TaskNotFound)
}

/// Tâches actives, de la plus récente à la plus ancienne. Pagination par curseur
/// `(created_at, id)`, comme la boîte : insensible aux ajouts pendant le parcours.
pub fn list_tasks(conn: &Connection, before: Option<&Cursor>, limit: u32) -> Result<TaskPage, TaskError> {
    let limit = limit.clamp(1, inbox::MAX_LIST_LIMIT);
    let total: u32 =
        conn.query_row("SELECT count(*) FROM tasks WHERE deleted_at IS NULL", [], |row| row.get(0))?;

    let mut items: Vec<Task> = match before {
        None => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLUMNS} FROM tasks WHERE deleted_at IS NULL
                 ORDER BY created_at DESC, id DESC LIMIT ?1"
            ))?;
            let rows = stmt.query_map(params![limit + 1], map_task)?;
            rows.collect::<Result<_, _>>()?
        }
        Some(cursor) => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {COLUMNS} FROM tasks WHERE deleted_at IS NULL AND (created_at, id) < (?2, ?3)
                 ORDER BY created_at DESC, id DESC LIMIT ?1"
            ))?;
            let rows = stmt.query_map(params![limit + 1, cursor.sort_key, cursor.id], map_task)?;
            rows.collect::<Result<_, _>>()?
        }
    };
    let next_cursor = if items.len() > limit as usize {
        items.truncate(limit as usize);
        items.last().map(|last| Cursor { sort_key: last.created_at, id: last.id.clone() })
    } else {
        None
    };
    Ok(TaskPage { items, total, next_cursor })
}

/// Titre proposé : première ligne non vide, espaces normalisés, 120 caractères au plus
/// (« … » en fin si coupé). Le texte complet reste de toute façon dans `details`.
pub fn suggest_title(content: &str) -> String {
    let line = content.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let line = collapse_spaces(line);
    if line.chars().count() <= MAX_TITLE_CHARS {
        return line;
    }
    let mut cut: String = line.chars().take(MAX_TITLE_CHARS - 1).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    cut
}

/// Titre proposé pour une capture existante (lecture seule).
pub fn suggest_title_for_item(conn: &Connection, item_id: &str) -> Result<String, TaskError> {
    let item = inbox::get_item(conn, item_id).map_err(item_error)?;
    Ok(suggest_title(&item.content))
}

fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Titre fourni par l'utilisateur : espaces normalisés, ni vide ni trop long.
fn normalize_title(title: &str) -> Result<String, TaskError> {
    let title = collapse_spaces(title);
    if title.is_empty() {
        return Err(TaskError::EmptyTitle);
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(TaskError::TitleTooLong);
    }
    Ok(title)
}

fn item_error(error: InboxError) -> TaskError {
    match error {
        InboxError::NotFound => TaskError::ItemNotFound,
        InboxError::Storage(cause) => TaskError::Storage(cause),
        _ => TaskError::Unavailable,
    }
}

/// Transforme une capture en tâche, en une seule transaction.
///
/// `expected_updated_at` : la version de la capture que l'utilisateur a sous les yeux ; une
/// capture modifiée depuis est refusée (`Conflict`), jamais convertie « à l'aveugle ».
/// `title` : `None` utilise la suggestion. La capture garde son texte, sa destination et son
/// `updated_at` ; seuls `converted_at` et `converted_task_id` changent.
pub fn convert_item(
    conn: &Connection,
    item_id: &str,
    expected_updated_at: i64,
    title: Option<&str>,
    now_ms: i64,
) -> Result<Conversion, TaskError> {
    let given_title = title.map(normalize_title).transpose()?;

    // IMMEDIATE : le verrou d'écriture est pris AVANT les lectures de contrôle, donc rien ne
    // peut les rendre périmées avant l'écriture (y compris depuis une autre connexion).
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;

    let item = inbox::get_item(&tx, item_id).map_err(item_error)?;
    if item.deleted_at.is_some() {
        return Err(TaskError::ItemTrashed);
    }
    if let Some(task_id) = item.converted_task_id.clone() {
        return Err(TaskError::AlreadyConverted { task_id });
    }
    if item.updated_at != expected_updated_at {
        return Err(TaskError::Conflict);
    }
    let title = match given_title {
        Some(title) => title,
        None => {
            let suggestion = suggest_title(&item.content);
            normalize_title(&suggestion)?
        }
    };

    let task_id = uuid::Uuid::now_v7().to_string();
    let inserted = tx.execute(
        "INSERT INTO tasks (id, title, details, status, destination_id, origin_inbox_item_id,
                            created_at, updated_at)
         VALUES (?1, ?2, ?3, 'todo', ?4, ?5, ?6, ?6)",
        params![task_id, title, item.content, item.destination_id, item.id, now_ms],
    );
    if let Err(error) = inserted {
        return Err(unique_violation_to_already_converted(&tx, &item.id, error));
    }

    let marked = tx.execute(
        "UPDATE inbox_items SET converted_at = ?2, converted_task_id = ?3
         WHERE id = ?1 AND deleted_at IS NULL AND converted_at IS NULL AND updated_at = ?4",
        params![item.id, now_ms, task_id, expected_updated_at],
    )?;
    if marked != 1 {
        // Impossible sous le verrou IMMEDIATE ; en cas de doute, tout est annulé (drop = rollback).
        return Err(TaskError::Inconsistent);
    }
    tx.commit()?;

    Ok(Conversion {
        task: get_task(conn, &task_id)?,
        item: inbox::get_item(conn, &item.id).map_err(item_error)?,
    })
}

/// L'index « une tâche active par capture » a refusé l'insertion : une tâche active existe.
fn unique_violation_to_already_converted(
    tx: &Transaction<'_>,
    item_id: &str,
    error: rusqlite::Error,
) -> TaskError {
    let violated = matches!(
        &error,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == rusqlite::ErrorCode::ConstraintViolation
    );
    if violated {
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM tasks WHERE origin_inbox_item_id = ?1 AND deleted_at IS NULL",
                params![item_id],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten();
        if let Some(task_id) = existing {
            return TaskError::AlreadyConverted { task_id };
        }
    }
    TaskError::Storage(error)
}

/// Annule la conversion : la capture revient dans « À organiser » à l'identique, la tâche
/// est conservée (`deleted_at`). Refusée si la tâche a été modifiée ou avancée.
pub fn cancel_conversion(conn: &Connection, task_id: &str, now_ms: i64) -> Result<Conversion, TaskError> {
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;

    let task = get_task(&tx, task_id)?;
    if task.deleted_at.is_some() {
        return Err(TaskError::NotActive);
    }
    let Some(origin) = task.origin_inbox_item_id.clone() else {
        return Err(TaskError::NoOrigin);
    };

    // La garde « non modifiée » est dans le UPDATE lui-même.
    let cancelled = tx.execute(
        "UPDATE tasks SET deleted_at = ?2
         WHERE id = ?1 AND deleted_at IS NULL AND updated_at = created_at AND status = 'todo'",
        params![task_id, now_ms],
    )?;
    if cancelled != 1 {
        return Err(TaskError::Modified);
    }
    let restored = tx.execute(
        "UPDATE inbox_items SET converted_at = NULL, converted_task_id = NULL
         WHERE id = ?1 AND converted_task_id = ?2",
        params![origin, task_id],
    )?;
    if restored != 1 {
        return Err(TaskError::Inconsistent); // tout est annulé : la tâche reste active
    }
    tx.commit()?;

    Ok(Conversion {
        task: get_task(conn, task_id)?,
        item: inbox::get_item(conn, &origin).map_err(item_error)?,
    })
}

#[derive(Debug)]
pub enum TaskError {
    /// Capture introuvable.
    ItemNotFound,
    /// Capture dans la corbeille : à restaurer d'abord.
    ItemTrashed,
    /// La capture est déjà convertie en cette tâche active.
    AlreadyConverted { task_id: String },
    /// La capture a été modifiée depuis son ouverture.
    Conflict,
    EmptyTitle,
    TitleTooLong,
    /// Tâche introuvable.
    TaskNotFound,
    /// Tâche déjà annulée.
    NotActive,
    /// Tâche créée sans capture d'origine : rien à remettre dans la boîte.
    NoOrigin,
    /// Tâche modifiée ou avancée depuis sa création : annulation refusée.
    Modified,
    /// État incohérent détecté ; la transaction a été annulée sans rien modifier.
    Inconsistent,
    Storage(rusqlite::Error),
    Unavailable,
}

impl TaskError {
    /// Code stable, utilisable par l'interface.
    pub fn code(&self) -> &'static str {
        match self {
            TaskError::ItemNotFound => "not_found",
            TaskError::ItemTrashed => "trashed",
            TaskError::AlreadyConverted { .. } => "already_converted",
            TaskError::Conflict => "version_conflict",
            TaskError::EmptyTitle => "empty_title",
            TaskError::TitleTooLong => "title_too_long",
            TaskError::TaskNotFound => "task_not_found",
            TaskError::NotActive => "task_not_active",
            TaskError::NoOrigin => "no_origin",
            TaskError::Modified => "task_modified",
            TaskError::Inconsistent => "inconsistent_state",
            TaskError::Storage(_) => "storage",
            TaskError::Unavailable => "unavailable",
        }
    }
}

impl fmt::Display for TaskError {
    /// Message destiné à l'utilisateur : ne reprend jamais le contenu de la capture.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::ItemNotFound => write!(f, "Cette capture n'existe plus."),
            TaskError::ItemTrashed => {
                write!(f, "Cette capture est dans la corbeille : restaurez-la d'abord.")
            }
            TaskError::AlreadyConverted { .. } => {
                write!(f, "Cette capture est déjà transformée en tâche.")
            }
            TaskError::Conflict => write!(f, "Cette capture a été modifiée depuis son ouverture."),
            TaskError::EmptyTitle => write!(f, "Le titre de la tâche est vide."),
            TaskError::TitleTooLong => {
                write!(f, "Le titre dépasse {MAX_TITLE_CHARS} caractères.")
            }
            TaskError::TaskNotFound => write!(f, "Cette tâche n'existe plus."),
            TaskError::NotActive => write!(f, "Cette tâche est déjà annulée."),
            TaskError::NoOrigin => write!(f, "Cette tâche ne vient pas d'une capture."),
            TaskError::Modified => write!(
                f,
                "Cette tâche a déjà été modifiée : ouvrez-la plutôt que d'annuler sa création."
            ),
            TaskError::Inconsistent => {
                write!(f, "Un état inattendu a été détecté ; rien n'a été modifié.")
            }
            TaskError::Storage(_) => write!(f, "L'enregistrement local a échoué."),
            TaskError::Unavailable => write!(f, "La base de données est indisponible."),
        }
    }
}

impl std::error::Error for TaskError {}

impl From<rusqlite::Error> for TaskError {
    fn from(error: rusqlite::Error) -> Self {
        TaskError::Storage(error)
    }
}

/// Envoyé à l'interface : `{ "code", "message" }`, plus `taskId` quand la capture est déjà
/// convertie (pour proposer d'ouvrir la tâche existante).
impl Serialize for TaskError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("TaskError", 3)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        if let TaskError::AlreadyConverted { task_id } = self {
            state.serialize_field("taskId", task_id)?;
        }
        state.end()
    }
}

#[cfg(test)]
mod tests;
