//! Boîte « À organiser » : création et lecture des captures, destinations.
//!
//! Le contenu saisi est toujours traité comme une donnée : requêtes paramétrées
//! uniquement, aucune construction de SQL à partir du texte de l'utilisateur.

use std::fmt;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize, Serializer};

/// Longueur maximale d'une capture, en caractères (même règle dans le schéma SQL).
pub const MAX_CONTENT_CHARS: usize = 10_000;
/// Nombre maximal de captures renvoyées en une fois.
pub const MAX_LIST_LIMIT: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DestinationKind {
    /// Espace de responsabilité (Moi, Externe).
    Responsibility,
    /// Rubrique fonctionnelle (Administratif, Finances, Inventaire).
    Section,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Destination {
    pub id: String,
    pub label: String,
    pub kind: DestinationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub content: String,
    pub destination_id: Option<String>,
    pub created_at: i64,
    /// Dernière modification du texte ou de la destination (pas la mise à la corbeille).
    pub updated_at: i64,
    /// Renseigné seulement pour une capture dans la corbeille.
    pub deleted_at: Option<i64>,
}

/// Position dans une liste, pour charger les éléments plus anciens. Opaque pour l'interface :
/// elle renvoie telle quelle la valeur reçue dans `next_cursor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cursor {
    /// `created_at` (boîte) ou `deleted_at` (corbeille) du dernier élément reçu.
    pub sort_key: i64,
    pub id: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxPage {
    /// Du plus récent au plus ancien.
    pub items: Vec<InboxItem>,
    /// Nombre total d'éléments correspondant au filtre (au-delà de la page).
    pub total: u32,
    /// Présent s'il reste des éléments plus anciens à charger.
    pub next_cursor: Option<Cursor>,
}

/// Où chercher : captures actives de la boîte, ou corbeille.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Active,
    Trash,
}

/// Filtre d'affichage de la boîte. Le texte saisi n'intervient jamais ici.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InboxFilter {
    All,
    /// Captures sans destination (« À classer »).
    Unclassified,
    Destination { id: String },
}

