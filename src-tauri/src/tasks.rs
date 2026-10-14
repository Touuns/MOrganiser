//! Tâches : modèle enrichi (003-B1b), conversion d'une capture en tâche (002-A, 003-A),
//! création directe, modification, annulation de conversion.
//!
//! Invariants (garantis par des transactions `IMMEDIATE`, des gardes dans les `UPDATE` et
//! des index/déclencheurs SQL — voir `migrations/0002_tasks.sql` et `0003_task_model.sql`) :
//! 1. une capture a au plus UNE tâche active ;
//! 2. une capture est « convertie » si et seulement si `converted_task_id` désigne une tâche
//!    active dont `origin_inbox_item_id` est cette capture ;
//! 3. rien n'est jamais supprimé : annuler = `deleted_at` sur la tâche, la capture revient
//!    avec son texte, sa destination et son `updated_at` d'origine ;
//! 4. une tâche modifiée depuis sa création (`updated_at` > `created_at`, ou statut autre
//!    que « à faire ») ne peut plus être annulée : la conversion ne masque pas un travail fait.
//!
//! 5. une tâche créée directement n'a PAS de capture d'origine (aucune provenance inventée) ;
//!    la provenance d'une tâche existante ne se réécrit jamais ;
//! 6. univers (`responsibility_id`) et rubrique (`section_id`) sont deux dimensions
//!    indépendantes et facultatives, chacune d'une nature précise de destination.
//!
//! Le texte saisi n'intervient jamais dans le SQL autrement que comme paramètre.

use std::fmt;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize, Serializer};

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
    /// Description de la tâche. Par défaut (conversion sans description fournie), copie intégrale
    /// du texte de la capture ; depuis 003-A, l'utilisateur peut la réécrire avant de créer la
    /// tâche. La capture, elle, n'est jamais modifiée par la conversion.
    pub details: String,
    pub status: TaskStatus,
    /// Univers de responsabilité (Moi, Externe), facultatif.
    pub responsibility_id: Option<String>,
    /// Rubrique (Administratif, Finances, Inventaire…), facultative.
    pub section_id: Option<String>,
    /// Jour calendaire `AAAA-MM-JJ`, sans heure ni fuseau.
    pub start_date: Option<String>,
    pub due_date: Option<String>,
    /// Date effective de réalisation : colonne préparée, utilisée à partir de 003-C.
    pub completed_on: Option<String>,
    /// 1 basse, 2 normale, 3 haute (4 et 5 réservées) ; `None` = non définie.
    pub priority: Option<i64>,
    /// Capture d'origine ; `None` pour une tâche créée directement. Immuable.
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

const COLUMNS: &str = "id, title, details, status, responsibility_id, section_id, start_date, due_date, \
     completed_on, priority, origin_inbox_item_id, created_at, updated_at, deleted_at";

fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let status: String = row.get(3)?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        details: row.get(2)?,
        // Le CHECK du schéma garantit une valeur connue ; `Todo` ne sert que de repli.
        status: TaskStatus::parse(&status).unwrap_or(TaskStatus::Todo),
        responsibility_id: row.get(4)?,
        section_id: row.get(5)?,
        start_date: row.get(6)?,
        due_date: row.get(7)?,
        completed_on: row.get(8)?,
        priority: row.get(9)?,
        origin_inbox_item_id: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        deleted_at: row.get(13)?,
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

