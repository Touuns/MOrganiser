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
    pub updated_at: i64,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct InboxPage {
    pub items: Vec<InboxItem>,
    /// Nombre total de captures correspondant au filtre (au-delà de la limite affichée).
    pub total: u32,
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
    };
    conn.execute(
        "INSERT INTO inbox_items (id, content, destination_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![item.id, item.content, item.destination_id, item.created_at, item.updated_at],
    )?;
    Ok(item)
}

/// Captures non supprimées, les plus récentes d'abord.
pub fn list_items(conn: &Connection, filter: &InboxFilter, limit: u32) -> Result<InboxPage, InboxError> {
    let limit = limit.clamp(1, MAX_LIST_LIMIT);
    // Clauses fixes : seule la valeur de l'identifiant passe en paramètre.
    let (condition, destination) = match filter {
        InboxFilter::All => ("1 = 1", None),
        InboxFilter::Unclassified => ("destination_id IS NULL", None),
        InboxFilter::Destination { id } => ("destination_id = ?1", Some(id.as_str())),
    };
    let base = format!("FROM inbox_items WHERE deleted_at IS NULL AND {condition}");

    let total: u32 = match destination {
        Some(id) => conn.query_row(&format!("SELECT count(*) {base}"), params![id], |row| row.get(0))?,
        None => conn.query_row(&format!("SELECT count(*) {base}"), [], |row| row.get(0))?,
    };

    let sql = format!(
        "SELECT id, content, destination_id, created_at, updated_at {base}
         ORDER BY created_at DESC, id DESC LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let map = |row: &rusqlite::Row<'_>| {
        Ok(InboxItem {
            id: row.get(0)?,
            content: row.get(1)?,
            destination_id: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    };
    let items = match destination {
        Some(id) => stmt.query_map(params![id], map)?.collect::<Result<_, _>>()?,
        None => stmt.query_map([], map)?.collect::<Result<_, _>>()?,
    };
    Ok(InboxPage { items, total })
}

#[derive(Debug)]
pub enum InboxError {
    EmptyContent,
    ContentTooLong,
    UnknownDestination,
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
        assert_eq!(all(&conn), InboxPage { items: vec![item], total: 1 });
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
}