/// Destinations actives, dans l'ordre d'affichage.
pub fn list_destinations(conn: &Connection) -> Result<Vec<Destination>, InboxError> {
    let mut stmt = conn.prepare(
        "SELECT id, label, kind FROM destinations WHERE archived_at IS NULL ORDER BY position, id",
    )?;
    let rows = stmt.query_map([], |row| {
        let kind: String = row.get(2)?;
        Ok(Destination {
            id: row.get(0)?,
            label: row.get(1)?,
            kind: if kind == "responsibility" {
                DestinationKind::Responsibility
            } else {
                DestinationKind::Section
            },
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Nettoie et valide le texte d'une capture.
pub fn normalize_content(content: &str) -> Result<&str, InboxError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(InboxError::EmptyContent);
    }
    if trimmed.chars().count() > MAX_CONTENT_CHARS {
        return Err(InboxError::ContentTooLong);
    }
    Ok(trimmed)
}

/// Enregistre une capture. `now_ms` : horodatage UTC en millisecondes (fourni par
/// l'appelant pour rendre les tests déterministes).
pub fn create_item(
    conn: &Connection,
    content: &str,
    destination_id: Option<&str>,
    now_ms: i64,
) -> Result<InboxItem, InboxError> {
    let content = normalize_content(content)?;
    if let Some(id) = destination_id {
        let active: Option<String> = conn
            .query_row(
                "SELECT id FROM destinations WHERE id = ?1 AND archived_at IS NULL",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        if active.is_none() {
            return Err(InboxError::UnknownDestination);
        }
    }

    let item = InboxItem {
        id: uuid::Uuid::now_v7().to_string(),
        content: content.to_owned(),
        destination_id: destination_id.map(str::to_owned),
        created_at: now_ms,
        updated_at: now_ms,
        deleted_at: None,
    };
    conn.execute(
        "INSERT INTO inbox_items (id, content, destination_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![item.id, item.content, item.destination_id, item.created_at, item.updated_at],
    )?;
    Ok(item)
}

const COLUMNS: &str = "id, content, destination_id, created_at, updated_at, deleted_at";

fn map_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<InboxItem> {
    Ok(InboxItem {
        id: row.get(0)?,
        content: row.get(1)?,
        destination_id: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        deleted_at: row.get(5)?,
    })
}

/// Une page de captures, de la plus récente à la plus ancienne (boîte : par date de
/// création ; corbeille : par date de mise à la corbeille). Pagination par curseur
/// `(date, id)` : insensible aux captures ajoutées pendant le parcours.
pub fn list_items(
    conn: &Connection,
    filter: &InboxFilter,
    scope: Scope,
    before: Option<&Cursor>,
    limit: u32,
) -> Result<InboxPage, InboxError> {
    use rusqlite::types::Value;

    let limit = limit.clamp(1, MAX_LIST_LIMIT);
    // Clauses fixes : les valeurs fournies par l'utilisateur ne passent qu'en paramètres.
    let (scope_condition, sort_column) = match scope {
        Scope::Active => ("deleted_at IS NULL", "created_at"),
        Scope::Trash => ("deleted_at IS NOT NULL", "deleted_at"),
    };
    let mut conditions = vec![scope_condition.to_string()];
    let mut args: Vec<Value> = Vec::new();
    match filter {
        InboxFilter::All => {}
        InboxFilter::Unclassified => conditions.push("destination_id IS NULL".into()),
        InboxFilter::Destination { id } => {
            conditions.push("destination_id = ?".into());
            args.push(Value::Text(id.clone()));
        }
    }

    let total: u32 = conn.query_row(
        &format!("SELECT count(*) FROM inbox_items WHERE {}", conditions.join(" AND ")),
        rusqlite::params_from_iter(args.iter()),
        |row| row.get(0),
    )?;

    if let Some(cursor) = before {
        conditions.push(format!("({sort_column}, id) < (?, ?)"));
        args.push(Value::Integer(cursor.sort_key));
        args.push(Value::Text(cursor.id.clone()));
    }
    // Une ligne de plus que demandé : sert uniquement à savoir s'il en reste.
    let sql = format!(
        "SELECT {COLUMNS} FROM inbox_items WHERE {} ORDER BY {sort_column} DESC, id DESC LIMIT {}",
        conditions.join(" AND "),
        limit + 1
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut items: Vec<InboxItem> = stmt
        .query_map(rusqlite::params_from_iter(args.iter()), map_item)?
        .collect::<Result<_, _>>()?;

    let next_cursor = if items.len() > limit as usize {
        items.truncate(limit as usize);
        items.last().map(|last| Cursor {
            sort_key: match scope {
                Scope::Active => last.created_at,
                Scope::Trash => last.deleted_at.unwrap_or(last.created_at),
            },
            id: last.id.clone(),
        })
    } else {
        None
    };
    Ok(InboxPage { items, total, next_cursor })
}

/// Lit une capture, y compris dans la corbeille.
pub fn get_item(conn: &Connection, id: &str) -> Result<InboxItem, InboxError> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM inbox_items WHERE id = ?1"),
        params![id],
        map_item,
    )
    .optional()?
    .ok_or(InboxError::NotFound)
}

/// Modifie le texte et la destination d'une capture.
///
/// `expected_updated_at` est la version que l'utilisateur a ouverte : l'écriture n'a lieu
/// que si elle est toujours la version enregistrée (verrouillage optimiste, vérifié
/// atomiquement dans le `UPDATE`). Toute modification effective fait croître
/// strictement `updated_at`, même si l'horloge n'a pas avancé. Sans changement réel,
/// rien n'est écrit.
pub fn update_item(
    conn: &Connection,
    id: &str,
    content: &str,
    destination_id: Option<&str>,
    expected_updated_at: i64,
    now_ms: i64,
) -> Result<InboxItem, InboxError> {
    let content = normalize_content(content)?;
    let current = get_item(conn, id)?;
    if current.deleted_at.is_some() {
        return Err(InboxError::Trashed);
    }
    if current.updated_at != expected_updated_at {
        return Err(InboxError::Conflict);
    }
    if current.content == content && current.destination_id.as_deref() == destination_id {
        return Ok(current);
    }
    // Une destination inchangée reste acceptée même si elle a été archivée entre-temps.
    if destination_id != current.destination_id.as_deref() {
        if let Some(destination) = destination_id {
            let active: Option<String> = conn
                .query_row(
                    "SELECT id FROM destinations WHERE id = ?1 AND archived_at IS NULL",
                    params![destination],
                    |row| row.get(0),
                )
                .optional()?;
            if active.is_none() {
                return Err(InboxError::UnknownDestination);
            }
        }
    }

    let new_updated_at = now_ms.max(expected_updated_at + 1);
    let changed = conn.execute(
        "UPDATE inbox_items SET content = ?2, destination_id = ?3, updated_at = ?4
         WHERE id = ?1 AND updated_at = ?5 AND deleted_at IS NULL",
        params![id, content, destination_id, new_updated_at, expected_updated_at],
    )?;
    if changed == 0 {
        // Modifiée ou supprimée entre la lecture et l'écriture : on dit laquelle.
        let latest = get_item(conn, id)?;
        return Err(if latest.deleted_at.is_some() {
            InboxError::Trashed
        } else {
            InboxError::Conflict
        });
    }
    get_item(conn, id)
}

/// Met une capture à la corbeille (suppression logique, aucune donnée détruite).
pub fn trash_item(conn: &Connection, id: &str, now_ms: i64) -> Result<InboxItem, InboxError> {
    let changed = conn.execute(
        "UPDATE inbox_items SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, now_ms],
    )?;
    if changed == 0 {
        get_item(conn, id)?; // introuvable -> NotFound
        return Err(InboxError::Trashed);
    }
    get_item(conn, id)
}

/// Sort une capture de la corbeille : mêmes identifiant, dates, texte et destination.
pub fn restore_item(conn: &Connection, id: &str) -> Result<InboxItem, InboxError> {
    let changed = conn.execute(
        "UPDATE inbox_items SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL",
        params![id],
    )?;
    if changed == 0 {
        get_item(conn, id)?; // introuvable -> NotFound
        return Err(InboxError::NotTrashed);
    }
    get_item(conn, id)
}

#[derive(Debug)]
pub enum InboxError {
    EmptyContent,
    ContentTooLong,
    UnknownDestination,
    /// Capture introuvable.
    NotFound,
    /// Capture déjà dans la corbeille (modification ou nouvelle mise à la corbeille).
    Trashed,
    /// Restauration d'une capture qui n'est pas dans la corbeille.
    NotTrashed,
    /// La capture a été modifiée depuis son ouverture.
    Conflict,
    Storage(rusqlite::Error),
    Unavailable,
}

impl InboxError {
    /// Code stable, utilisable par l'interface.
    pub fn code(&self) -> &'static str {
        match self {
            InboxError::EmptyContent => "empty_content",
            InboxError::ContentTooLong => "content_too_long",
            InboxError::UnknownDestination => "unknown_destination",
            InboxError::NotFound => "not_found",
            InboxError::Trashed => "trashed",
            InboxError::NotTrashed => "not_trashed",
            InboxError::Conflict => "version_conflict",
            InboxError::Storage(_) => "storage",
            InboxError::Unavailable => "unavailable",
        }
    }
}

impl fmt::Display for InboxError {
    /// Message destiné à l'utilisateur : ne reprend jamais le contenu de la capture.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InboxError::EmptyContent => write!(f, "La capture est vide."),
            InboxError::ContentTooLong => {
                write!(f, "La capture dépasse {MAX_CONTENT_CHARS} caractères.")
            }
            InboxError::UnknownDestination => write!(f, "Cette destination n'est pas disponible."),
            InboxError::NotFound => write!(f, "Cette capture n'existe plus."),
            InboxError::Trashed => write!(f, "Cette capture est dans la corbeille."),
            InboxError::NotTrashed => write!(f, "Cette capture n'est pas dans la corbeille."),
            InboxError::Conflict => {
                write!(f, "Cette capture a été modifiée depuis son ouverture.")
            }
            InboxError::Storage(_) => write!(f, "L'enregistrement local a échoué."),
            InboxError::Unavailable => write!(f, "La base de données est indisponible."),
        }
    }
}