/// Classement de la tâche à la conversion.
///
/// Côté interface : `{ "type": "keep" }`, `{ "type": "clear" }`, `{ "type": "set", "id": "…" }`
/// ou `{ "type": "split", "responsibilityId": …, "sectionId": … }`. États explicites, car
/// « absent » et « aucune » ne se distinguent pas dans une commande.
///
/// La capture garde UNE destination ; la tâche en porte deux (univers et rubrique). Les trois
/// premières variantes (003-A) restent donc interprétées ainsi :
/// - `Keep` : la destination de la capture est reprise dans SA dimension (selon sa nature) ;
/// - `Clear` : aucun classement ;
/// - `Set` : cette destination, placée dans sa dimension ; l'autre dimension reste vide.
///
/// `Split` fixe explicitement les deux dimensions (`null` = aucune).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DestinationChoice {
    /// Reprendre la destination enregistrée de la capture (comportement historique).
    #[default]
    Keep,
    /// Tâche sans classement, quelle que soit la destination de la capture.
    Clear,
    /// Une autre destination, qui doit exister et être active.
    Set { id: String },
    /// Univers et rubrique choisis séparément.
    #[serde(rename_all = "camelCase")]
    Split {
        #[serde(default)]
        responsibility_id: Option<String>,
        #[serde(default)]
        section_id: Option<String>,
    },
}

/// Ce que l'utilisateur a saisi dans le brouillon de tâche. Tout est facultatif : un champ
/// absent reprend la valeur de la capture ou la suggestion (comportement de la brique 002).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConversionDraft {
    /// `None` : titre suggéré.
    pub title: Option<String>,
    /// `None` : copie intégrale du texte de la capture.
    pub details: Option<String>,
    pub destination: DestinationChoice,
    pub start_date: Option<String>,
    pub due_date: Option<String>,
    /// 1 à 3 ; `None` = non définie.
    pub priority: Option<i64>,
}

/// Transforme une capture en tâche avec les valeurs par défaut (titre suggéré, texte et
/// destination de la capture). Voir [`convert_item_with`]. Réservée aux tests (l'application passe
/// toujours par `convert_item_with`) : elle garde leur signature historique.
#[cfg(test)]
pub fn convert_item(
    conn: &Connection,
    item_id: &str,
    expected_updated_at: i64,
    title: Option<&str>,
    now_ms: i64,
) -> Result<Conversion, TaskError> {
    let draft = ConversionDraft { title: title.map(str::to_owned), ..ConversionDraft::default() };
    convert_item_with(conn, item_id, expected_updated_at, &draft, now_ms)
}

/// Description fournie par l'utilisateur : extrémités nettoyées, **vide acceptée** (description
/// facultative depuis 003-B), pas trop longue. Les retours à la ligne et espaces intérieurs sont
/// conservés (c'est un texte libre).
fn normalize_details(details: &str) -> Result<String, TaskError> {
    let details = details.trim();
    if details.chars().count() > inbox::MAX_CONTENT_CHARS {
        return Err(TaskError::DetailsTooLong);
    }
    Ok(details.to_owned())
}