impl std::error::Error for InboxError {}

impl From<rusqlite::Error> for InboxError {
    fn from(error: rusqlite::Error) -> Self {
        InboxError::Storage(error)
    }
}

/// Envoyé à l'interface sous la forme `{ "code": "...", "message": "..." }`.
impl Serialize for InboxError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("InboxError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;

    const T0: i64 = 1_760_000_000_000;

    /// Boîte active, première page (signature historique des tests de 001-A).
    fn list_items(conn: &Connection, filter: &InboxFilter, limit: u32) -> Result<InboxPage, InboxError> {
        super::list_items(conn, filter, Scope::Active, None, limit)
    }

    fn all(conn: &Connection) -> InboxPage {
        list_items(conn, &InboxFilter::All, 50).unwrap()
    }

    #[test]
    fn les_cinq_destinations_initiales_sont_disponibles_dans_l_ordre() {
        let conn = storage::open_in_memory();
        let destinations = list_destinations(&conn).unwrap();
        let ids: Vec<&str> = destinations.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["moi", "externe", "administratif", "finances", "inventaire"]);
        assert_eq!(destinations[0].kind, DestinationKind::Responsibility);
        assert_eq!(destinations[1].kind, DestinationKind::Responsibility);
        assert_eq!(destinations[2].kind, DestinationKind::Section);
        assert!(!ids.contains(&"vente"));
    }

    #[test]
    fn une_destination_archivee_n_est_plus_proposee_ni_acceptee() {
        let conn = storage::open_in_memory();
        conn.execute("UPDATE destinations SET archived_at = 1 WHERE id = 'finances'", []).unwrap();
        assert!(!list_destinations(&conn).unwrap().iter().any(|d| d.id == "finances"));
        assert!(matches!(
            create_item(&conn, "Payer", Some("finances"), T0),
            Err(InboxError::UnknownDestination)
        ));
    }

    #[test]
    fn creer_sans_destination_puis_relire() {
        let conn = storage::open_in_memory();
        let item = create_item(&conn, "Vendre ma PlayStation 5", None, T0).unwrap();
        assert_eq!(item.destination_id, None);
        assert_eq!(item.created_at, T0);
        assert_eq!(item.updated_at, T0);
        assert_eq!(all(&conn), InboxPage { items: vec![item], total: 1, next_cursor: None });
    }

    #[test]
    fn creer_avec_destination_un_seul_enregistrement() {
        let conn = storage::open_in_memory();
        create_item(&conn, "Vendre ma PlayStation 5", Some("inventaire"), T0).unwrap();
        let page = all(&conn);
        assert_eq!(page.total, 1, "jamais de copie");
        assert_eq!(page.items[0].destination_id.as_deref(), Some("inventaire"));
    }

    #[test]
    fn le_texte_est_nettoye_mais_les_retours_a_la_ligne_internes_conserves() {
        let conn = storage::open_in_memory();
        let item = create_item(&conn, "  \n Ligne 1\nLigne 2 \t ", None, T0).unwrap();
        assert_eq!(item.content, "Ligne 1\nLigne 2");
    }

    #[test]
    fn une_capture_vide_ou_d_espaces_est_refusee_et_rien_n_est_cree() {
        let conn = storage::open_in_memory();
        for content in ["", "   ", "\n\t \n"] {
            assert!(matches!(create_item(&conn, content, None, T0), Err(InboxError::EmptyContent)));
        }
        assert_eq!(all(&conn).total, 0);
    }

    #[test]
    fn la_longueur_maximale_est_respectee() {
        let conn = storage::open_in_memory();
        let max = "é".repeat(MAX_CONTENT_CHARS);
        assert!(create_item(&conn, &max, None, T0).is_ok());
        let too_long = "é".repeat(MAX_CONTENT_CHARS + 1);
        assert!(matches!(create_item(&conn, &too_long, None, T0), Err(InboxError::ContentTooLong)));
        assert_eq!(all(&conn).total, 1);
    }

    #[test]
    fn une_destination_inconnue_est_refusee() {
        let conn = storage::open_in_memory();
        for id in ["vente", "", "Moi", "moi'; --"] {
            assert!(matches!(
                create_item(&conn, "Texte", Some(id), T0),
                Err(InboxError::UnknownDestination)
            ));
        }
        assert_eq!(all(&conn).total, 0);
    }

    #[test]
    fn le_texte_dangereux_est_stocke_tel_quel_sans_execution() {
        let conn = storage::open_in_memory();
        let dangerous = "'; DROP TABLE inbox_items; -- l'été « déjà » <script>alert(1)</script>";
        create_item(&conn, dangerous, None, T0).unwrap();
        create_item(&conn, "Après", None, T0 + 1).unwrap();
        let page = all(&conn);
        assert_eq!(page.total, 2, "la table existe toujours");
        assert_eq!(page.items[1].content, dangerous);
    }

    #[test]
    fn les_plus_recentes_d_abord_meme_a_la_meme_milliseconde() {
        let conn = storage::open_in_memory();
        create_item(&conn, "Première", None, T0).unwrap();
        create_item(&conn, "Deuxième", None, T0).unwrap();
        create_item(&conn, "Troisième", None, T0 + 5).unwrap();
        let contents: Vec<String> = all(&conn).items.into_iter().map(|i| i.content).collect();
        assert_eq!(contents, ["Troisième", "Deuxième", "Première"]);
    }

    #[test]
    fn les_filtres_retrouvent_les_bonnes_captures() {
        let conn = storage::open_in_memory();
        create_item(&conn, "Sans destination", None, T0).unwrap();
        create_item(&conn, "Déclarer", Some("administratif"), T0 + 1).unwrap();
        create_item(&conn, "Rembourser", Some("finances"), T0 + 2).unwrap();
        create_item(&conn, "Impôts", Some("administratif"), T0 + 3).unwrap();

        let unclassified = list_items(&conn, &InboxFilter::Unclassified, 50).unwrap();
        assert_eq!(unclassified.total, 1);
        assert_eq!(unclassified.items[0].content, "Sans destination");

        let filter = InboxFilter::Destination { id: "administratif".into() };
        let admin = list_items(&conn, &filter, 50).unwrap();
        let contents: Vec<&str> = admin.items.iter().map(|i| i.content.as_str()).collect();
        assert_eq!(contents, ["Impôts", "Déclarer"]);

        // Les captures avec destination restent dans la vue complète.
        assert_eq!(all(&conn).total, 4);
    }

    #[test]
    fn la_limite_est_appliquee_et_le_total_reste_exact() {
        let conn = storage::open_in_memory();
        for i in 0..5 {
            create_item(&conn, &format!("Capture {i}"), None, T0 + i).unwrap();
        }
        let page = list_items(&conn, &InboxFilter::All, 2).unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total, 5);
        // Limite 0 → au moins 1 ; limite énorme → plafonnée.
        assert_eq!(list_items(&conn, &InboxFilter::All, 0).unwrap().items.len(), 1);
        assert_eq!(list_items(&conn, &InboxFilter::All, u32::MAX).unwrap().items.len(), 5);
    }

    #[test]
    fn les_captures_supprimees_logiquement_sont_masquees() {
        let conn = storage::open_in_memory();
        let item = create_item(&conn, "À masquer", None, T0).unwrap();
        conn.execute("UPDATE inbox_items SET deleted_at = 1 WHERE id = ?1", params![item.id]).unwrap();
        assert_eq!(all(&conn).total, 0);
    }

    #[test]
    fn les_captures_survivent_a_la_fermeture_de_la_base() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(storage::DB_FILE_NAME);
        {
            let conn = storage::open(&path).unwrap();
            create_item(&conn, "Persistante", Some("moi"), T0).unwrap();
        }
        let conn = storage::open(&path).unwrap();
        let page = all(&conn);
        assert_eq!(page.items[0].content, "Persistante");
        assert_eq!(page.items[0].destination_id.as_deref(), Some("moi"));
    }

    #[test]
    fn un_echec_d_ecriture_est_signale_sans_rien_creer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(storage::DB_FILE_NAME);
        storage::open(&path).unwrap();
        // Simulation d'un échec SQLite : base ouverte en lecture seule.
        let readonly =
            Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let error = create_item(&readonly, "Texte à préserver", None, T0).unwrap_err();
        assert!(matches!(error, InboxError::Storage(_)));
        assert!(!error.to_string().contains("Texte à préserver"), "le message ne cite pas la capture");
        assert_eq!(all(&storage::open(&path).unwrap()).total, 0);
    }

    #[test]
    fn les_erreurs_sont_transmises_avec_un_code_stable() {
        let json = serde_json::to_value(InboxError::EmptyContent).unwrap();
        assert_eq!(json["code"], "empty_content");
        assert_eq!(json["message"], "La capture est vide.");
    }

    #[test]
    fn le_filtre_est_lu_depuis_le_format_de_l_interface() {
        let parse = |s: &str| serde_json::from_str::<InboxFilter>(s).unwrap();
        assert_eq!(parse(r#"{"type":"all"}"#), InboxFilter::All);
        assert_eq!(parse(r#"{"type":"unclassified"}"#), InboxFilter::Unclassified);
        assert_eq!(
            parse(r#"{"type":"destination","id":"moi"}"#),
            InboxFilter::Destination { id: "moi".into() }
        );
    }

    // --- 001-B : lecture, modification, corbeille, pagination ---

    fn page(conn: &Connection, filter: &InboxFilter, scope: Scope, before: Option<&Cursor>, limit: u32) -> InboxPage {
        super::list_items(conn, filter, scope, before, limit).unwrap()
    }

    fn contents(page: &InboxPage) -> Vec<&str> {
        page.items.iter().map(|i| i.content.as_str()).collect()
    }

    #[test]
    fn lire_une_capture_par_identifiant() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Lisible", Some("moi"), T0).unwrap();
        assert_eq!(get_item(&conn, &created.id).unwrap(), created);
        assert!(matches!(get_item(&conn, "inconnu"), Err(InboxError::NotFound)));
    }

    #[test]
    fn modifier_texte_et_destination_met_a_jour_updated_at_seulement() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Avant", None, T0).unwrap();
        let updated =
            update_item(&conn, &created.id, "  Après  ", Some("finances"), created.updated_at, T0 + 500).unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.content, "Après");
        assert_eq!(updated.destination_id.as_deref(), Some("finances"));
        assert_eq!(updated.created_at, T0, "création inchangée");
        assert_eq!(updated.updated_at, T0 + 500);
        assert_eq!(get_item(&conn, &created.id).unwrap(), updated);
        assert_eq!(all(&conn).total, 1, "jamais de copie");
    }

    #[test]
    fn retirer_la_destination_est_possible() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Classée", Some("moi"), T0).unwrap();
        let updated = update_item(&conn, &created.id, "Classée", None, created.updated_at, T0 + 1).unwrap();
        assert_eq!(updated.destination_id, None);
    }

    #[test]
    fn sans_changement_reel_rien_n_est_ecrit() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Identique", Some("moi"), T0).unwrap();
        let same = update_item(&conn, &created.id, " Identique ", Some("moi"), created.updated_at, T0 + 9_999).unwrap();
        assert_eq!(same.updated_at, T0, "updated_at ne bouge pas");
    }

    #[test]
    fn updated_at_croit_strictement_meme_si_l_horloge_n_avance_pas() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "v0", None, T0).unwrap();
        // Horloge figée, et même en retard sur la dernière modification.
        let v1 = update_item(&conn, &created.id, "v1", None, created.updated_at, T0).unwrap();
        assert_eq!(v1.updated_at, T0 + 1);
        let v2 = update_item(&conn, &created.id, "v2", None, v1.updated_at, T0 - 5_000).unwrap();
        assert_eq!(v2.updated_at, T0 + 2);
        let v3 = update_item(&conn, &created.id, "v3", None, v2.updated_at, T0).unwrap();
        assert_eq!(v3.updated_at, T0 + 3);
    }

    #[test]
    fn deux_modifications_a_la_meme_milliseconde_donnent_un_conflit_pour_la_seconde() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Base", None, T0).unwrap();
        // Deux fenêtres ont ouvert la même version, puis enregistrent « en même temps ».
        let first = update_item(&conn, &created.id, "Première", None, created.updated_at, T0 + 10).unwrap();
        let second = update_item(&conn, &created.id, "Seconde", None, created.updated_at, T0 + 10);
        assert!(matches!(second, Err(InboxError::Conflict)));
        assert_eq!(get_item(&conn, &created.id).unwrap(), first, "la première est conservée");
    }

    #[test]
    fn la_garde_du_update_est_atomique_dans_la_requete_sql() {
        // Simule une écriture concurrente arrivée entre la lecture et l'UPDATE : le
        // UPDATE conditionnel ne doit modifier aucune ligne.
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Base", None, T0).unwrap();
        conn.execute("UPDATE inbox_items SET content = 'Ailleurs', updated_at = ?2 WHERE id = ?1",
            params![created.id, T0 + 7]).unwrap();
        let changed = conn
            .execute(
                "UPDATE inbox_items SET content = 'Écrasement' WHERE id = ?1 AND updated_at = ?2 AND deleted_at IS NULL",
                params![created.id, created.updated_at],
            )
            .unwrap();
        assert_eq!(changed, 0);
        assert_eq!(get_item(&conn, &created.id).unwrap().content, "Ailleurs");
    }

    #[test]
    fn modifier_distingue_introuvable_corbeille_et_conflit() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Cible", None, T0).unwrap();
        assert!(matches!(
            update_item(&conn, "inconnu", "x", None, 0, T0),
            Err(InboxError::NotFound)
        ));
        assert!(matches!(
            update_item(&conn, &created.id, "x", None, created.updated_at - 1, T0),
            Err(InboxError::Conflict)
        ));
        trash_item(&conn, &created.id, T0 + 1).unwrap();
        assert!(matches!(
            update_item(&conn, &created.id, "x", None, created.updated_at, T0 + 2),
            Err(InboxError::Trashed)
        ));
        assert_eq!(get_item(&conn, &created.id).unwrap().content, "Cible", "rien n'a changé");
    }

    #[test]
    fn modifier_refuse_texte_vide_trop_long_et_destination_inconnue_sans_rien_changer() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Intacte", Some("moi"), T0).unwrap();
        let version = created.updated_at;
        assert!(matches!(update_item(&conn, &created.id, "  ", Some("moi"), version, T0 + 1), Err(InboxError::EmptyContent)));
        let long = "x".repeat(MAX_CONTENT_CHARS + 1);
        assert!(matches!(update_item(&conn, &created.id, &long, Some("moi"), version, T0 + 1), Err(InboxError::ContentTooLong)));
        assert!(matches!(update_item(&conn, &created.id, "Ok", Some("vente"), version, T0 + 1), Err(InboxError::UnknownDestination)));
        assert_eq!(get_item(&conn, &created.id).unwrap(), created);
    }

    #[test]
    fn une_destination_archivee_deja_attribuee_reste_acceptee_si_inchangee() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Texte", Some("finances"), T0).unwrap();
        conn.execute("UPDATE destinations SET archived_at = 1 WHERE id = 'finances'", []).unwrap();
        let updated =
            update_item(&conn, &created.id, "Texte corrigé", Some("finances"), created.updated_at, T0 + 1).unwrap();
        assert_eq!(updated.destination_id.as_deref(), Some("finances"));
        // En revanche, on ne peut pas la choisir pour une autre capture.
        let other = create_item(&conn, "Autre", None, T0).unwrap();
        assert!(matches!(
            update_item(&conn, &other.id, "Autre", Some("finances"), other.updated_at, T0 + 1),
            Err(InboxError::UnknownDestination)
        ));
    }

    #[test]
    fn mettre_a_la_corbeille_masque_la_capture_sans_rien_detruire() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "À jeter", Some("externe"), T0).unwrap();
        let trashed = trash_item(&conn, &created.id, T0 + 50).unwrap();
        assert_eq!(trashed.deleted_at, Some(T0 + 50));
        assert_eq!(trashed.content, "À jeter");
        assert_eq!(trashed.updated_at, created.updated_at, "la corbeille ne compte pas comme modification");
        assert_eq!(all(&conn).total, 0);
        let in_trash = page(&conn, &InboxFilter::All, Scope::Trash, None, 50);
        assert_eq!(contents(&in_trash), ["À jeter"]);
        assert_eq!(in_trash.total, 1);
    }

    #[test]
    fn restaurer_remet_exactement_la_capture_d_origine() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "À garder", Some("administratif"), T0).unwrap();
        let edited = update_item(&conn, &created.id, "À garder, modifiée", Some("administratif"), created.updated_at, T0 + 5).unwrap();
        trash_item(&conn, &created.id, T0 + 100).unwrap();
        let restored = restore_item(&conn, &created.id).unwrap();
        assert_eq!(restored, edited, "identifiant, texte, destination et dates identiques");
        assert_eq!(restored.deleted_at, None);
        assert_eq!(contents(&all(&conn)), ["À garder, modifiée"]);
        assert_eq!(page(&conn, &InboxFilter::All, Scope::Trash, None, 50).total, 0);
    }

    #[test]
    fn corbeille_et_restauration_signalent_les_etats_impossibles() {
        let conn = storage::open_in_memory();
        let created = create_item(&conn, "Etat", None, T0).unwrap();
        assert!(matches!(restore_item(&conn, &created.id), Err(InboxError::NotTrashed)));
        trash_item(&conn, &created.id, T0 + 1).unwrap();
        assert!(matches!(trash_item(&conn, &created.id, T0 + 2), Err(InboxError::Trashed)));
        assert!(matches!(trash_item(&conn, "inconnu", T0), Err(InboxError::NotFound)));
        assert!(matches!(restore_item(&conn, "inconnu"), Err(InboxError::NotFound)));
        // La seconde mise à la corbeille n'a pas déplacé la date de la première.
        assert_eq!(get_item(&conn, &created.id).unwrap().deleted_at, Some(T0 + 1));
    }

    #[test]
    fn la_corbeille_est_triee_de_la_plus_recemment_supprimee_a_la_plus_ancienne() {
        let conn = storage::open_in_memory();
        let a = create_item(&conn, "A", None, T0).unwrap();
        let b = create_item(&conn, "B", None, T0 + 1).unwrap();
        let c = create_item(&conn, "C", None, T0 + 2).unwrap();
        trash_item(&conn, &b.id, T0 + 10).unwrap();
        trash_item(&conn, &c.id, T0 + 20).unwrap();
        trash_item(&conn, &a.id, T0 + 30).unwrap();
        let trash = page(&conn, &InboxFilter::All, Scope::Trash, None, 50);
        assert_eq!(contents(&trash), ["A", "C", "B"]);
    }

    #[test]
    fn la_pagination_parcourt_tout_sans_doublon_ni_omission_meme_a_dates_egales() {
        let conn = storage::open_in_memory();
        for i in 0..23 {
            // Dates volontairement égales par groupes de 5 : le curseur doit départager par id.
            create_item(&conn, &format!("N{i:02}"), None, T0 + (i / 5)).unwrap();
        }
        let mut seen: Vec<String> = Vec::new();
        let mut cursor: Option<Cursor> = None;
        let mut pages = 0;
        loop {
            let p = page(&conn, &InboxFilter::All, Scope::Active, cursor.as_ref(), 10);
            assert_eq!(p.total, 23, "le total ne dépend pas du curseur");
            seen.extend(p.items.iter().map(|i| i.id.clone()));
            pages += 1;
            match p.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(pages, 3);
        assert_eq!(seen.len(), 23);
        let unique: std::collections::HashSet<_> = seen.iter().collect();
        assert_eq!(unique.len(), 23, "aucun doublon");
    }

    #[test]
    fn des_captures_ajoutees_pendant_la_pagination_ne_creent_ni_doublon_ni_trou() {
        let conn = storage::open_in_memory();
        for i in 0..12 {
            create_item(&conn, &format!("Ancienne {i:02}"), None, T0 + i).unwrap();
        }
        let first = page(&conn, &InboxFilter::All, Scope::Active, None, 5);
        let cursor = first.next_cursor.clone().expect("il en reste");
        // De nouvelles captures arrivent entre deux chargements.
        for i in 0..4 {
            create_item(&conn, &format!("Nouvelle {i}"), None, T0 + 1_000 + i).unwrap();
        }
        let second = page(&conn, &InboxFilter::All, Scope::Active, Some(&cursor), 5);
        let third = page(&conn, &InboxFilter::All, Scope::Active, second.next_cursor.as_ref(), 5);
        let mut parcourues: Vec<&str> = Vec::new();
        parcourues.extend(contents(&first));
        parcourues.extend(contents(&second));
        parcourues.extend(contents(&third));
        let attendues: Vec<String> = (0..12).rev().map(|i| format!("Ancienne {i:02}")).collect();
        assert_eq!(parcourues, attendues, "exactement les 12 anciennes, dans l'ordre");
        assert!(third.next_cursor.is_none());
        assert_eq!(third.total, 16, "le total reflète les ajouts");
    }

    #[test]
    fn la_pagination_respecte_les_filtres() {
        let conn = storage::open_in_memory();
        for i in 0..7 {
            create_item(&conn, &format!("F{i}"), Some("finances"), T0 + i).unwrap();
            create_item(&conn, &format!("M{i}"), Some("moi"), T0 + i).unwrap();
        }
        let filter = InboxFilter::Destination { id: "finances".into() };
        let first = page(&conn, &filter, Scope::Active, None, 3);
        assert_eq!(contents(&first), ["F6", "F5", "F4"]);
        assert_eq!(first.total, 7);
        let second = page(&conn, &filter, Scope::Active, first.next_cursor.as_ref(), 3);
        assert_eq!(contents(&second), ["F3", "F2", "F1"]);
        let third = page(&conn, &filter, Scope::Active, second.next_cursor.as_ref(), 3);
        assert_eq!(contents(&third), ["F0"]);
        assert!(third.next_cursor.is_none());
    }

    #[test]
    fn la_pagination_de_la_corbeille_utilise_la_date_de_suppression() {
        let conn = storage::open_in_memory();
        let ids: Vec<String> = (0..6)
            .map(|i| create_item(&conn, &format!("T{i}"), None, T0 + i).unwrap().id)
            .collect();
        // Supprimées dans un ordre différent de l'ordre de création.
        for (rank, index) in [3usize, 0, 5, 1, 4, 2].iter().enumerate() {
            trash_item(&conn, &ids[*index], T0 + 100 + rank as i64).unwrap();
        }
        let first = page(&conn, &InboxFilter::All, Scope::Trash, None, 4);
        assert_eq!(contents(&first), ["T2", "T4", "T1", "T5"]);
        let second = page(&conn, &InboxFilter::All, Scope::Trash, first.next_cursor.as_ref(), 4);
        assert_eq!(contents(&second), ["T0", "T3"]);
        assert!(second.next_cursor.is_none());
    }

    #[test]
    fn modification_et_corbeille_survivent_a_la_fermeture_de_la_base() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(storage::DB_FILE_NAME);
        let (kept, trashed) = {
            let conn = storage::open(&path).unwrap();
            let a = create_item(&conn, "Gardée", None, T0).unwrap();
            let b = create_item(&conn, "Supprimée", Some("moi"), T0 + 1).unwrap();
            let a = update_item(&conn, &a.id, "Gardée, modifiée", Some("finances"), a.updated_at, T0 + 2).unwrap();
            trash_item(&conn, &b.id, T0 + 3).unwrap();
            (a, b)
        };
        let conn = storage::open(&path).unwrap();
        assert_eq!(get_item(&conn, &kept.id).unwrap(), kept);
        assert_eq!(all(&conn).total, 1);
        let restored = restore_item(&conn, &trashed.id).unwrap();
        assert_eq!(restored, trashed, "restaurée telle qu'à l'origine");
    }

    #[test]
    fn un_echec_d_ecriture_est_signale_pour_modifier_corbeille_et_restauration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(storage::DB_FILE_NAME);
        let (kept, trashed) = {
            let conn = storage::open(&path).unwrap();
            let a = create_item(&conn, "Active", None, T0).unwrap();
            let b = create_item(&conn, "Corbeille", None, T0 + 1).unwrap();
            trash_item(&conn, &b.id, T0 + 2).unwrap();
            (a, b)
        };
        let readonly = Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        assert!(matches!(
            update_item(&readonly, &kept.id, "Modifiée", None, kept.updated_at, T0 + 9),
            Err(InboxError::Storage(_))
        ));
        assert!(matches!(trash_item(&readonly, &kept.id, T0 + 9), Err(InboxError::Storage(_))));
        assert!(matches!(restore_item(&readonly, &trashed.id), Err(InboxError::Storage(_))));
        let conn = storage::open(&path).unwrap();
        assert_eq!(get_item(&conn, &kept.id).unwrap(), kept);
        assert!(get_item(&conn, &trashed.id).unwrap().deleted_at.is_some());
    }

    #[test]
    fn les_nouveaux_codes_d_erreur_sont_stables() {
        for (error, code) in [
            (InboxError::NotFound, "not_found"),
            (InboxError::Trashed, "trashed"),
            (InboxError::NotTrashed, "not_trashed"),
            (InboxError::Conflict, "version_conflict"),
        ] {
            assert_eq!(serde_json::to_value(error).unwrap()["code"], code);
        }
    }

    #[test]
    fn le_format_de_page_et_de_curseur_correspond_a_l_interface() {
        let conn = storage::open_in_memory();
        for i in 0..3 {
            create_item(&conn, &format!("P{i}"), None, T0 + i).unwrap();
        }
        let p = page(&conn, &InboxFilter::All, Scope::Active, None, 2);
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["total"], 3);
        assert_eq!(json["items"][0]["deletedAt"], serde_json::Value::Null);
        assert!(json["nextCursor"]["sortKey"].is_i64());
        assert!(json["nextCursor"]["id"].is_string());
        let cursor: Cursor = serde_json::from_value(json["nextCursor"].clone()).unwrap();
        assert_eq!(Some(cursor), p.next_cursor);
    }
}