/// Transforme une capture en tâche, en une seule transaction.
///
/// `expected_updated_at` : la version de la capture que l'utilisateur a sous les yeux ; une
/// capture modifiée depuis est refusée (`Conflict`), jamais convertie « à l'aveugle ».
/// `draft` : titre (suggestion si absent), description (texte de la capture si absente) et
/// destination. La capture garde son texte, sa destination et son `updated_at` quoi que la
/// tâche contienne ; seuls `converted_at` et `converted_task_id` changent.
pub fn convert_item_with(
    conn: &Connection,
    item_id: &str,
    expected_updated_at: i64,
    draft: &ConversionDraft,
    now_ms: i64,
) -> Result<Conversion, TaskError> {
    // Validations qui ne dépendent pas de la base : avant de prendre le verrou d'écriture.
    let given_title = draft.title.as_deref().map(normalize_title).transpose()?;
    let given_details = draft.details.as_deref().map(normalize_details).transpose()?;
    validate_schedule(draft.start_date.as_deref(), draft.due_date.as_deref(), draft.priority)?;

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
    let details = given_details.unwrap_or_else(|| item.content.clone());

    // Le classement de la tâche est indépendant de celui de la capture, qui ne change jamais.
    let (responsibility_id, section_id) = resolve_classification(&tx, &item, &draft.destination)?;

    let task_id = uuid::Uuid::now_v7().to_string();
    let inserted = tx.execute(
        "INSERT INTO tasks (id, title, details, status, responsibility_id, section_id,
                            start_date, due_date, priority, origin_inbox_item_id,
                            created_at, updated_at)
         VALUES (?1, ?2, ?3, 'todo', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![
            task_id,
            title,
            details,
            responsibility_id,
            section_id,
            draft.start_date,
            draft.due_date,
            draft.priority,
            item.id,
            now_ms
        ],
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

// --- Champs d'une tâche : validation commune (conversion, création, modification) ------------

/// Ce que l'utilisateur saisit pour créer ou modifier une tâche. Validé UNE fois côté Rust.
/// Le statut, l'origine et la date de réalisation n'en font pas partie : une édition ne les
/// modifie jamais implicitement.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFields {
    pub title: String,
    /// Description facultative : vide acceptée.
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub responsibility_id: Option<String>,
    #[serde(default)]
    pub section_id: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub priority: Option<i64>,
}

/// Niveaux de priorité acceptés pour de NOUVELLES saisies (le schéma admet 1 à 5).
const PRIORITY_RANGE: std::ops::RangeInclusive<i64> = 1..=3;

/// `AAAA-MM-JJ` strict ET jour réel du calendrier. Au moins aussi strict que le `CHECK`
/// `date(x) IS x` du schéma, pour renvoyer une erreur claire plutôt qu'un échec de stockage.
pub fn is_valid_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) =
        (value[0..4].parse::<u32>(), value[5..7].parse::<u32>(), value[8..10].parse::<u32>())
    else {
        return false;
    };
    if year == 0 {
        return false;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// Dates calendaires valides, début non postérieur à l'échéance, priorité parmi 1 à 3.
/// (La comparaison de chaînes `AAAA-MM-JJ` est chronologique.)
fn validate_schedule(
    start_date: Option<&str>,
    due_date: Option<&str>,
    priority: Option<i64>,
) -> Result<(), TaskError> {
    for date in [start_date, due_date].into_iter().flatten() {
        if !is_valid_calendar_date(date) {
            return Err(TaskError::InvalidDate);
        }
    }
    if let (Some(start), Some(due)) = (start_date, due_date) {
        if start > due {
            return Err(TaskError::DatesIncoherent);
        }
    }
    if let Some(level) = priority {
        if !PRIORITY_RANGE.contains(&level) {
            return Err(TaskError::InvalidPriority);
        }
    }
    Ok(())
}

/// Champs normalisés et validés (hors vérification des destinations, qui lit la base).
struct ValidFields {
    title: String,
    details: String,
    responsibility_id: Option<String>,
    section_id: Option<String>,
    start_date: Option<String>,
    due_date: Option<String>,
    priority: Option<i64>,
}

fn validate_fields(fields: &TaskFields) -> Result<ValidFields, TaskError> {
    let title = normalize_title(&fields.title)?;
    let details = normalize_details(&fields.details)?;
    validate_schedule(fields.start_date.as_deref(), fields.due_date.as_deref(), fields.priority)?;
    Ok(ValidFields {
        title,
        details,
        responsibility_id: fields.responsibility_id.clone().filter(|id| !id.is_empty()),
        section_id: fields.section_id.clone().filter(|id| !id.is_empty()),
        start_date: fields.start_date.clone(),
        due_date: fields.due_date.clone(),
        priority: fields.priority,
    })
}

/// Nature et état d'une destination, `None` si elle n'existe pas.
fn destination_info(conn: &Connection, id: &str) -> Result<Option<(String, bool)>, TaskError> {
    Ok(conn
        .query_row(
            "SELECT kind, archived_at IS NOT NULL FROM destinations WHERE id = ?1",
            params![id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()?)
}

/// Une destination choisie pour une dimension : elle existe, est active (sauf si c'est la valeur
/// DÉJÀ portée, qui reste valide même archivée depuis) et a la bonne nature.
fn check_dimension(
    conn: &Connection,
    id: &str,
    expected_kind: &str,
    already: Option<&str>,
) -> Result<(), TaskError> {
    let Some((kind, archived)) = destination_info(conn, id)? else {
        return Err(TaskError::UnknownDestination);
    };
    if kind != expected_kind {
        return Err(TaskError::WrongDestinationKind);
    }
    if archived && already != Some(id) {
        return Err(TaskError::UnknownDestination);
    }
    Ok(())
}

/// Répartit un identifiant de destination dans sa dimension selon sa NATURE.
fn place_by_kind(kind: &str, id: String) -> (Option<String>, Option<String>) {
    if kind == "responsibility" {
        (Some(id), None)
    } else {
        (None, Some(id))
    }
}

/// Classement (univers, rubrique) d'une tâche issue d'une capture.
fn resolve_classification(
    conn: &Connection,
    item: &InboxItem,
    choice: &DestinationChoice,
) -> Result<(Option<String>, Option<String>), TaskError> {
    let from_capture = item.destination_id.as_deref();
    match choice {
        DestinationChoice::Keep => match from_capture {
            None => Ok((None, None)),
            Some(id) => match destination_info(conn, id)? {
                Some((kind, _)) => Ok(place_by_kind(&kind, id.to_owned())),
                None => Err(TaskError::UnknownDestination),
            },
        },
        DestinationChoice::Clear => Ok((None, None)),
        DestinationChoice::Set { id } => {
            let Some((kind, archived)) = destination_info(conn, id)? else {
                return Err(TaskError::UnknownDestination);
            };
            // Une destination inchangée reste acceptée même archivée depuis (comme en 001-B).
            if archived && from_capture != Some(id.as_str()) {
                return Err(TaskError::UnknownDestination);
            }
            Ok(place_by_kind(&kind, id.clone()))
        }
        DestinationChoice::Split { responsibility_id, section_id } => {
            let responsibility_id = responsibility_id.clone().filter(|id| !id.is_empty());
            let section_id = section_id.clone().filter(|id| !id.is_empty());
            if let Some(id) = &responsibility_id {
                check_dimension(conn, id, "responsibility", from_capture)?;
            }
            if let Some(id) = &section_id {
                check_dimension(conn, id, "section", from_capture)?;
            }
            Ok((responsibility_id, section_id))
        }
    }
}

/// Crée une tâche **sans capture d'origine** (aucune provenance inventée), au statut « à faire ».
pub fn create_task(conn: &Connection, fields: &TaskFields, now_ms: i64) -> Result<Task, TaskError> {
    let valid = validate_fields(fields)?;
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    if let Some(id) = &valid.responsibility_id {
        check_dimension(&tx, id, "responsibility", None)?;
    }
    if let Some(id) = &valid.section_id {
        check_dimension(&tx, id, "section", None)?;
    }
    let task_id = uuid::Uuid::now_v7().to_string();
    tx.execute(
        "INSERT INTO tasks (id, title, details, status, responsibility_id, section_id,
                            start_date, due_date, priority, origin_inbox_item_id,
                            created_at, updated_at)
         VALUES (?1, ?2, ?3, 'todo', ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?9)",
        params![
            task_id,
            valid.title,
            valid.details,
            valid.responsibility_id,
            valid.section_id,
            valid.start_date,
            valid.due_date,
            valid.priority,
            now_ms
        ],
    )?;
    tx.commit()?;
    get_task(conn, &task_id)
}

/// Modifie les champs éditables d'une tâche active.
///
/// `expected_updated_at` : la version que l'utilisateur a ouverte ; l'écriture n'a lieu que si elle
/// est toujours la version enregistrée (vérifié dans le `UPDATE` lui-même). Toute modification
/// effective fait croître strictement `updated_at` ; sans changement réel, **rien n'est écrit**.
/// Le statut, l'origine, la date de réalisation et `created_at` ne sont jamais touchés.
pub fn update_task(
    conn: &Connection,
    id: &str,
    expected_updated_at: i64,
    fields: &TaskFields,
    now_ms: i64,
) -> Result<Task, TaskError> {
    let valid = validate_fields(fields)?;
    let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;

    let current = get_task(&tx, id)?;
    if current.deleted_at.is_some() {
        return Err(TaskError::NotActive);
    }
    if current.updated_at != expected_updated_at {
        return Err(TaskError::TaskConflict);
    }
    // Une valeur DÉJÀ portée reste valide même si sa destination a été archivée depuis.
    if let Some(value) = &valid.responsibility_id {
        check_dimension(&tx, value, "responsibility", current.responsibility_id.as_deref())?;
    }
    if let Some(value) = &valid.section_id {
        check_dimension(&tx, value, "section", current.section_id.as_deref())?;
    }

    let unchanged = current.title == valid.title
        && current.details == valid.details
        && current.responsibility_id == valid.responsibility_id
        && current.section_id == valid.section_id
        && current.start_date == valid.start_date
        && current.due_date == valid.due_date
        && current.priority == valid.priority;
    if unchanged {
        return Ok(current);
    }

    let new_updated_at = now_ms.max(expected_updated_at + 1);
    let changed = tx.execute(
        "UPDATE tasks
         SET title = ?3, details = ?4, responsibility_id = ?5, section_id = ?6,
             start_date = ?7, due_date = ?8, priority = ?9, updated_at = ?10
         WHERE id = ?1 AND updated_at = ?2 AND deleted_at IS NULL",
        params![
            id,
            expected_updated_at,
            valid.title,
            valid.details,
            valid.responsibility_id,
            valid.section_id,
            valid.start_date,
            valid.due_date,
            valid.priority,
            new_updated_at
        ],
    )?;
    if changed != 1 {
        // Impossible sous le verrou IMMEDIATE ; en cas de doute, tout est annulé (drop = rollback).
        return Err(TaskError::Inconsistent);
    }
    tx.commit()?;
    get_task(conn, id)
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
    DetailsTooLong,
    /// Destination choisie pour la tâche inconnue ou archivée.
    UnknownDestination,
    /// Une rubrique dans l'univers, ou un univers dans la rubrique.
    WrongDestinationKind,
    /// Date absente du calendrier ou hors du format `AAAA-MM-JJ`.
    InvalidDate,
    /// Date de début postérieure à l'échéance.
    DatesIncoherent,
    /// Priorité hors des niveaux disponibles (1 à 3).
    InvalidPriority,
    /// La tâche a été modifiée depuis son ouverture.
    TaskConflict,
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
            TaskError::DetailsTooLong => "details_too_long",
            TaskError::UnknownDestination => "unknown_destination",
            TaskError::WrongDestinationKind => "destination_kind",
            TaskError::InvalidDate => "invalid_date",
            TaskError::DatesIncoherent => "dates_incoherent",
            TaskError::InvalidPriority => "invalid_priority",
            TaskError::TaskConflict => "task_version_conflict",
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
            TaskError::DetailsTooLong => {
                write!(f, "La description dépasse {} caractères.", inbox::MAX_CONTENT_CHARS)
            }
            TaskError::UnknownDestination => write!(f, "Cette destination n'est pas disponible."),
            TaskError::WrongDestinationKind => {
                write!(f, "Cette destination ne peut pas être utilisée à cet endroit.")
            }
            TaskError::InvalidDate => {
                write!(f, "Une date n'est pas valide : utilisez une vraie date du calendrier.")
            }
            TaskError::DatesIncoherent => {
                write!(f, "La date de début ne peut pas être postérieure à l'échéance.")
            }
            TaskError::InvalidPriority => write!(f, "Cette priorité n'est pas disponible."),
            TaskError::TaskConflict => write!(f, "Cette tâche a été modifiée depuis son ouverture."),
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
