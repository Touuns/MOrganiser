//! Répétition d'une migration (v1 → v2, v2 → v3, ou chaînée v1 → v3) sur une COPIE ISOLÉE d'une base.
//!
//! Les versions de départ et d'arrivée sont **explicites** (`MORGANISER_REHEARSAL_FROM` et
//! `MORGANISER_REHEARSAL_TO`) : seules les combinaisons de [`SUPPORTED_PLANS`] sont admises, la
//! version réelle de la copie doit être EXACTEMENT celle demandée, et toute autre situation est un
//! échec explicite (jamais une réussite silencieuse).
//!
//! Deux usages :
//! - tests ordinaires : le dispositif et ses garde-fous sont vérifiés sur des bases **fictives**
//!   créées à la volée, dans des dossiers temporaires ;
//! - test `#[ignore]` : le propriétaire fournit, par variable d'environnement, le chemin d'une
//!   copie qu'il a faite lui-même (application fermée, `.db` + `-wal` + `-shm` ensemble) :
//!
//!   ```text
//!   $env:MORGANISER_REHEARSAL_DB   = "C:\MOrganiserEssais\dev-AAAA-MM-JJ\morganiser.db"
//!   $env:MORGANISER_REHEARSAL_FROM = "2"
//!   $env:MORGANISER_REHEARSAL_TO   = "3"
//!   cargo test --lib repetition_sur_copie_isolee -- --ignored --nocapture
//!   ```
//!
//!   Le filtre `repetition_sur_copie_isolee` est obligatoire : sans lui, `--ignored` lancerait
//!   aussi les autres tests ignorés (sous-processus qui attendent d'être tués).
//!
//! # Garde-fous
//! - Variable absente, vide ou composée d'espaces : **échec explicite** (jamais une réussite).
//! - Chemin : absolu, sans `..`, nommé `morganiser.db`, fichier ordinaire (pas un lien).
//!   Refusé s'il se trouve dans `%LOCALAPPDATA%` / `%APPDATA%`, dans le dossier de données
//!   Dev ou Stable (calculés avec `environment::data_dir`), ou s'il contient un composant
//!   `com.morganiser*` (préfixe tiré de `environment::STABLE_IDENTIFIER`). Ces contrôles
//!   portent sur le chemin saisi ET sur le chemin canonique ; les comparaisons sont faites par
//!   composants, sans tenir compte de la casse, et préfixe verbatim (`\\?\`) retiré.
//!   Un chemin dont la forme canonique diffère de la forme saisie (lien symbolique, jonction,
//!   nom abrégé 8.3) est refusé : il faut indiquer le chemin réel.
//! - Si une racine protégée ne peut pas être résolue, la vérification échoue (refus par sécurité).
//! - `-wal` et `-shm`, lorsqu'ils existent, subissent les mêmes contrôles (pas de lien, même
//!   dossier réel, hors zones protégées).
//! - Aucun fichier n'est créé ni ouvert par SQLite avant la fin de la validation ; la source
//!   n'est JAMAIS ouverte par SQLite (lecture binaire seule pour la copie et le SHA-256).
//! - SHA-256 de chaque fichier source (présence, taille, date, empreinte) : avant la copie,
//!   après la copie, après l'opération, même en cas d'erreur. La copie temporaire est comparée
//!   à la source avant toute ouverture SQLite.
//! - Le rapport n'affiche ni texte, ni chemin, ni empreinte : seulement des comptes et
//!   `OK` / `DIFFÉRENT` / un message d'échec.
//!
//! # Limites du nettoyage du dossier de travail
//! Le dossier de travail (copie des données + sauvegardes + « mis de côté ») est un
//! `tempfile::TempDir` préfixé `morganiser-rehearsal-` :
//! - **réussite** : supprimé à la fin de la fonction (les connexions SQLite sont fermées avant,
//!   car Windows ne supprime pas un fichier ouvert) ;
//! - **erreur normale** (`Err` renvoyé) : supprimé de la même façon ;
//! - **panique** : le test est compilé en mode « unwind » (le réglage `panic = "abort"` du
//!   profil release ne s'applique pas aux tests), donc `Drop` s'exécute pendant le déroulement ;
//! - **arrêt brutal** (`taskkill /F`, fermeture du terminal, coupure de courant, certains
//!   `Ctrl+C`) : **aucun nettoyage**, le dossier et les données copiées restent dans `%TEMP%` ;
//!   le test `un_arret_brutal_laisse_le_dossier_de_travail` le démontre ;
//! - **suppression partielle** : `TempDir` ignore les erreurs de suppression (fichier tenu par un
//!   antivirus ou un indexeur) : un reliquat est possible même en cas de réussite.
//!
//! Nettoyage ciblé recommandé : lister puis supprimer uniquement `morganiser-rehearsal-*`
//! (voir la procédure PowerShell de la fiche 002), jamais le contenu de `%TEMP%` en bloc.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};
use std::time::SystemTime;

use rusqlite::types::Value;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use super::*;
use crate::environment::{data_dir, Channel, DEV_IDENTIFIER, STABLE_IDENTIFIER};
use crate::{inbox, tasks};

const ENV_VAR: &str = "MORGANISER_REHEARSAL_DB";
/// Version de départ attendue de la copie (explicite, sans déduction).
const ENV_FROM: &str = "MORGANISER_REHEARSAL_FROM";
/// Version d'arrivée de la répétition (explicite, sans déduction).
const ENV_TO: &str = "MORGANISER_REHEARSAL_TO";
/// Préfixe du dossier de travail : permet un nettoyage ciblé.
const WORK_PREFIX: &str = "morganiser-rehearsal-";
/// Fichiers qui forment ensemble la base (`""` = `morganiser.db`).
const SUFFIXES: [&str; 3] = ["", "-wal", "-shm"];


// ---------------------------------------------------------------------------------------------
// Protection des chemins
// ---------------------------------------------------------------------------------------------

/// Composants comparables : minuscules, sans préfixe verbatim, sans séparateurs.
fn normalized(path: &Path) -> Vec<String> {
    let lower = |s: &std::ffi::OsStr| s.to_string_lossy().to_lowercase();
    path.components()
        .filter_map(|component| match component {
            Component::Prefix(prefix) => Some(match prefix.kind() {
                Prefix::VerbatimDisk(letter) | Prefix::Disk(letter) => {
                    format!("{}:", (letter as char).to_ascii_lowercase())
                }
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                    format!("\\\\{}\\{}", lower(server), lower(share))
                }
                Prefix::Verbatim(name) | Prefix::DeviceNS(name) => lower(name),
            }),
            Component::RootDir | Component::CurDir => None,
            Component::ParentDir => Some("..".to_string()),
            Component::Normal(name) => Some(lower(name)),
        })
        .collect()
}

fn starts_with(path: &[String], prefix: &[String]) -> bool {
    !prefix.is_empty() && path.len() >= prefix.len() && path[..prefix.len()] == *prefix
}

/// Préfixe commun aux identifiants Dev et Stable (`com.morganiser`), tiré d'`environment.rs`.
fn family_prefix() -> String {
    let (family, _) = STABLE_IDENTIFIER.rsplit_once('.').unwrap_or((STABLE_IDENTIFIER, ""));
    debug_assert!(DEV_IDENTIFIER.starts_with(family));
    family.to_lowercase()
}

/// Zones où une copie d'essai n'a pas le droit de se trouver.
struct Protected {
    /// (libellé sans chemin personnel, composants normalisés), forme saisie ET canonique.
    roots: Vec<(String, Vec<String>)>,
    /// Dossiers de données Dev et Stable de chaque racine, via `environment::data_dir`.
    data_dirs: Vec<(String, Vec<String>)>,
}

impl Protected {
    /// Échoue si une racine n'est pas absolue ou ne peut pas être résolue : refus par sécurité.
    fn from_roots(roots: &[(String, PathBuf)]) -> Result<Self, String> {
        if roots.is_empty() {
            return Err("aucune zone protégée définie : vérification impossible".into());
        }
        let mut protected = Protected { roots: Vec::new(), data_dirs: Vec::new() };
        for (label, root) in roots {
            if !root.is_absolute() {
                return Err(format!("{label} n'est pas un chemin absolu : vérification impossible"));
            }
            let canonical = fs::canonicalize(root)
                .map_err(|_| format!("{label} : zone protégée non résolue, vérification impossible"))?;
            for form in [root.as_path(), canonical.as_path()] {
                protected.roots.push((label.clone(), normalized(form)));
                for channel in [Channel::Dev, Channel::Stable] {
                    protected
                        .data_dirs
                        .push((format!("dossier de données {channel:?}"), normalized(&data_dir(form, channel))));
                }
            }
        }
        Ok(protected)
    }

    /// `lookup` donne la valeur d'une variable d'environnement (injectable pour les tests).
    fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self, String> {
        let mut roots = Vec::new();
        for name in ["LOCALAPPDATA", "APPDATA"] {
            let value = lookup(name)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("%{name}% non définie : protection impossible à vérifier"))?;
            roots.push((format!("%{name}%"), PathBuf::from(value)));
        }
        Self::from_roots(&roots)
    }

    fn from_env() -> Result<Self, String> {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// Raison du refus si `path` est dans une zone protégée (jamais le chemin lui-même).
    fn refuses(&self, path: &Path) -> Option<String> {
        let components = normalized(path);
        for (label, dir) in &self.data_dirs {
            if starts_with(&components, dir) {
                return Some(format!("chemin dans le {label}"));
            }
        }
        for (label, root) in &self.roots {
            if starts_with(&components, root) {
                return Some(format!("chemin sous {label}"));
            }
        }
        let family = family_prefix();
        if components.iter().any(|c| c.starts_with(&family)) {
            return Some(format!("chemin contenant un composant « {family}* »"));
        }
        None
    }
}

fn sibling(db: &Path, suffix: &str) -> PathBuf {
    let mut name = db.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn file_label(suffix: &str) -> &str {
    if suffix.is_empty() {
        ".db"
    } else {
        suffix
    }
}

/// Valide le chemin de la copie fournie. Ne crée rien et n'ouvre aucune base.
fn validate_source(path: &Path, protected: &Protected) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("le chemin doit être absolu".into());
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("le chemin ne doit pas contenir « .. »".into());
    }
    let name_ok = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(DB_FILE_NAME));
    if !name_ok {
        return Err(format!("le fichier doit s'appeler {DB_FILE_NAME}"));
    }
    // Chemin saisi : avant toute lecture du disque.
    if let Some(reason) = protected.refuses(path) {
        return Err(format!("{reason} (chemin saisi)"));
    }

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Err("fichier introuvable".into()),
        Err(_) => return Err("fichier inaccessible : refus par sécurité".into()),
    };
    if metadata.file_type().is_symlink() {
        return Err("le fichier est un lien symbolique : indiquez le fichier réel".into());
    }
    if !metadata.is_file() {
        return Err("le chemin n'est pas un fichier ordinaire".into());
    }

    let canonical =
        fs::canonicalize(path).map_err(|_| "chemin non résolu : refus par sécurité".to_string())?;
    if let Some(reason) = protected.refuses(&canonical) {
        return Err(format!("{reason} (chemin canonique)"));
    }
    if normalized(path) != normalized(&canonical) {
        return Err(
            "le chemin traverse un lien symbolique, une jonction ou un nom abrégé : \
             indiquez le chemin réel"
                .into(),
        );
    }

    // Fichiers associés : ni lien, ni dossier, même dossier réel, hors zones protégées.
    let canonical_name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .ok_or_else(|| "nom de fichier non résolu".to_string())?;
    for suffix in ["-wal", "-shm"] {
        let companion = sibling(path, suffix);
        let metadata = match fs::symlink_metadata(&companion) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(format!("fichier {suffix} inaccessible : refus par sécurité")),
        };
        if metadata.file_type().is_symlink() {
            return Err(format!("{suffix} est un lien symbolique : refus"));
        }
        if !metadata.is_file() {
            return Err(format!("{suffix} n'est pas un fichier ordinaire"));
        }
        let resolved = fs::canonicalize(&companion)
            .map_err(|_| format!("{suffix} : chemin non résolu, refus par sécurité"))?;
        if let Some(reason) = protected.refuses(&resolved) {
            return Err(format!("{suffix} : {reason}"));
        }
        let expected = format!("{canonical_name}{suffix}");
        let same_place = resolved.parent().map(normalized) == canonical.parent().map(normalized)
            && resolved
                .file_name()
                .is_some_and(|n| n.to_string_lossy().to_lowercase() == expected);
        if !same_place {
            return Err(format!("{suffix} ne désigne pas le fichier attendu à côté de la base : refus"));
        }
    }
    Ok(())
}

/// Valeur de la variable d'environnement → chemin validé. N'ouvre ni ne crée aucune base.
fn resolve_source(raw: Option<OsString>, protected: &Protected) -> Result<PathBuf, String> {
    let raw = raw.ok_or_else(|| {
        format!("{ENV_VAR} absente : définissez-la avec le chemin de la copie isolée (morganiser.db)")
    })?;
    if raw.to_string_lossy().trim().is_empty() {
        return Err(format!("{ENV_VAR} est vide ou composée d'espaces"));
    }
    let path = PathBuf::from(raw);
    validate_source(&path, protected)?;
    Ok(path)
}

// ---------------------------------------------------------------------------------------------
// Empreintes des fichiers source (SHA-256) — distinctes des empreintes logiques SQLite plus bas
// ---------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Clone)]
struct FileIdentity {
    len: u64,
    modified: Option<SystemTime>,
    sha256: [u8; 32],
}

/// État des trois fichiers : présence, taille, date, SHA-256.
#[derive(Debug, PartialEq, Eq)]
struct SourceSnapshot {
    files: Vec<(&'static str, Option<FileIdentity>)>,
}

fn sha256_of(path: &Path) -> Result<[u8; 32], String> {
    use io::Read;
    // Lecture binaire seule : jamais ouvert par SQLite, jamais en écriture.
    let mut file = fs::File::open(path).map_err(|e| format!("lecture impossible : {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| format!("lecture impossible : {e}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

impl SourceSnapshot {
    fn capture(db: &Path) -> Result<Self, String> {
        let mut files = Vec::new();
        for suffix in SUFFIXES {
            let path = sibling(db, suffix);
            let identity = match fs::symlink_metadata(&path) {
                Ok(metadata) => {
                    if !metadata.is_file() {
                        return Err(format!("{} n'est pas un fichier ordinaire", file_label(suffix)));
                    }
                    Some(FileIdentity {
                        len: metadata.len(),
                        modified: metadata.modified().ok(),
                        sha256: sha256_of(&path).map_err(|e| format!("{} : {e}", file_label(suffix)))?,
                    })
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(format!("{} : {error}", file_label(suffix))),
            };
            files.push((suffix, identity));
        }
        Ok(SourceSnapshot { files })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum SourceCheck {
    Unchanged,
    Different,
    Unverifiable(String),
    /// La source n'a pas pu être lue au départ : rien n'a été fait.
    NotChecked,
}

impl SourceCheck {
    fn describe(&self) -> String {
        match self {
            SourceCheck::Unchanged => "OK".into(),
            SourceCheck::Different => "DIFFÉRENT".into(),
            SourceCheck::Unverifiable(reason) => format!("NON VÉRIFIABLE ({reason})"),
            SourceCheck::NotChecked => "NON VÉRIFIÉE (opération non démarrée)".into(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Plans pris en charge : versions de départ et d'arrivée EXPLICITES
// ---------------------------------------------------------------------------------------------

/// Étape de migration répétée. Les versions ne sont jamais déduites : elles sont fournies par
/// l'appelant (variables d'environnement pour l'usage réel) et validées contre cette liste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Plan {
    from: u32,
    to: u32,
}

const PLAN_V1_V2: Plan = Plan { from: 1, to: 2 };
const PLAN_V2_V3: Plan = Plan { from: 2, to: 3 };
const PLAN_V1_V3: Plan = Plan { from: 1, to: 3 };
/// Seules combinaisons répétables : toute autre est refusée avant la moindre création de fichier.
const SUPPORTED_PLANS: [Plan; 3] = [PLAN_V1_V2, PLAN_V2_V3, PLAN_V1_V3];

impl Plan {
    fn new(from: u32, to: u32) -> Result<Plan, String> {
        let plan = Plan { from, to };
        if SUPPORTED_PLANS.contains(&plan) {
            Ok(plan)
        } else {
            Err(format!(
                "combinaison de versions non prise en charge : v{from} -> v{to} (admises : v1 -> v2, v2 -> v3, v1 -> v3)"
            ))
        }
    }

    /// Lit une version dans une variable d'environnement : absente, vide ou non numérique = refus.
    fn read_version(name: &str, raw: Option<OsString>) -> Result<u32, String> {
        let raw = raw.ok_or_else(|| format!("{name} absente : indiquez explicitement la version (1, 2 ou 3)"))?;
        let text = raw.to_string_lossy();
        text.trim()
            .parse::<u32>()
            .map_err(|_| format!("{name} doit être un numéro de version (1, 2 ou 3)"))
    }

    fn from_values(from: Option<OsString>, to: Option<OsString>) -> Result<Plan, String> {
        Plan::new(Plan::read_version(ENV_FROM, from)?, Plan::read_version(ENV_TO, to)?)
    }

    /// Migrations appliquées par ce plan (jusqu'à la version d'arrivée incluse).
    fn migrations(self) -> &'static [&'static str] {
        &MIGRATIONS[..self.to as usize]
    }
}

// ---------------------------------------------------------------------------------------------
// Instantané structurel et projection attendue (indépendante du SQL de migration)
// ---------------------------------------------------------------------------------------------

type Row = Vec<Value>;

const DESTINATION_COLUMNS: &[&str] = &["id", "label", "kind", "position", "archived_at"];
const CAPTURE_COLUMNS: &[&str] =
    &["id", "content", "destination_id", "created_at", "updated_at", "deleted_at"];
const LINK_COLUMNS: &[&str] = &["id", "converted_at", "converted_task_id"];
const TASK_COLUMNS_V2: &[&str] = &[
    "id",
    "title",
    "details",
    "status",
    "destination_id",
    "origin_inbox_item_id",
    "created_at",
    "updated_at",
    "deleted_at",
];
const TASK_COLUMNS_V3: &[&str] = &[
    "id",
    "title",
    "details",
    "status",
    "responsibility_id",
    "section_id",
    "start_date",
    "due_date",
    "completed_on",
    "priority",
    "origin_inbox_item_id",
    "created_at",
    "updated_at",
    "deleted_at",
];

/// État logique d'une base à une version donnée : valeurs réelles (jamais un résumé), conservées
/// en mémoire seulement. Rien n'est affiché : les rapports ne citent que `table#id.champ`.
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    version: u32,
    destinations: Vec<Row>,
    captures: Vec<Row>,
    /// Liens capture → tâche (v2 et au-delà) : `id`, `converted_at`, `converted_task_id`.
    links: Vec<Row>,
    tasks: Vec<Row>,
}

fn rows(conn: &Connection, sql: &str) -> Result<Vec<Row>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let columns = stmt.column_count();
    let mapped = stmt
        .query_map([], |row| (0..columns).map(|i| row.get::<_, Value>(i)).collect::<Result<Row, _>>())
        .map_err(|e| e.to_string())?;
    mapped.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

fn read_snapshot(conn: &Connection, version: u32) -> Result<Snapshot, String> {
    let destinations = rows(
        conn,
        "SELECT id, label, kind, position, archived_at FROM destinations ORDER BY id",
    )?;
    let captures = rows(
        conn,
        "SELECT id, content, destination_id, created_at, updated_at, deleted_at
         FROM inbox_items ORDER BY id",
    )?;
    let links = if version >= 2 {
        rows(conn, "SELECT id, converted_at, converted_task_id FROM inbox_items ORDER BY id")?
    } else {
        Vec::new()
    };
    let tasks = match version {
        1 => Vec::new(),
        2 => rows(
            conn,
            "SELECT id, title, details, status, destination_id, origin_inbox_item_id,
                    created_at, updated_at, deleted_at FROM tasks ORDER BY id",
        )?,
        _ => rows(
            conn,
            "SELECT id, title, details, status, responsibility_id, section_id, start_date, due_date,
                    completed_on, priority, origin_inbox_item_id, created_at, updated_at, deleted_at
             FROM tasks ORDER BY id",
        )?,
    };
    Ok(Snapshot { version, destinations, captures, links, tasks })
}

/// Projection d'une tâche v2 vers la v3 : l'ancienne destination est répartie selon sa NATURE ;
/// les colonnes nouvelles sont vides. Écrite ICI, sans réutiliser le SQL de la migration.
fn project_task(old: &Row, kinds: &HashMap<String, String>) -> Result<Row, String> {
    let (responsibility, section) = match &old[4] {
        Value::Null => (Value::Null, Value::Null),
        Value::Text(id) => match kinds.get(id).map(String::as_str) {
            Some("responsibility") => (old[4].clone(), Value::Null),
            Some("section") => (Value::Null, old[4].clone()),
            _ => return Err("une tâche référence une destination inconnue ou de nature inattendue".into()),
        },
        _ => return Err("destination de tâche de type inattendu".into()),
    };
    Ok(vec![
        old[0].clone(),
        old[1].clone(),
        old[2].clone(),
        old[3].clone(),
        responsibility,
        section,
        Value::Null, // start_date
        Value::Null, // due_date
        Value::Null, // completed_on
        Value::Null, // priority
        old[5].clone(),
        old[6].clone(),
        old[7].clone(),
        old[8].clone(),
    ])
}

/// État attendu à la version `to` à partir de l'état de départ : calculé indépendamment du SQL.
fn expected_after(before: &Snapshot, to: u32) -> Result<Snapshot, String> {
    let kinds: HashMap<String, String> = before
        .destinations
        .iter()
        .filter_map(|row| match (&row[0], &row[2]) {
            (Value::Text(id), Value::Text(kind)) => Some((id.clone(), kind.clone())),
            _ => None,
        })
        .collect();
    // Les liens existent à partir de la v2 : une capture v1 migrée n'est liée à aucune tâche.
    let links = if to < 2 {
        Vec::new()
    } else if before.version >= 2 {
        before.links.clone()
    } else {
        before.captures.iter().map(|c| vec![c[0].clone(), Value::Null, Value::Null]).collect()
    };
    let tasks = match (before.version, to) {
        (1, _) => Vec::new(),
        (2, 2) => before.tasks.clone(),
        (2, 3) => before.tasks.iter().map(|t| project_task(t, &kinds)).collect::<Result<_, _>>()?,
        (from, to) => return Err(format!("projection non prise en charge : v{from} -> v{to}")),
    };
    Ok(Snapshot {
        version: to,
        destinations: before.destinations.clone(),
        captures: before.captures.clone(),
        links,
        tasks,
    })
}

/// Une différence, décrite SANS contenu : table, identifiant technique, nom du champ.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Divergence {
    table: &'static str,
    id: String,
    field: String,
}

fn row_id(row: &Row) -> String {
    match &row[0] {
        Value::Text(id) => id.clone(),
        other => format!("{other:?}"),
    }
}

fn diff_table(
    table: &'static str,
    columns: &[&str],
    expected: &[Row],
    actual: &[Row],
    out: &mut Vec<Divergence>,
) {
    let by_id = |rows: &[Row]| -> BTreeMap<String, Row> {
        rows.iter().map(|r| (row_id(r), r.clone())).collect()
    };
    let (expected, actual) = (by_id(expected), by_id(actual));
    for (id, want) in &expected {
        match actual.get(id) {
            None => out.push(Divergence { table, id: id.clone(), field: "ligne absente".into() }),
            Some(got) if got.len() != want.len() => {
                out.push(Divergence { table, id: id.clone(), field: "structure".into() })
            }
            Some(got) => {
                for (index, (a, b)) in want.iter().zip(got).enumerate() {
                    if a != b {
                        let field = columns.get(index).copied().unwrap_or("?");
                        out.push(Divergence { table, id: id.clone(), field: field.into() });
                    }
                }
            }
        }
    }
    for id in actual.keys().filter(|id| !expected.contains_key(*id)) {
        out.push(Divergence { table, id: id.clone(), field: "ligne inattendue".into() });
    }
}

/// Compare deux états : liste vide = identiques. Aucune valeur n'est copiée dans le résultat.
fn compare(expected: &Snapshot, actual: &Snapshot) -> Vec<Divergence> {
    let mut out = Vec::new();
    if expected.version != actual.version {
        out.push(Divergence { table: "schema", id: "-".into(), field: "version".into() });
    }
    diff_table("destinations", DESTINATION_COLUMNS, &expected.destinations, &actual.destinations, &mut out);
    diff_table("inbox_items", CAPTURE_COLUMNS, &expected.captures, &actual.captures, &mut out);
    diff_table("inbox_items.liens", LINK_COLUMNS, &expected.links, &actual.links, &mut out);
    let task_columns = if expected.version >= 3 { TASK_COLUMNS_V3 } else { TASK_COLUMNS_V2 };
    diff_table("tasks", task_columns, &expected.tasks, &actual.tasks, &mut out);
    out
}

/// Message d'échec : nombre et premières divergences (`table#id.champ`), jamais de contenu.
fn describe(divergences: &[Divergence]) -> String {
    let shown: Vec<String> = divergences
        .iter()
        .take(5)
        .map(|d| format!("{}#{}.{}", d.table, d.id, d.field))
        .collect();
    format!("{} divergence(s) : {}", divergences.len(), shown.join(", "))
}

// ---------------------------------------------------------------------------------------------
// Contrôles d'intégrité et de schéma
// ---------------------------------------------------------------------------------------------

/// `integrity_check` = ok et aucune violation de clé étrangère.
fn check_integrity(conn: &Connection) -> Result<(), String> {
    let integrity: String =
        conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if integrity != "ok" {
        return Err("integrity_check en échec".into());
    }
    let broken: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if broken != 0 {
        return Err(format!("{broken} violation(s) de clés étrangères"));
    }
    Ok(())
}

/// Objets que le schéma de la version `version` doit contenir.
fn expected_objects(version: u32) -> Vec<&'static str> {
    let mut names = vec!["destinations", "inbox_items", "inbox_items_recent", "inbox_items_destination"];
    if version >= 2 {
        names.extend([
            "tasks",
            "tasks_one_active_per_origin",
            "tasks_recent",
            "tasks_updated_at_must_grow",
            "tasks_origin_is_immutable",
            "inbox_items_converted_task",
            "inbox_items_converted",
        ]);
    }
    if version >= 3 {
        names.extend([
            "tasks_responsibility_kind_insert",
            "tasks_responsibility_kind_update",
            "tasks_section_kind_insert",
            "tasks_section_kind_update",
        ]);
    }
    names
}

/// Version, objets attendus présents, forme de `tasks`, aucune table temporaire résiduelle.
fn check_schema(conn: &Connection, version: u32) -> Result<(), String> {
    let current: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if current != version {
        return Err(format!("version de schéma v{current} au lieu de v{version}"));
    }
    let present: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master")
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .map_err(|e| e.to_string())?;
    for name in expected_objects(version) {
        if !present.iter().any(|p| p == name) {
            return Err(format!("objet de schéma manquant : {name}"));
        }
    }
    if version >= 3 {
        let has_old_column: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('tasks') WHERE name = 'destination_id'",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if has_old_column != 0 {
            return Err("l'ancienne colonne destination_id subsiste sur tasks".into());
        }
        let leftover: i64 = conn
            .query_row("SELECT count(*) FROM sqlite_master WHERE name = 'tasks_new'", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if leftover != 0 {
            return Err("la table de travail tasks_new subsiste".into());
        }
    }
    let temp: i64 = conn
        .query_row("SELECT count(*) FROM sqlite_temp_master", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if temp != 0 {
        return Err("objet temporaire résiduel".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// La répétition
// ---------------------------------------------------------------------------------------------

/// L'aller-retour conversion/annulation n'est JAMAIS déclaré sans avoir eu lieu : sur une capture
/// existante de la copie, ou, si elle n'en contient aucune, sur une capture FICTIVE créée puis
/// supprimée dans la copie temporaire (l'état attendu est comparé ensuite).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum RoundTrip {
    ExistingCapture,
    FictitiousCapture,
}

#[derive(Debug)]
struct Report {
    plan: Plan,
    version_before: u32,
    version_after: u32,
    /// `(table, nombre de lignes)` de l'état de départ.
    tables: Vec<(&'static str, usize)>,
    live_captures: usize,
    /// Sur quelle capture l'aller-retour a réellement eu lieu.
    round_trip: RoundTrip,
    backup: String,
    /// Présence de `.db`, `-wal`, `-shm` dans la source.
    present: Vec<(&'static str, bool)>,
}

/// Étapes réellement vérifiées (aucune n'est présumée) : sert à la ligne-marqueur du rapport.
#[derive(Debug, Default, Clone, Copy)]
struct Stages {
    integrity: bool,
    schema: bool,
    projection: bool,
    backup: bool,
    round_trip: bool,
    restore: bool,
}

impl Stages {
    fn all(self) -> bool {
        self.integrity && self.schema && self.projection && self.backup && self.round_trip && self.restore
    }
}

#[derive(Debug)]
struct Outcome {
    plan: Plan,
    result: Result<Report, String>,
    source: SourceCheck,
    stages: Stages,
}

type Copier<'a> = &'a dyn Fn(&Path, &Path) -> io::Result<u64>;
/// Modification volontaire appliquée à la copie migrée (test de la détection de divergence).
type Mutation<'a> = &'a dyn Fn(&Connection);

/// Paramètres d'injection de pannes : par défaut, comportement réel.
struct Options<'a> {
    copier: Copier<'a>,
    base: Option<&'a Path>,
    /// Appelée après la migration, AVANT les contrôles : simule une migration défectueuse.
    mutate_after_migration: Option<Mutation<'a>>,
    /// Remplace la liste des migrations (migration défaillante). Longueur = version d'arrivée.
    migrations: Option<Vec<String>>,
}

fn default_copier(from: &Path, to: &Path) -> io::Result<u64> {
    fs::copy(from, to)
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options { copier: &default_copier, base: None, mutate_after_migration: None, migrations: None }
    }
}

fn new_work_dir(base: Option<&Path>) -> io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(WORK_PREFIX);
    match base {
        Some(base) => builder.tempdir_in(base),
        None => builder.tempdir(),
    }
}

/// Répétition complète sur une copie temporaire de `source_db`.
fn rehearse(source_db: &Path, plan: Plan) -> Outcome {
    rehearse_options(source_db, plan, &Options::default())
}

/// `copier` et `base` sont injectables pour simuler des pannes et observer le nettoyage.
fn rehearse_with(source_db: &Path, plan: Plan, copier: Copier<'_>, base: Option<&Path>) -> Outcome {
    rehearse_options(source_db, plan, &Options { copier, base, ..Options::default() })
}

fn rehearse_options(source_db: &Path, plan: Plan, options: &Options<'_>) -> Outcome {
    let before = match SourceSnapshot::capture(source_db) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Outcome {
                plan,
                result: Err(format!("lecture de la source impossible : {error}")),
                source: SourceCheck::NotChecked,
                stages: Stages::default(),
            }
        }
    };
    let mut stages = Stages::default();
    let result = run(source_db, plan, &before, options, &mut stages);
    // Contrôle final de la source, y compris quand `run` a échoué.
    let source = match SourceSnapshot::capture(source_db) {
        Ok(after) if after == before => SourceCheck::Unchanged,
        Ok(_) => SourceCheck::Different,
        Err(error) => SourceCheck::Unverifiable(error),
    };
    Outcome { plan, result, source, stages }
}

/// Aller-retour avec le code de production (schéma v3). Renvoie les identifiants des tâches créées.
fn round_trip_production(conn: &Connection, target_id: &str, stamp: &mut i64) -> Result<Vec<String>, String> {
    let mut created = Vec::new();
    for _ in 0..2 {
        let current = inbox::get_item(conn, target_id).map_err(|e| e.to_string())?;
        *stamp += 1;
        let done = tasks::convert_item(conn, target_id, current.updated_at, None, *stamp)
            .map_err(|e| format!("conversion : {e}"))?;
        *stamp += 1;
        tasks::cancel_conversion(conn, &done.task.id, *stamp).map_err(|e| format!("annulation : {e}"))?;
        created.push(done.task.id);
    }
    Ok(created)
}

/// Aller-retour en SQL v2 (le code de production cible la v3 et ne s'applique pas à une base v2).
/// Reproduit exactement les écritures de la brique 002 sur le schéma v2.
fn round_trip_v2(conn: &Connection, target_id: &str, stamp: &mut i64) -> Result<Vec<String>, String> {
    let mut created = Vec::new();
    for n in 0..2 {
        let (content, destination): (String, Option<String>) = conn
            .query_row(
                "SELECT content, destination_id FROM inbox_items WHERE id = ?1 AND deleted_at IS NULL
                 AND converted_task_id IS NULL",
                [target_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| format!("conversion : {e}"))?;
        let title: String = content.lines().next().unwrap_or("Répétition").chars().take(120).collect();
        let task_id = format!("repetition-{n}-{target_id}");
        *stamp += 1;
        conn.execute(
            "INSERT INTO tasks (id, title, details, status, destination_id, origin_inbox_item_id,
                                created_at, updated_at)
             VALUES (?1, ?2, ?3, 'todo', ?4, ?5, ?6, ?6)",
            rusqlite::params![task_id, title, content, destination, target_id, *stamp],
        )
        .map_err(|e| format!("conversion : {e}"))?;
        conn.execute(
            "UPDATE inbox_items SET converted_at = ?2, converted_task_id = ?3 WHERE id = ?1",
            rusqlite::params![target_id, *stamp, task_id],
        )
        .map_err(|e| format!("conversion : {e}"))?;
        *stamp += 1;
        conn.execute(
            "UPDATE tasks SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL AND updated_at = created_at",
            rusqlite::params![task_id, *stamp],
        )
        .map_err(|e| format!("annulation : {e}"))?;
        let restored = conn
            .execute(
                "UPDATE inbox_items SET converted_at = NULL, converted_task_id = NULL
                 WHERE id = ?1 AND converted_task_id = ?2",
                rusqlite::params![target_id, task_id],
            )
            .map_err(|e| format!("annulation : {e}"))?;
        if restored != 1 {
            return Err("annulation : la capture n'a pas été remise dans la boîte".into());
        }
        created.push(task_id);
    }
    Ok(created)
}

fn run(
    source_db: &Path,
    plan: Plan,
    before: &SourceSnapshot,
    options: &Options<'_>,
    stages: &mut Stages,
) -> Result<Report, String> {
    // `work` est déclaré en premier : il est supprimé en dernier, après la fermeture des connexions.
    let work = new_work_dir(options.base).map_err(|e| format!("dossier de travail : {e}"))?;
    let db = work.path().join(DB_FILE_NAME);

    // 1. Copie des fichiers présents (sous le MÊME nom pour que `-wal` reste apparié). Jamais le
    //    seul `.db` : une transaction validée peut n'exister que dans `-wal`.
    for (suffix, identity) in &before.files {
        if identity.is_none() {
            continue;
        }
        let target = sibling(&db, suffix);
        (options.copier)(&sibling(source_db, suffix), &target)
            .map_err(|e| format!("copie de « {} » : {e}", file_label(suffix)))?;
        // La copie de référence est en lecture seule (procédure PowerShell) et `fs::copy` en
        // propage l'attribut : on le lève sur la copie de TRAVAIL uniquement, la source n'est
        // jamais touchée.
        let mut permissions = fs::metadata(&target)
            .map_err(|e| format!("copie de « {} » : {e}", file_label(suffix)))?
            .permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&target, permissions)
            .map_err(|e| format!("copie de « {} » : {e}", file_label(suffix)))?;
    }
    // La source n'a pas bougé pendant la copie, et la copie est fidèle (avant toute ouverture SQLite).
    let during = SourceSnapshot::capture(source_db).map_err(|e| format!("relecture de la source : {e}"))?;
    if during != *before {
        return Err("les fichiers source ont changé pendant la copie : essai abandonné".into());
    }
    for (suffix, identity) in &before.files {
        if let Some(identity) = identity {
            let copy = sha256_of(&sibling(&db, suffix))
                .map_err(|e| format!("copie de « {} » : {e}", file_label(suffix)))?;
            if copy != identity.sha256 {
                return Err(format!("la copie de « {} » diffère de la source (SHA-256)", file_label(suffix)));
            }
        }
    }

    // 2. État de départ, lu sur la copie (la fermeture reporte -wal dans la copie). La version
    //    réelle doit être EXACTEMENT celle du plan ; la source doit être cohérente.
    let reference = {
        let conn = Connection::open(&db).map_err(|e| e.to_string())?;
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if version != plan.from {
            return Err(format!(
                "base en version {version} : la répétition demandée attend une base v{}",
                plan.from
            ));
        }
        check_integrity(&conn).map_err(|e| format!("source incohérente avant migration : {e}"))?;
        read_snapshot(&conn, version)?
    };
    let expected = expected_after(&reference, plan.to)
        .map_err(|e| format!("source incohérente avant migration : {e}"))?;

    // 3. Migration réelle (sauvegarde automatique vérifiée, puis migrations jusqu'à `plan.to`).
    let migrations: Vec<&str> = match &options.migrations {
        Some(custom) => custom.iter().map(String::as_str).collect(),
        None => plan.migrations().to_vec(),
    };
    let conn = open_with(&db, &migrations).map_err(|e| format!("migration : {e}"))?;
    let version_after: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if let Some(mutate) = options.mutate_after_migration {
        mutate(&conn);
    }
    check_integrity(&conn).map_err(|e| format!("après migration : {e}"))?;
    stages.integrity = true;
    check_schema(&conn, plan.to).map_err(|e| format!("schéma après migration : {e}"))?;
    stages.schema = true;
    let divergences = compare(&expected, &read_snapshot(&conn, plan.to)?);
    if !divergences.is_empty() {
        return Err(format!("projection non conforme : {}", describe(&divergences)));
    }
    stages.projection = true;

    // 4. Sauvegarde produite : une seule, à la version de départ, à l'état de référence.
    let backups: Vec<PathBuf> = fs::read_dir(backup::backup_dir(&db))
        .map_err(|e| format!("dossier des sauvegardes : {e}"))?
        .flatten()
        .map(|e| e.path())
        .collect();
    if backups.len() != 1 {
        return Err(format!("{} sauvegarde(s) au lieu d'une", backups.len()));
    }
    {
        let copy = Connection::open(&backups[0]).map_err(|e| e.to_string())?;
        let v: u32 = copy.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(|e| e.to_string())?;
        if v != plan.from {
            return Err(format!("la sauvegarde est en v{v} au lieu de v{}", plan.from));
        }
        let divergences = compare(&reference, &read_snapshot(&copy, v)?);
        if !divergences.is_empty() {
            return Err(format!(
                "la sauvegarde ne correspond pas à l'état de départ : {}",
                describe(&divergences)
            ));
        }
    }
    stages.backup = true;

    // 5. Aller-retour métier : conversion, annulation, reconversion, annulation. Sur une capture
    //    existante ; sinon sur une capture fictive créée ICI (copie temporaire) puis supprimée.
    let live = inbox::list_items(&conn, &inbox::InboxFilter::All, inbox::Scope::Active, None, 1)
        .map_err(|e| e.to_string())?;
    let live_captures = live.total as usize;
    let (target_id, round_trip) = match live.items.first() {
        Some(item) => (item.id.clone(), RoundTrip::ExistingCapture),
        None => {
            let created = inbox::create_item(&conn, "Capture fictive de répétition", None, 3_999_999_999_000)
                .map_err(|e| format!("capture fictive : {e}"))?;
            (created.id, RoundTrip::FictitiousCapture)
        }
    };
    let mut stamp = 4_000_000_000_000i64;
    let created_tasks = if plan.to >= 3 {
        round_trip_production(&conn, &target_id, &mut stamp)?
    } else {
        round_trip_v2(&conn, &target_id, &mut stamp)?
    };
    // Aucun résidu : les tâches de l'essai d'abord (clé étrangère), puis la capture fictive.
    for task_id in &created_tasks {
        conn.execute("DELETE FROM tasks WHERE id = ?1", [task_id]).map_err(|e| e.to_string())?;
    }
    if round_trip == RoundTrip::FictitiousCapture {
        conn.execute("DELETE FROM inbox_items WHERE id = ?1", [&target_id]).map_err(|e| e.to_string())?;
    }
    let divergences = compare(&expected, &read_snapshot(&conn, plan.to)?);
    if !divergences.is_empty() {
        return Err(format!("l'aller-retour a modifié des données : {}", describe(&divergences)));
    }
    // Seulement ici : les conversions et annulations ont réellement été exécutées.
    stages.round_trip = true;

    // 6. Restauration depuis la sauvegarde (procédure documentée), puis remigration.
    drop(conn);
    let aside = work.path().join("mis-de-cote");
    fs::create_dir_all(&aside).map_err(|e| e.to_string())?;
    for suffix in SUFFIXES {
        let file = sibling(&db, suffix);
        if file.is_file() {
            fs::rename(&file, aside.join(file.file_name().unwrap())).map_err(|e| e.to_string())?;
        }
    }
    fs::copy(&backups[0], &db).map_err(|e| e.to_string())?;
    let restored = open_with(&db, plan.migrations()).map_err(|e| format!("réouverture après restauration : {e}"))?;
    check_integrity(&restored).map_err(|e| format!("après restauration : {e}"))?;
    let divergences = compare(&expected, &read_snapshot(&restored, plan.to)?);
    if !divergences.is_empty() {
        return Err(format!("la base restaurée diffère de l'attendu : {}", describe(&divergences)));
    }
    stages.restore = true;
    let backup_name = backups[0].file_name().unwrap().to_string_lossy().into_owned();
    drop(restored);

    let count = |table: &'static str, rows: &[Row]| (table, rows.len());
    Ok(Report {
        plan,
        version_before: reference.version,
        version_after,
        tables: vec![
            count("destinations", &reference.destinations),
            count("inbox_items", &reference.captures),
            count("tasks", &reference.tasks),
        ],
        live_captures,
        round_trip,
        backup: backup_name,
        present: before.files.iter().map(|(s, identity)| (*s, identity.is_some())).collect(),
    })
}

/// Ligne-marqueur lue par la procédure PowerShell. Vocabulaire FIXE : aucun chemin, aucune
/// empreinte, aucun texte, aucun message d'erreur ; chaque valeur n'est `OK` que si l'étape
/// correspondante a réellement été vérifiée. `from` et `to` sont les versions DEMANDÉES.
fn marker_line(outcome: &Outcome) -> String {
    let ok_or_fail = |ok: bool| if ok { "OK" } else { "FAIL" };
    let source = match &outcome.source {
        SourceCheck::Unchanged => "OK",
        SourceCheck::Different => "DIFFERENT",
        SourceCheck::Unverifiable(_) => "UNVERIFIABLE",
        SourceCheck::NotChecked => "NOTCHECKED",
    };
    let stages = outcome.stages;
    // `OK` global seulement si le résultat est bon, la source intacte ET chaque étape obligatoire
    // validée : aucune étape ne peut manquer sans faire échouer le tout.
    let status = outcome.result.is_ok() && outcome.source == SourceCheck::Unchanged && stages.all();
    format!(
        "REHEARSAL-RESULT status={} from={} to={} integrity={} schema={} projection={} backup={} roundtrip={} restore={} source={source}",
        ok_or_fail(status),
        outcome.plan.from,
        outcome.plan.to,
        ok_or_fail(stages.integrity),
        ok_or_fail(stages.schema),
        ok_or_fail(stages.projection),
        ok_or_fail(stages.backup),
        ok_or_fail(stages.round_trip),
        ok_or_fail(stages.restore),
    )
}

/// Marqueur d'un échec avant toute répétition (variable, plan, garde-fous ou validation).
const MARKER_NOT_STARTED: &str = "REHEARSAL-RESULT status=FAIL from=? to=? integrity=FAIL schema=FAIL projection=FAIL backup=FAIL roundtrip=FAIL restore=FAIL source=NOTCHECKED";

/// Validation (plan, puis chemin) puis répétition : rien n'est créé ni ouvert si elle échoue.
fn rehearsal_from_values(
    raw_path: Option<OsString>,
    raw_from: Option<OsString>,
    raw_to: Option<OsString>,
    protected: &Protected,
) -> Result<Outcome, String> {
    let plan = Plan::from_values(raw_from, raw_to)?;
    let path = resolve_source(raw_path, protected)?;
    Ok(rehearse(&path, plan))
}

// ---------------------------------------------------------------------------------------------
// Jeux d'essai fictifs
// ---------------------------------------------------------------------------------------------

fn insert_fictitious_rows(conn: &Connection) {
    for (i, text) in ["Capture fictive A", "Fictive é à ü 日本語", "'; DROP TABLE inbox_items; --"]
        .iter()
        .enumerate()
    {
        conn.execute(
            "INSERT INTO inbox_items (id, content, destination_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            rusqlite::params![format!("f{i}"), text, (i == 1).then_some("finances"), 1_000 + i as i64],
        )
        .unwrap();
    }
    conn.execute("UPDATE inbox_items SET deleted_at = 9 WHERE id = 'f2'", []).unwrap();
}

/// Base v1 fictive, fermée proprement (en général sans `-wal`).
fn fictitious_v1() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    {
        let conn = Connection::open(&path).unwrap();
        configure(&conn).unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        insert_fictitious_rows(&conn);
    }
    (dir, path)
}

/// Base v1 fictive avec `-wal` et `-shm` non vidés (écritures présentes seulement dans `-wal`),
/// copiée pendant que la connexion d'origine est encore ouverte, comme après un arrêt brutal.
fn fictitious_v1_with_wal() -> (tempfile::TempDir, PathBuf) {
    let origin = tempfile::tempdir().unwrap();
    let live = origin.path().join(DB_FILE_NAME);
    let conn = Connection::open(&live).unwrap();
    configure(&conn).unwrap();
    conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    conn.execute_batch(MIGRATIONS[0]).unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    insert_fictitious_rows(&conn);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    for suffix in SUFFIXES {
        let from = sibling(&live, suffix);
        if from.is_file() {
            fs::copy(&from, sibling(&path, suffix)).unwrap();
        }
    }
    drop(conn);
    assert!(fs::metadata(sibling(&path, "-wal")).unwrap().len() > 0, "le -wal fictif doit contenir des données");
    (dir, path)
}

/// Zone protégée fictive (joue le rôle de `%LOCALAPPDATA%`), dans un dossier temporaire.
fn fake_protected() -> (tempfile::TempDir, Protected) {
    let root = tempfile::tempdir().unwrap();
    let protected =
        Protected::from_roots(&[("%LOCALAPPDATA% fictif".to_string(), root.path().to_path_buf())]).unwrap();
    (root, protected)
}

/// Validation du chemin avec un plan valide (v1 -> v2) : isole les tests de chemin du plan.
fn rehearsal_from_value(raw: Option<OsString>, protected: &Protected) -> Result<Outcome, String> {
    rehearsal_from_values(raw, Some("1".into()), Some("2".into()), protected)
}

fn dir_is_empty(dir: &Path) -> bool {
    fs::read_dir(dir).unwrap().next().is_none()
}

fn assert_unchanged(outcome: &Outcome) {
    assert_eq!(outcome.source, SourceCheck::Unchanged, "{:?}", outcome.result);
}

// ---------------------------------------------------------------------------------------------
// Variable d'environnement et validation du chemin
// ---------------------------------------------------------------------------------------------

#[test]
fn variable_absente_echec_explicite_sans_rien_creer() {
    let (_root, protected) = fake_protected();
    let error = rehearsal_from_value(None, &protected).unwrap_err();
    assert!(error.contains(ENV_VAR) && error.contains("absente"), "{error}");
}

#[test]
fn variable_vide_ou_espaces_echec_explicite() {
    let (_root, protected) = fake_protected();
    for value in ["", " ", "   \t  "] {
        let error = rehearsal_from_value(Some(value.into()), &protected).unwrap_err();
        assert!(error.contains("vide ou composée d'espaces"), "{value:?}: {error}");
    }
}

#[test]
fn chemin_invalide_echec_sans_creer_ni_ouvrir_de_base() {
    let (_root, protected) = fake_protected();
    let dir = tempfile::tempdir().unwrap();

    // Relatif, avec « .. », mauvais nom, inexistant : tous refusés, rien n'est créé.
    let relative = Path::new("morganiser.db");
    let parent = dir.path().join("a").join("..").join(DB_FILE_NAME);
    let wrong_name = dir.path().join("autre.db");
    let missing = dir.path().join(DB_FILE_NAME);
    for candidate in [relative, parent.as_path(), wrong_name.as_path(), missing.as_path()] {
        let error = rehearsal_from_value(Some(candidate.as_os_str().to_owned()), &protected).unwrap_err();
        assert!(!error.is_empty());
    }
    assert!(dir_is_empty(dir.path()), "aucun fichier ne doit être créé par une validation refusée");
    assert!(!missing.exists(), "la base inexistante n'a pas été créée par une ouverture SQLite");
}

#[test]
fn un_dossier_ou_un_fichier_non_ordinaire_est_refuse() {
    let (_root, protected) = fake_protected();
    let dir = tempfile::tempdir().unwrap();
    let as_dir = dir.path().join(DB_FILE_NAME);
    fs::create_dir(&as_dir).unwrap();
    let error = validate_source(&as_dir, &protected).unwrap_err();
    assert!(error.contains("fichier ordinaire"), "{error}");
}

#[test]
fn les_zones_protegees_sont_refusees_sur_le_chemin_saisi() {
    let (root, protected) = fake_protected();

    // Dossier de données Dev et Stable, calculés avec `environment::data_dir`.
    for channel in [Channel::Dev, Channel::Stable] {
        let dir = data_dir(root.path(), channel);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(DB_FILE_NAME);
        fs::write(&file, b"x").unwrap();
        let error = validate_source(&file, &protected).unwrap_err();
        assert!(error.contains("dossier de données") && error.contains("chemin saisi"), "{channel:?}: {error}");
    }

    // Autre dossier sous la racine protégée.
    let other = root.path().join("Autre");
    fs::create_dir_all(&other).unwrap();
    let file = other.join(DB_FILE_NAME);
    fs::write(&file, b"x").unwrap();
    assert!(validate_source(&file, &protected).unwrap_err().contains("chemin sous"));

    // Composant « com.morganiser* », même hors de toute racine, quelle que soit la casse.
    for name in [DEV_IDENTIFIER, STABLE_IDENTIFIER, "COM.MORGANISER.DESKTOP", "com.morganiser.autre"] {
        let outside = tempfile::tempdir().unwrap();
        let folder = outside.path().join(name);
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join(DB_FILE_NAME);
        fs::write(&file, b"x").unwrap();
        let error = validate_source(&file, &protected).unwrap_err();
        assert!(error.contains("com.morganiser"), "{name}: {error}");
    }
}

#[test]
fn la_comparaison_ignore_la_casse_et_le_prefixe_verbatim() {
    let (root, protected) = fake_protected();
    // Chemin inexistant : le refus vient du contrôle du chemin saisi, avant tout accès disque.
    let shouting = PathBuf::from(root.path().to_string_lossy().to_uppercase()).join("X").join(DB_FILE_NAME);
    assert!(validate_source(&shouting, &protected).unwrap_err().contains("chemin sous"));

    #[cfg(windows)]
    {
        let verbatim = PathBuf::from(format!(r"\\?\{}", root.path().display())).join("X").join(DB_FILE_NAME);
        assert!(validate_source(&verbatim, &protected).unwrap_err().contains("chemin sous"));
        assert_eq!(
            normalized(Path::new(r"C:\Users\Moi\a")),
            normalized(Path::new(r"\\?\c:\users\MOI\A"))
        );
    }
}

#[test]
fn une_zone_protegee_non_resolue_fait_echouer_la_verification() {
    // Racine inexistante : on n'ignore pas, on refuse de continuer.
    let missing = tempfile::tempdir().unwrap().path().join("n-existe-pas");
    let error = Protected::from_roots(&[("%LOCALAPPDATA%".into(), missing)]).err().unwrap();
    assert!(error.contains("non résolue"), "{error}");
    // Racine relative.
    assert!(Protected::from_roots(&[("%APPDATA%".into(), PathBuf::from("relatif"))]).is_err());
    // Variables absentes ou vides.
    let error = Protected::from_lookup(|_| None).err().unwrap();
    assert!(error.contains("LOCALAPPDATA"), "{error}");
    assert!(Protected::from_lookup(|_| Some(OsString::new())).is_err());
    // Une seule des deux variables : échec aussi.
    let dir = tempfile::tempdir().unwrap();
    let only_local = |name: &str| (name == "LOCALAPPDATA").then(|| dir.path().as_os_str().to_owned());
    assert!(Protected::from_lookup(only_local).err().unwrap().contains("APPDATA"));
    // Les deux définies et valides : succès.
    assert!(Protected::from_lookup(|_| Some(dir.path().as_os_str().to_owned())).is_ok());
}

#[test]
fn les_refus_ne_revelent_jamais_de_chemin_personnel() {
    let (root, protected) = fake_protected();
    let folder = root.path().join("Dossier-personnel-secret");
    fs::create_dir_all(&folder).unwrap();
    let file = folder.join(DB_FILE_NAME);
    fs::write(&file, b"x").unwrap();
    let error = validate_source(&file, &protected).unwrap_err();
    assert!(!error.contains("Dossier-personnel-secret"), "{error}");
    assert!(!error.contains(&root.path().to_string_lossy().to_string()), "{error}");
}

/// Crée une jonction Windows (`mklink /J`, sans droits d'administrateur). `None` si impossible.
#[cfg(windows)]
struct Junction(PathBuf);

#[cfg(windows)]
impl Junction {
    fn create(link: &Path, target: &Path) -> Option<Junction> {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()?;
        status.success().then(|| Junction(link.to_path_buf()))
    }
}

#[cfg(windows)]
impl Drop for Junction {
    fn drop(&mut self) {
        // Retire la jonction elle-même ; la cible n'est jamais touchée.
        let _ = fs::remove_dir(&self.0);
    }
}

#[cfg(windows)]
#[test]
fn une_jonction_vers_une_zone_protegee_est_refusee_sur_le_chemin_canonique() {
    let (root, protected) = fake_protected();
    let real = data_dir(root.path(), Channel::Dev); // dossier fictif « Dev », jamais le vrai
    fs::create_dir_all(&real).unwrap();
    fs::write(real.join(DB_FILE_NAME), b"x").unwrap();

    let outside = tempfile::tempdir().unwrap();
    let link = outside.path().join("essai");
    let Some(_junction) = Junction::create(&link, &real) else {
        eprintln!("mklink /J indisponible ici : cas non testé");
        return;
    };
    let through = link.join(DB_FILE_NAME);
    // Le chemin saisi n'a rien de protégé ; seule la forme canonique l'est.
    assert_eq!(protected.refuses(&through), None);
    let error = validate_source(&through, &protected).unwrap_err();
    assert!(error.contains("chemin canonique"), "{error}");
}

#[cfg(windows)]
#[test]
fn une_jonction_vers_un_dossier_ordinaire_est_refusee_comme_redirection() {
    let (_root, protected) = fake_protected();
    let target = tempfile::tempdir().unwrap();
    fs::write(target.path().join(DB_FILE_NAME), b"x").unwrap();
    let outside = tempfile::tempdir().unwrap();
    let link = outside.path().join("essai");
    let Some(_junction) = Junction::create(&link, target.path()) else {
        eprintln!("mklink /J indisponible ici : cas non testé");
        return;
    };
    let error = validate_source(&link.join(DB_FILE_NAME), &protected).unwrap_err();
    assert!(error.contains("jonction"), "{error}");
}

fn try_symlink_file(link: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_file(target, link);
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(target, link);
    result.is_ok()
}

#[test]
fn un_lien_symbolique_pour_la_base_ou_ses_fichiers_associes_est_refuse() {
    let (_root, protected) = fake_protected();
    let real = tempfile::tempdir().unwrap();
    let real_db = real.path().join(DB_FILE_NAME);
    fs::write(&real_db, b"x").unwrap();

    // La base elle-même est un lien.
    let a = tempfile::tempdir().unwrap();
    let link_db = a.path().join(DB_FILE_NAME);
    if try_symlink_file(&link_db, &real_db) {
        let error = validate_source(&link_db, &protected).unwrap_err();
        assert!(error.contains("lien symbolique"), "{error}");
    } else {
        eprintln!("création de liens symboliques non autorisée ici : cas non testé");
    }

    // Le -wal est un lien vers un autre fichier.
    let b = tempfile::tempdir().unwrap();
    let db = b.path().join(DB_FILE_NAME);
    fs::write(&db, b"x").unwrap();
    let other = real.path().join("autre-wal");
    fs::write(&other, b"w").unwrap();
    if try_symlink_file(&sibling(&db, "-wal"), &other) {
        let error = validate_source(&db, &protected).unwrap_err();
        assert!(error.contains("-wal") && error.contains("lien symbolique"), "{error}");
    }
}

#[test]
fn les_fichiers_associes_absents_sont_acceptes_et_un_dossier_wal_est_refuse() {
    let (_root, protected) = fake_protected();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join(DB_FILE_NAME);
    fs::write(&db, b"x").unwrap();
    assert!(validate_source(&db, &protected).is_ok(), "-wal et -shm absents : normal après une fermeture propre");
    fs::write(sibling(&db, "-wal"), b"w").unwrap();
    fs::write(sibling(&db, "-shm"), b"s").unwrap();
    assert!(validate_source(&db, &protected).is_ok());
    fs::remove_file(sibling(&db, "-shm")).unwrap();
    fs::create_dir(sibling(&db, "-shm")).unwrap();
    assert!(validate_source(&db, &protected).unwrap_err().contains("-shm"));
}

// ---------------------------------------------------------------------------------------------
// Empreintes SHA-256 de la source
// ---------------------------------------------------------------------------------------------

#[test]
fn le_sha256_est_celui_du_standard() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("abc");
    fs::write(&file, b"abc").unwrap();
    let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let hex: String = sha256_of(&file).unwrap().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, expected);
    // Fichier plus grand que le tampon de lecture, vide.
    fs::write(&file, vec![7u8; 200_000]).unwrap();
    assert!(sha256_of(&file).is_ok());
    fs::write(&file, b"").unwrap();
    let empty: String = sha256_of(&file).unwrap().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(empty, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn l_instantane_distingue_presence_absence_et_contenu() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join(DB_FILE_NAME);
    fs::write(&db, b"base").unwrap();
    let alone = SourceSnapshot::capture(&db).unwrap();
    assert_eq!(alone.files[0].0, "");
    assert!(alone.files[0].1.is_some() && alone.files[1].1.is_none() && alone.files[2].1.is_none());
    assert_eq!(alone, SourceSnapshot::capture(&db).unwrap(), "deux lectures identiques");

    fs::write(sibling(&db, "-wal"), b"journal").unwrap();
    let with_wal = SourceSnapshot::capture(&db).unwrap();
    assert_ne!(alone, with_wal, "l'apparition d'un -wal est détectée");
    fs::remove_file(sibling(&db, "-wal")).unwrap();
    assert_eq!(alone, SourceSnapshot::capture(&db).unwrap());
}

#[test]
fn un_changement_de_contenu_a_taille_identique_est_detecte() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join(DB_FILE_NAME);
    fs::write(&db, b"0123456789").unwrap();
    let before = SourceSnapshot::capture(&db).unwrap();
    fs::write(&db, b"0123456780").unwrap(); // même taille, un octet change
    assert_ne!(before, SourceSnapshot::capture(&db).unwrap());
}

#[test]
fn un_changement_de_contenu_avec_la_date_restauree_est_detecte() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join(DB_FILE_NAME);
    fs::write(&db, b"0123456789").unwrap();
    let metadata = fs::metadata(&db).unwrap();
    let (len, modified) = (metadata.len(), metadata.modified().unwrap());
    let before = SourceSnapshot::capture(&db).unwrap();

    fs::write(&db, b"0123456780").unwrap();
    fs::OpenOptions::new().write(true).open(&db).unwrap().set_modified(modified).unwrap();

    // L'ancien contrôle (taille + date) n'y verrait rien…
    let after_meta = fs::metadata(&db).unwrap();
    assert_eq!((after_meta.len(), after_meta.modified().unwrap()), (len, modified));
    // … le SHA-256 si.
    assert_ne!(before, SourceSnapshot::capture(&db).unwrap());
}

// ---------------------------------------------------------------------------------------------
// La répétition sur des bases fictives
// ---------------------------------------------------------------------------------------------

#[test]
fn la_repetition_fonctionne_sans_wal_et_la_source_est_intacte() {
    let (_dir, path) = fictitious_v1();
    assert!(!sibling(&path, "-wal").exists(), "fermeture propre : pas de -wal");
    let before = SourceSnapshot::capture(&path).unwrap();

    let outcome = rehearse(&path, PLAN_V1_V2);
    assert_unchanged(&outcome);
    let report = outcome.result.unwrap();

    assert_eq!((report.version_before, report.version_after), (1, 2));
    assert_eq!(report.tables.iter().find(|t| t.0 == "inbox_items").unwrap().1, 3);
    assert_eq!(report.tables.iter().find(|t| t.0 == "destinations").unwrap().1, 5);
    assert_eq!(report.live_captures, 2); // la 3e est à la corbeille
    assert!(report.backup.starts_with("morganiser-v1-"));
    assert_eq!(report.present, vec![("", true), ("-wal", false), ("-shm", false)]);
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
}

/// Base v1 fictive SANS capture active (destinations seules, ou captures toutes à la corbeille).
fn fictitious_v1_without_active_capture(trashed_only: bool) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    {
        let conn = Connection::open(&path).unwrap();
        configure(&conn).unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        if trashed_only {
            for i in 0..2 {
                conn.execute(
                    "INSERT INTO inbox_items (id, content, created_at, updated_at, deleted_at)
                     VALUES (?1, 'À la corbeille', 5, 5, 9)",
                    [format!("t{i}")],
                )
                .unwrap();
            }
        }
    }
    (dir, path)
}

#[test]
fn sans_capture_active_l_aller_retour_a_lieu_sur_une_capture_fictive_sans_residu() {
    for trashed_only in [false, true] {
        let (_dir, path) = fictitious_v1_without_active_capture(trashed_only);
        let before = SourceSnapshot::capture(&path).unwrap();

        let outcome = rehearse(&path, PLAN_V1_V2);
        assert_unchanged(&outcome);
        assert!(outcome.stages.round_trip, "l'aller-retour a réellement été exécuté");
        let report = outcome.result.unwrap();
        assert_eq!(report.live_captures, 0);
        assert_eq!(report.round_trip, RoundTrip::FictitiousCapture);
        // Le contenu de référence est intact : 0 ou 2 captures, aucune capture fictive restante.
        let expected_rows = if trashed_only { 2 } else { 0 };
        assert_eq!(report.tables.iter().find(|t| t.0 == "inbox_items").unwrap().1, expected_rows);
        assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
        assert_eq!(
            marker_line(&rehearse(&path, PLAN_V1_V2)),
            "REHEARSAL-RESULT status=OK from=1 to=2 integrity=OK schema=OK projection=OK backup=OK roundtrip=OK restore=OK source=OK"
        );
    }
}

#[test]
fn avec_des_captures_actives_l_aller_retour_utilise_une_capture_existante() {
    let (_dir, path) = fictitious_v1();
    let report = rehearse(&path, PLAN_V1_V2).result.unwrap();
    assert_eq!(report.round_trip, RoundTrip::ExistingCapture);
}

#[test]
fn le_roundtrip_n_est_jamais_declare_quand_il_n_a_pas_eu_lieu() {
    // Échec AVANT l'aller-retour (base déjà migrée) : roundtrip jamais « OK ».
    let (_dir, path) = fictitious_v1_without_active_capture(false);
    drop(open(&path).unwrap());
    let outcome = rehearse(&path, PLAN_V1_V2);
    assert!(outcome.result.is_err());
    assert!(!outcome.stages.round_trip);
    assert!(marker_line(&outcome).contains("roundtrip=FAIL"));
}

#[test]
fn la_repetition_fonctionne_avec_wal_et_shm_et_ne_les_modifie_pas() {
    let (_dir, path) = fictitious_v1_with_wal();
    let before = SourceSnapshot::capture(&path).unwrap();
    assert!(before.files[1].1.is_some(), "-wal présent");

    let outcome = rehearse(&path, PLAN_V1_V2);
    assert_unchanged(&outcome);
    let report = outcome.result.unwrap();
    // Les trois captures n'existaient que dans -wal : elles sont bien reprises.
    assert_eq!(report.tables.iter().find(|t| t.0 == "inbox_items").unwrap().1, 3);
    assert!(report.present[1].1, "-wal signalé présent");
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before, "-wal et -shm intacts, octet pour octet");
}

#[test]
fn une_copie_de_reference_en_lecture_seule_est_repetee_sans_etre_modifiee() {
    // La procédure PowerShell met les trois fichiers de référence en lecture seule.
    let (_dir, path) = fictitious_v1_with_wal();
    for suffix in SUFFIXES {
        let file = sibling(&path, suffix);
        if file.is_file() {
            let mut permissions = fs::metadata(&file).unwrap().permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&file, permissions).unwrap();
        }
    }
    let before = SourceSnapshot::capture(&path).unwrap();

    let outcome = rehearse(&path, PLAN_V1_V2);
    assert_unchanged(&outcome);
    assert!(outcome.result.is_ok(), "{:?}", outcome.result);
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
    for suffix in SUFFIXES {
        let file = sibling(&path, suffix);
        if file.is_file() {
            assert!(fs::metadata(&file).unwrap().permissions().readonly(), "{suffix} doit rester en lecture seule");
            // Nettoyage du test : le dossier temporaire doit pouvoir être supprimé.
            let mut permissions = fs::metadata(&file).unwrap().permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            fs::set_permissions(&file, permissions).unwrap();
        }
    }
}

#[test]
fn la_repetition_refuse_une_base_qui_n_est_pas_en_v1_sans_toucher_la_source() {
    let (_dir, path) = fictitious_v1();
    drop(open(&path).unwrap()); // migrée jusqu'à la version courante par l'application fictive
    let before = SourceSnapshot::capture(&path).unwrap();
    let outcome = rehearse(&path, PLAN_V1_V2);
    assert!(outcome.result.as_ref().unwrap_err().contains("version 3"));
    assert_unchanged(&outcome);
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
}

#[test]
fn un_echec_de_copie_est_signale_source_intacte_et_travail_nettoye() {
    let (_dir, path) = fictitious_v1_with_wal();
    let before = SourceSnapshot::capture(&path).unwrap();
    let base = tempfile::tempdir().unwrap();

    let failing = |from: &Path, to: &Path| -> io::Result<u64> {
        if from.to_string_lossy().ends_with("-wal") {
            Err(io::Error::other("panne simulée"))
        } else {
            fs::copy(from, to)
        }
    };
    let outcome = rehearse_with(&path, PLAN_V1_V2, &failing, Some(base.path()));

    let error = outcome.result.unwrap_err();
    assert!(error.contains("-wal") && error.contains("panne simulée"), "{error}");
    assert_eq!(outcome.source, SourceCheck::Unchanged);
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
    assert!(dir_is_empty(base.path()), "erreur normale : dossier de travail supprimé");
}

#[test]
fn une_source_modifiee_pendant_la_copie_est_detectee() {
    let (_dir, path) = fictitious_v1();
    let base = tempfile::tempdir().unwrap();
    // Le « propriétaire » relance l'application pendant la copie : le .db change.
    let racing = |from: &Path, to: &Path| -> io::Result<u64> {
        let copied = fs::copy(from, to)?;
        let mut bytes = fs::read(from)?;
        bytes[200] ^= 0xFF;
        fs::write(from, bytes)?;
        Ok(copied)
    };
    let outcome = rehearse_with(&path, PLAN_V1_V2, &racing, Some(base.path()));
    assert!(outcome.result.unwrap_err().contains("changé pendant la copie"));
    assert_eq!(outcome.source, SourceCheck::Different, "la différence est signalée explicitement");
    assert!(dir_is_empty(base.path()));
}

#[test]
fn une_copie_infidele_est_refusee_avant_toute_ouverture_sqlite() {
    let (_dir, path) = fictitious_v1();
    let base = tempfile::tempdir().unwrap();
    let corrupting = |from: &Path, to: &Path| -> io::Result<u64> {
        let copied = fs::copy(from, to)?;
        let mut bytes = fs::read(to)?;
        bytes[200] ^= 0xFF; // altère la COPIE seulement
        fs::write(to, bytes)?;
        Ok(copied)
    };
    let outcome = rehearse_with(&path, PLAN_V1_V2, &corrupting, Some(base.path()));
    assert!(outcome.result.unwrap_err().contains("SHA-256"));
    assert_eq!(outcome.source, SourceCheck::Unchanged);
    assert!(dir_is_empty(base.path()));
}

#[test]
fn une_source_illisible_n_est_pas_verifiee_et_rien_n_est_cree() {
    let dir = tempfile::tempdir().unwrap();
    let base = tempfile::tempdir().unwrap();
    let as_dir = dir.path().join(DB_FILE_NAME);
    fs::create_dir(&as_dir).unwrap(); // « morganiser.db » est un dossier
    let outcome = rehearse_with(&as_dir, PLAN_V1_V2, &|f, t| fs::copy(f, t), Some(base.path()));
    assert!(outcome.result.is_err());
    assert_eq!(outcome.source, SourceCheck::NotChecked);
    assert!(dir_is_empty(base.path()), "aucun dossier de travail créé");
}

#[test]
fn l_etat_logique_distingue_deux_contenus_et_reconnait_deux_copies() {
    // Les instantanés structurels (valeurs réelles) remplacent les anciennes empreintes FNV.
    let (_dir_a, a) = fictitious_v1();
    let (_dir_b, b) = fictitious_v1();
    let conn_a = Connection::open(&a).unwrap();
    let conn_b = Connection::open(&b).unwrap();
    let snap_a = read_snapshot(&conn_a, 1).unwrap();
    assert!(compare(&snap_a, &read_snapshot(&conn_b, 1).unwrap()).is_empty());
    conn_b.execute("UPDATE inbox_items SET content = content || ' ' WHERE id = 'f0'", []).unwrap();
    let divergences = compare(&snap_a, &read_snapshot(&conn_b, 1).unwrap());
    assert_eq!(divergences.len(), 1);
    assert_eq!(
        (divergences[0].table, divergences[0].id.as_str(), divergences[0].field.as_str()),
        ("inbox_items", "f0", "content")
    );
}

// ---------------------------------------------------------------------------------------------
// Ligne-marqueur du rapport
// ---------------------------------------------------------------------------------------------

/// Chaque jeton `clé=valeur` du marqueur appartient au vocabulaire fixe attendu par la procédure.
fn assert_fixed_vocabulary(line: &str) {
    let mut parts = line.split(' ');
    assert_eq!(parts.next(), Some("REHEARSAL-RESULT"));
    for part in parts {
        let (key, value) = part.split_once('=').expect("jeton clé=valeur");
        let allowed: &[&str] = match key {
            "status" | "integrity" | "schema" | "projection" | "backup" | "roundtrip" | "restore" => {
                &["OK", "FAIL"]
            }
            "from" | "to" => &["1", "2", "3", "?"],
            "source" => &["OK", "DIFFERENT", "UNVERIFIABLE", "NOTCHECKED"],
            other => panic!("clé inattendue : {other}"),
        };
        assert!(allowed.contains(&value), "{key}={value}");
    }
}

#[test]
fn le_marqueur_de_reussite_est_exact_et_a_vocabulaire_fixe() {
    let (_dir, path) = fictitious_v1_with_wal();
    let line = marker_line(&rehearse(&path, PLAN_V1_V2));
    assert_eq!(
        line,
        "REHEARSAL-RESULT status=OK from=1 to=2 integrity=OK schema=OK projection=OK backup=OK roundtrip=OK restore=OK source=OK"
    );
    assert_fixed_vocabulary(&line);
    assert_fixed_vocabulary(MARKER_NOT_STARTED);
}

#[test]
fn le_marqueur_d_echec_ne_valide_que_les_etapes_atteintes() {
    let base = tempfile::tempdir().unwrap();

    // Échec dès la copie : aucune étape vérifiée, aucun message brut dans le marqueur.
    let (_dir, path) = fictitious_v1_with_wal();
    let failing = |_: &Path, _: &Path| -> io::Result<u64> { Err(io::Error::other(r"C:\Users\secret\chemin")) };
    let line = marker_line(&rehearse_with(&path, PLAN_V1_V2, &failing, Some(base.path())));
    assert_eq!(
        line,
        "REHEARSAL-RESULT status=FAIL from=1 to=2 integrity=FAIL schema=FAIL projection=FAIL backup=FAIL roundtrip=FAIL restore=FAIL source=OK"
    );
    assert!(!line.contains("secret") && !line.contains('\\'), "{line}");
    assert_fixed_vocabulary(&line);

    // Base déjà migrée : refus avant la migration, source intacte.
    let (_d2, migrated) = fictitious_v1();
    drop(open(&migrated).unwrap());
    let line = marker_line(&rehearse(&migrated, PLAN_V1_V2));
    assert!(line.starts_with("REHEARSAL-RESULT status=FAIL from=1 to=2 integrity=FAIL"), "{line}");
    assert!(line.ends_with("source=OK"), "{line}");
    assert_fixed_vocabulary(&line);

    // Source modifiée pendant la copie : DIFFERENT.
    let (_d3, racing_path) = fictitious_v1();
    let racing = |from: &Path, to: &Path| -> io::Result<u64> {
        let copied = fs::copy(from, to)?;
        let mut bytes = fs::read(from)?;
        bytes[200] ^= 0xFF;
        fs::write(from, bytes)?;
        Ok(copied)
    };
    let line = marker_line(&rehearse_with(&racing_path, PLAN_V1_V2, &racing, Some(base.path())));
    assert!(line.starts_with("REHEARSAL-RESULT status=FAIL"), "{line}");
    assert!(line.ends_with("source=DIFFERENT"), "{line}");
    assert_fixed_vocabulary(&line);

    // Source illisible : rien n'a démarré.
    let dir = tempfile::tempdir().unwrap();
    let as_dir = dir.path().join(DB_FILE_NAME);
    fs::create_dir(&as_dir).unwrap();
    let line = marker_line(&rehearse(&as_dir, PLAN_V1_V2));
    assert!(line.ends_with("source=NOTCHECKED"), "{line}");
    assert_fixed_vocabulary(&line);
}

// ---------------------------------------------------------------------------------------------
// Nettoyage du dossier de travail : réussite, erreur, panique, arrêt brutal
// ---------------------------------------------------------------------------------------------

#[test]
fn le_dossier_de_travail_est_supprime_apres_une_reussite() {
    let (_dir, path) = fictitious_v1();
    let base = tempfile::tempdir().unwrap();
    let outcome = rehearse_with(&path, PLAN_V1_V2, &|f, t| fs::copy(f, t), Some(base.path()));
    assert!(outcome.result.is_ok());
    assert!(dir_is_empty(base.path()), "réussite : plus aucun dossier morganiser-rehearsal-*");
}

#[test]
fn le_dossier_de_travail_est_supprime_apres_une_panique() {
    let base = tempfile::tempdir().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let work = new_work_dir(Some(base.path())).unwrap();
        fs::write(work.path().join(DB_FILE_NAME), b"donnees fictives").unwrap();
        assert!(fs::read_dir(base.path()).unwrap().next().is_some());
        panic!("panique simulée");
    }));
    assert!(result.is_err());
    assert!(dir_is_empty(base.path()), "panique : `Drop` s'exécute pendant le déroulement");
}

const CHILD_BASE_ENV: &str = "MORGANISER_REHEARSAL_CHILD_BASE";
const CHILD_READY: &str = "WORKDIR-READY ";

/// Exécuté uniquement comme sous-processus par le test d'arrêt brutal.
#[test]
#[ignore = "sous-processus du test un_arret_brutal_laisse_le_dossier_de_travail"]
fn sous_processus_cree_un_dossier_de_travail_puis_attend_d_etre_tue() {
    let Some(base) = std::env::var_os(CHILD_BASE_ENV) else { return };
    let work = new_work_dir(Some(Path::new(&base))).unwrap();
    fs::write(work.path().join(DB_FILE_NAME), b"donnees fictives").unwrap();
    println!("{CHILD_READY}{}", work.path().display());
    std::thread::sleep(std::time::Duration::from_secs(60)); // tué avant la fin
}

#[test]
fn un_arret_brutal_laisse_le_dossier_de_travail() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let base = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::rehearsal_tests::sous_processus_cree_un_dossier_de_travail_puis_attend_d_etre_tue",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_BASE_ENV, base.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let line = stdout
        .lines()
        .map_while(Result::ok)
        .find(|line| line.contains(CHILD_READY))
        .expect("le sous-processus n'a pas signalé son dossier de travail");
    let work = PathBuf::from(line.split_once(CHILD_READY).unwrap().1.trim());

    child.kill().unwrap(); // arrêt brutal : aucun `Drop`
    child.wait().unwrap();

    // Limite réelle du nettoyage automatique : le dossier ET les données copiées subsistent.
    assert!(work.join(DB_FILE_NAME).is_file(), "après un arrêt brutal, rien n'a été nettoyé");
    assert!(work.file_name().unwrap().to_string_lossy().starts_with(WORK_PREFIX));
    // Nettoyage ciblé par préfixe (c'est ce que recommande la procédure), jamais `%TEMP%` en bloc.
    let leftovers: Vec<_> = fs::read_dir(base.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(WORK_PREFIX))
        .collect();
    assert_eq!(leftovers.len(), 1);
    fs::remove_dir_all(leftovers[0].path()).unwrap();
    assert!(dir_is_empty(base.path()));
}

// ---------------------------------------------------------------------------------------------
// Répétition sur une copie fournie par le propriétaire (jamais lancée par la suite ordinaire)
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "répétition sur une copie isolée : définir MORGANISER_REHEARSAL_DB, _FROM et _TO (voir l'en-tête du fichier)"]
fn repetition_sur_copie_isolee() {
    // Avec `--nocapture`, la première ligne suivrait « test … ... » : on la passe, pour que
    // chaque ligne utile (dont le marqueur) soit seule sur sa ligne.
    println!();
    let protected = Protected::from_env().unwrap_or_else(|reason| {
        println!("{MARKER_NOT_STARTED}");
        panic!("garde-fous indisponibles, aucun essai : {reason}");
    });
    let outcome = match rehearsal_from_values(
        std::env::var_os(ENV_VAR),
        std::env::var_os(ENV_FROM),
        std::env::var_os(ENV_TO),
        &protected,
    ) {
        Ok(outcome) => outcome,
        Err(reason) => {
            println!("{MARKER_NOT_STARTED}");
            panic!("validation refusée, aucune base créée ni ouverte : {reason}");
        }
    };

    println!("Fichiers source (.db, -wal, -shm) inchangés : {}", outcome.source.describe());
    match &outcome.result {
        Ok(report) => {
            println!("Répétition réussie (aucun texte, chemin ni empreinte affiché).");
            println!("  versions demandées : v{} -> v{}", report.plan.from, report.plan.to);
            println!("  version : v{} -> v{}", report.version_before, report.version_after);
            for (table, count) in &report.tables {
                println!("  {table}: {count} ligne(s) à l'état de départ, projection attendue vérifiée après migration");
            }
            println!("  captures actives : {}", report.live_captures);
            println!(
                "  aller-retour conversion/annulation : {}",
                match report.round_trip {
                    RoundTrip::ExistingCapture => "sur une capture existante de la copie",
                    RoundTrip::FictitiousCapture =>
                        "sur une capture FICTIVE créée puis supprimée dans la copie temporaire",
                }
            );
            println!("  sauvegarde avant migration : vérifiée ({})", report.backup);
            for (suffix, present) in &report.present {
                println!("  {} dans la source : {}", file_label(suffix), if *present { "présent" } else { "absent" });
            }
            println!("  copie temporaire fidèle à la source (SHA-256) : OK");
            println!("  intégrité, clés étrangères, schéma, projection : OK ; restauration : OK");
        }
        Err(error) => println!("Répétition EN ÉCHEC : {error}"),
    }
    // Écrit AVANT les assertions ; `status=OK` seulement si tout a été vérifié.
    println!("{}", marker_line(&outcome));
    assert!(outcome.result.is_ok(), "répétition en échec (voir le message ci-dessus)");
    assert_eq!(outcome.source, SourceCheck::Unchanged, "les fichiers source ne doivent pas changer");
}

// ---------------------------------------------------------------------------------------------
// 003-B1c : répétition v2 → v3 (et chaînage v1 → v3), sur bases FICTIVES uniquement
// ---------------------------------------------------------------------------------------------

use super::rebuild_proof_tests::{seed as seed_v2, v2_cancel, v2_capture, v2_convert};

/// Base v2 fictive, fermée proprement (en général sans `-wal`).
fn fictitious_v2() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    {
        let mut conn = Connection::open(&path).unwrap();
        configure(&conn).unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..2]).unwrap();
        seed_v2(&conn);
    }
    (dir, path)
}

/// Base v2 fictive avec `-wal` et `-shm` non vidés : les transactions validées n'existent que dans
/// `-wal`, comme après un arrêt brutal. Copiée pendant que la connexion d'origine est ouverte.
fn fictitious_v2_with_wal() -> (tempfile::TempDir, PathBuf) {
    let origin = tempfile::tempdir().unwrap();
    let live = origin.path().join(DB_FILE_NAME);
    let mut conn = Connection::open(&live).unwrap();
    configure(&conn).unwrap();
    conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    migrate_with(&mut conn, &MIGRATIONS[..2]).unwrap();
    seed_v2(&conn);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    for suffix in SUFFIXES {
        let from = sibling(&live, suffix);
        if from.is_file() {
            fs::copy(&from, sibling(&path, suffix)).unwrap();
        }
    }
    drop(conn);
    assert!(fs::metadata(sibling(&path, "-wal")).unwrap().len() > 0, "le -wal fictif doit contenir des données");
    (dir, path)
}

/// Base v2 SANS capture active : toutes les captures sont converties ou à la corbeille.
fn fictitious_v2_without_active_capture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    {
        let mut conn = Connection::open(&path).unwrap();
        configure(&conn).unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..2]).unwrap();
        v2_capture(&conn, "a", "Convertie", Some("moi"), 1);
        v2_convert(&conn, "ta", "a", "Convertie", 10);
        v2_capture(&conn, "b", "Annulée puis convertie", Some("finances"), 2);
        v2_convert(&conn, "tb1", "b", "Annulée", 11);
        v2_cancel(&conn, "tb1", 12);
        v2_convert(&conn, "tb2", "b", "Reconvertie", 13);
        v2_capture(&conn, "c", "À la corbeille", None, 3);
        conn.execute("UPDATE inbox_items SET deleted_at = 99 WHERE id = 'c'", []).unwrap();
    }
    (dir, path)
}

/// Base v2 incohérente : une tâche référence une destination qui n'existe pas (clés étrangères
/// désactivées le temps de la corrompre, comme une base ancienne ou réparée à la main).
fn fictitious_v2_inconsistent() -> (tempfile::TempDir, PathBuf) {
    let (dir, path) = fictitious_v2();
    {
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "foreign_keys", false).unwrap();
        conn.execute(
            "INSERT INTO tasks (id, title, details, destination_id, created_at, updated_at)
             VALUES ('orpheline', 'Destination perdue', 'x', 'disparue', 1, 1)",
            [],
        )
        .unwrap();
    }
    (dir, path)
}

const MARKER_V2_V3_OK: &str = "REHEARSAL-RESULT status=OK from=2 to=3 integrity=OK schema=OK projection=OK backup=OK roundtrip=OK restore=OK source=OK";

#[test]
fn v2_vers_v3_projection_conforme_et_source_intacte() {
    let (_dir, path) = fictitious_v2();
    assert!(!sibling(&path, "-wal").exists(), "fermeture propre : pas de -wal");
    let before = SourceSnapshot::capture(&path).unwrap();

    let outcome = rehearse(&path, PLAN_V2_V3);
    assert_unchanged(&outcome);
    assert!(
        outcome.stages.integrity
            && outcome.stages.schema
            && outcome.stages.projection
            && outcome.stages.backup
            && outcome.stages.round_trip
            && outcome.stages.restore
    );
    assert_eq!(marker_line(&outcome), MARKER_V2_V3_OK);
    let report = outcome.result.unwrap();

    assert_eq!((report.version_before, report.version_after), (2, 3));
    assert_eq!(report.plan, PLAN_V2_V3);
    let count = |table: &str| report.tables.iter().find(|t| t.0 == table).unwrap().1;
    assert_eq!((count("destinations"), count("inbox_items"), count("tasks")), (5, 8, 8));
    assert_eq!(report.live_captures, 1); // c8 seule est à organiser
    assert_eq!(report.round_trip, RoundTrip::ExistingCapture);
    assert!(report.backup.starts_with("morganiser-v2-"), "{}", report.backup);
    assert_eq!(report.present, vec![("", true), ("-wal", false), ("-shm", false)]);
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
}

#[test]
fn v2_vers_v3_avec_wal_et_shm_les_transactions_du_wal_sont_reprises_sans_modifier_la_source() {
    let (_dir, path) = fictitious_v2_with_wal();
    let before = SourceSnapshot::capture(&path).unwrap();
    assert!(before.files[1].1.is_some(), "-wal présent");

    let outcome = rehearse(&path, PLAN_V2_V3);
    assert_unchanged(&outcome);
    assert_eq!(marker_line(&outcome), MARKER_V2_V3_OK);
    let report = outcome.result.unwrap();
    // Tout n'existait que dans -wal : les 8 tâches sont bien reprises.
    assert_eq!(report.tables.iter().find(|t| t.0 == "tasks").unwrap().1, 8);
    assert!(report.present[1].1, "-wal signalé présent");
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before, "-wal et -shm intacts, octet pour octet");
}

#[test]
fn copier_le_seul_db_perdrait_les_transactions_du_wal() {
    // Justifie la règle « jamais le .db seul » : la copie du seul fichier principal est incohérente.
    let (_dir, path) = fictitious_v2_with_wal();
    let lonely = tempfile::tempdir().unwrap();
    let copy = lonely.path().join(DB_FILE_NAME);
    fs::copy(&path, &copy).unwrap();
    let conn = Connection::open(&copy).unwrap();
    let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_ne!(version, 2, "le .db seul ne contient pas le schéma v2 validé dans -wal");
    drop(conn);
    // Alors que la répétition, qui copie les trois fichiers ensemble, voit bien la v2.
    assert!(rehearse(&path, PLAN_V2_V3).result.is_ok());
}

#[test]
fn v2_sans_capture_active_l_aller_retour_a_lieu_sur_une_capture_fictive_sans_residu() {
    let (_dir, path) = fictitious_v2_without_active_capture();
    let before = SourceSnapshot::capture(&path).unwrap();
    let outcome = rehearse(&path, PLAN_V2_V3);
    assert_unchanged(&outcome);
    assert!(outcome.stages.round_trip, "l'aller-retour a réellement été exécuté");
    let report = outcome.result.unwrap();
    assert_eq!(report.live_captures, 0);
    assert_eq!(report.round_trip, RoundTrip::FictitiousCapture);
    // L'état de départ est intact : 3 captures, 3 tâches, aucune capture ni tâche fictive restante.
    let count = |table: &str| report.tables.iter().find(|t| t.0 == table).unwrap().1;
    assert_eq!((count("inbox_items"), count("tasks")), (3, 3));
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
    assert_eq!(marker_line(&rehearse(&path, PLAN_V2_V3)), MARKER_V2_V3_OK);
}

#[test]
fn v1_vers_v3_enchaine_les_deux_migrations_avec_une_seule_sauvegarde() {
    let (_dir, path) = fictitious_v1_with_wal();
    let before = SourceSnapshot::capture(&path).unwrap();
    let outcome = rehearse(&path, PLAN_V1_V3);
    assert_unchanged(&outcome);
    assert_eq!(
        marker_line(&outcome),
        "REHEARSAL-RESULT status=OK from=1 to=3 integrity=OK schema=OK projection=OK backup=OK roundtrip=OK restore=OK source=OK"
    );
    let report = outcome.result.unwrap();
    assert_eq!((report.version_before, report.version_after), (1, 3));
    assert!(report.backup.starts_with("morganiser-v1-"));
    let count = |table: &str| report.tables.iter().find(|t| t.0 == table).unwrap().1;
    assert_eq!((count("inbox_items"), count("tasks")), (3, 0));
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
}

#[test]
fn v1_vers_v2_reste_repetable_sans_regression() {
    // Même scénario que la brique 002, avec les versions maintenant explicites.
    let (_dir, path) = fictitious_v1();
    let outcome = rehearse(&path, PLAN_V1_V2);
    assert_unchanged(&outcome);
    assert_eq!(
        marker_line(&outcome),
        "REHEARSAL-RESULT status=OK from=1 to=2 integrity=OK schema=OK projection=OK backup=OK roundtrip=OK restore=OK source=OK"
    );
    assert_eq!(outcome.result.unwrap().version_after, 2);
}

// --- Refus : versions, plans, source ---------------------------------------------------------

#[test]
fn les_plans_non_pris_en_charge_sont_refuses_avant_toute_creation() {
    for (from, to) in [(3, 2), (1, 1), (2, 2), (3, 3), (0, 1), (0, 3), (1, 4), (2, 4), (2, 1), (4, 5)] {
        let error = Plan::new(from, to).unwrap_err();
        assert!(error.contains("non prise en charge"), "v{from} -> v{to} : {error}");
    }
    for plan in SUPPORTED_PLANS {
        assert_eq!(Plan::new(plan.from, plan.to), Ok(plan));
    }
}

#[test]
fn les_versions_sont_explicites_et_une_valeur_invalide_est_un_echec() {
    let some = |s: &str| Some(OsString::from(s));
    assert_eq!(Plan::from_values(some("2"), some("3")), Ok(PLAN_V2_V3));
    assert_eq!(Plan::from_values(some(" 1 "), some("2")), Ok(PLAN_V1_V2));
    for (from, to) in [
        (None, some("3")),
        (some("2"), None),
        (some(""), some("3")),
        (some("deux"), some("3")),
        (some("2"), some("3.0")),
        (some("-1"), some("3")),
        (some("2"), some("trois")),
    ] {
        assert!(Plan::from_values(from.clone(), to.clone()).is_err(), "{from:?} -> {to:?}");
    }
    // Messages sans chemin ni valeur personnelle.
    let message = Plan::from_values(some("2"), None).unwrap_err();
    assert!(message.contains(ENV_TO) && !message.contains('\\'));
}

#[test]
fn une_base_de_mauvaise_version_est_refusee_sans_toucher_la_source() {
    let base = tempfile::tempdir().unwrap();
    // v1 demandée comme v2, v2 demandée comme v1, base déjà en v3 demandée comme v2.
    let (_d1, v1) = fictitious_v1();
    let (_d2, v2) = fictitious_v2();
    let (_d3, v3) = fictitious_v2();
    drop(open(&v3).unwrap());
    for (path, plan, expected) in [
        (&v1, PLAN_V2_V3, "attend une base v2"),
        (&v2, PLAN_V1_V2, "attend une base v1"),
        (&v2, PLAN_V1_V3, "attend une base v1"),
        (&v3, PLAN_V2_V3, "attend une base v2"),
    ] {
        let before = SourceSnapshot::capture(path).unwrap();
        let outcome = rehearse_with(path, plan, &|f, t| fs::copy(f, t), Some(base.path()));
        let error = outcome.result.as_ref().unwrap_err();
        assert!(error.contains(expected), "{error}");
        assert_unchanged(&outcome);
        assert_eq!(SourceSnapshot::capture(path).unwrap(), before);
        assert!(!outcome.stages.integrity && !outcome.stages.projection && !outcome.stages.restore);
        let line = marker_line(&outcome);
        assert!(line.starts_with("REHEARSAL-RESULT status=FAIL"), "{line}");
        assert!(line.contains("projection=FAIL") && line.ends_with("source=OK"), "{line}");
        assert!(dir_is_empty(base.path()), "dossier de travail supprimé");
    }
}

#[test]
fn une_source_incoherente_est_refusee_avant_toute_migration() {
    let base = tempfile::tempdir().unwrap();
    let (_dir, path) = fictitious_v2_inconsistent();
    let before = SourceSnapshot::capture(&path).unwrap();

    let outcome = rehearse_with(&path, PLAN_V2_V3, &|f, t| fs::copy(f, t), Some(base.path()));
    let error = outcome.result.as_ref().unwrap_err();
    assert!(error.contains("source incohérente avant migration"), "{error}");
    assert_unchanged(&outcome);
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
    assert!(!outcome.stages.integrity, "aucune migration tentée, donc aucune étape validée");
    assert!(marker_line(&outcome).starts_with("REHEARSAL-RESULT status=FAIL from=2 to=3 integrity=FAIL"));
    assert!(dir_is_empty(base.path()));
}

// --- Pannes et divergences -------------------------------------------------------------------

#[test]
fn une_panne_en_cours_de_migration_ne_valide_rien_et_laisse_la_source_intacte() {
    let (_dir, path) = fictitious_v2_with_wal();
    let before = SourceSnapshot::capture(&path).unwrap();
    let base = tempfile::tempdir().unwrap();
    // Migration 0003 de production, interrompue AVANT la restauration des liens.
    let production = MIGRATIONS[2];
    let cut = production.find("-- @stage: restore").expect("marqueur d'étape");
    let failing = format!("{}\nSELECT * FROM panne_volontaire_inexistante;\n", &production[..cut]);
    let options = Options {
        base: Some(base.path()),
        migrations: Some(vec![MIGRATIONS[0].to_string(), MIGRATIONS[1].to_string(), failing]),
        ..Options::default()
    };

    let outcome = rehearse_options(&path, PLAN_V2_V3, &options);
    let error = outcome.result.as_ref().unwrap_err();
    assert!(error.starts_with("migration :"), "{error}");
    assert_unchanged(&outcome);
    assert_eq!(SourceSnapshot::capture(&path).unwrap(), before);
    assert!(!outcome.stages.integrity && !outcome.stages.projection && !outcome.stages.backup);
    assert!(marker_line(&outcome).contains("status=FAIL"));
    assert!(dir_is_empty(base.path()));
}

fn run_with_mutation(mutate: impl Fn(&Connection)) -> Outcome {
    let (_dir, path) = fictitious_v2();
    let base = tempfile::tempdir().unwrap();
    let options = Options { base: Some(base.path()), mutate_after_migration: Some(&mutate), ..Options::default() };
    let outcome = rehearse_options(&path, PLAN_V2_V3, &options);
    assert_unchanged(&outcome);
    assert!(dir_is_empty(base.path()));
    outcome
}

#[test]
fn une_valeur_modifiee_apres_migration_est_detectee_sans_reveler_de_texte() {
    // Description d'une tâche altérée : la migration « termine sans erreur », la comparaison échoue.
    let outcome = run_with_mutation(|conn| {
        conn.execute(
            "UPDATE tasks SET details = details || ' ALTÉRÉ', updated_at = updated_at + 1 WHERE id = 't1'",
            [],
        )
        .unwrap();
    });
    let error = outcome.result.as_ref().unwrap_err();
    assert!(error.starts_with("projection non conforme"), "{error}");
    assert!(error.contains("tasks#t1.details") && error.contains("tasks#t1.updated_at"), "{error}");
    // Jamais le contenu : ni le texte d'origine ni la valeur altérée.
    for secret in ["Appeler la banque", "Demander le relevé", "ALTÉRÉ"] {
        assert!(!error.contains(secret), "le message ne doit contenir aucun texte : {error}");
    }
    assert!(outcome.stages.integrity && outcome.stages.schema && !outcome.stages.projection);
    let line = marker_line(&outcome);
    assert!(line.contains("integrity=OK schema=OK projection=FAIL") && line.contains("status=FAIL"), "{line}");
}

#[test]
fn un_mauvais_classement_un_lien_perdu_et_une_ligne_supprimee_sont_detectes() {
    let wrong_dimension = run_with_mutation(|conn| {
        // t1 était une rubrique « finances » : la placer dans l'univers est une erreur de projection.
        conn.execute(
            "UPDATE tasks SET section_id = NULL, responsibility_id = 'moi', updated_at = updated_at + 1
             WHERE id = 't1'",
            [],
        )
        .unwrap();
    });
    let error = wrong_dimension.result.unwrap_err();
    assert!(error.contains("tasks#t1.responsibility_id") && error.contains("tasks#t1.section_id"), "{error}");

    let lost_link = run_with_mutation(|conn| {
        conn.execute("UPDATE inbox_items SET converted_task_id = NULL WHERE id = 'c1'", []).unwrap();
    });
    assert!(lost_link.result.unwrap_err().contains("inbox_items.liens#c1.converted_task_id"));

    let lost_row = run_with_mutation(|conn| {
        conn.execute("UPDATE inbox_items SET converted_task_id = NULL WHERE id = 'c4'", []).unwrap();
        conn.execute("DELETE FROM tasks WHERE id = 't4'", []).unwrap();
    });
    assert!(lost_row.result.unwrap_err().contains("tasks#t4.ligne absente"));

    let invented_default = run_with_mutation(|conn| {
        // « Moi » attribué à une tâche non classée : exactement l'erreur à ne jamais commettre.
        conn.execute(
            "UPDATE tasks SET responsibility_id = 'moi', updated_at = updated_at + 1 WHERE id = 't4'",
            [],
        )
        .unwrap();
    });
    assert!(invented_default.result.unwrap_err().contains("tasks#t4.responsibility_id"));
}

#[test]
fn une_source_v2_modifiee_pendant_la_copie_est_refusee() {
    let (_dir, path) = fictitious_v2();
    let base = tempfile::tempdir().unwrap();
    let racing = |from: &Path, to: &Path| -> io::Result<u64> {
        let copied = fs::copy(from, to)?;
        let mut bytes = fs::read(from)?;
        bytes[200] ^= 0xFF;
        fs::write(from, bytes)?;
        Ok(copied)
    };
    let outcome = rehearse_with(&path, PLAN_V2_V3, &racing, Some(base.path()));
    assert!(outcome.result.as_ref().unwrap_err().contains("changé pendant la copie"));
    assert_eq!(outcome.source, SourceCheck::Different);
    assert!(marker_line(&outcome).ends_with("source=DIFFERENT"));
    assert!(dir_is_empty(base.path()));
}

// --- La projection attendue et la comparaison ------------------------------------------------

#[test]
fn la_projection_attendue_est_calculee_en_rust_selon_la_nature_des_destinations() {
    let (_dir, path) = fictitious_v2();
    let conn = Connection::open(&path).unwrap();
    let before = read_snapshot(&conn, 2).unwrap();
    let expected = expected_after(&before, 3).unwrap();
    assert_eq!(expected.version, 3);

    let column = |name: &str| TASK_COLUMNS_V3.iter().position(|c| *c == name).unwrap();
    let task = |id: &str| expected.tasks.iter().find(|r| r[0] == Value::Text(id.into())).unwrap();
    let text = |s: &str| Value::Text(s.into());
    // t1 : rubrique finances ; t2 : univers « externe » (archivé) ; t6 : rubrique « inventaire »
    // (archivée) ; t4 : non classée ; tâche directe : univers « moi ».
    assert_eq!((&task("t1")[column("responsibility_id")], &task("t1")[column("section_id")]), (&Value::Null, &text("finances")));
    assert_eq!((&task("t2")[column("responsibility_id")], &task("t2")[column("section_id")]), (&text("externe"), &Value::Null));
    assert_eq!((&task("t6")[column("responsibility_id")], &task("t6")[column("section_id")]), (&Value::Null, &text("inventaire")));
    assert_eq!((&task("t4")[column("responsibility_id")], &task("t4")[column("section_id")]), (&Value::Null, &Value::Null));
    assert_eq!((&task("directe-1")[column("responsibility_id")], &task("directe-1")[column("section_id")]), (&text("moi"), &Value::Null));
    // Colonnes nouvelles : jamais inventées.
    for row in &expected.tasks {
        for name in ["start_date", "due_date", "completed_on", "priority"] {
            assert_eq!(row[column(name)], Value::Null, "{name}");
        }
    }
    // Description, titre, statut, provenance et dates identiques à la v2.
    let old = before.tasks.iter().find(|r| r[0] == text("t3")).unwrap();
    let new = task("t3");
    assert_eq!((&new[1], &new[2], &new[3]), (&old[1], &old[2], &old[3]));
    assert_eq!((&new[column("origin_inbox_item_id")], &new[column("deleted_at")]), (&old[5], &old[8]));
    // Les liens et les captures sont repris tels quels.
    assert_eq!((&expected.links, &expected.captures), (&before.links, &before.captures));
}

#[test]
fn une_destination_inconnue_rend_la_projection_impossible_plutot_que_de_deviner() {
    let (_dir, path) = fictitious_v2_inconsistent();
    let conn = Connection::open(&path).unwrap();
    let before = read_snapshot(&conn, 2).unwrap();
    assert!(expected_after(&before, 3).is_err());
}

#[test]
fn la_comparaison_decrit_les_divergences_sans_contenu() {
    let text = |s: &str| Value::Text(s.into());
    let base = Snapshot {
        version: 3,
        destinations: vec![vec![text("moi"), text("Moi"), text("responsibility"), Value::Integer(10), Value::Null]],
        captures: vec![],
        links: vec![],
        tasks: vec![
            vec![text("a"), text("Titre SECRET"), text("Texte SECRET"), text("todo")],
            vec![text("b"), text("B"), text("b"), text("todo")],
        ],
    };
    assert!(compare(&base, &base.clone()).is_empty());

    let mut changed = base.clone();
    changed.tasks[0][2] = text("Autre texte SECRET");
    changed.tasks.remove(1);
    changed.tasks.push(vec![text("c"), text("C"), text("c"), text("todo")]);
    let divergences = compare(&base, &changed);
    let fields: Vec<(String, String)> = divergences.iter().map(|d| (d.id.clone(), d.field.clone())).collect();
    assert!(fields.contains(&("a".into(), "details".into())));
    assert!(fields.contains(&("b".into(), "ligne absente".into())));
    assert!(fields.contains(&("c".into(), "ligne inattendue".into())));
    let shown = describe(&divergences);
    assert!(shown.starts_with("3 divergence(s)") && !shown.contains("SECRET"), "{shown}");

    // Une différence de version est signalée comme telle.
    let mut other_version = base.clone();
    other_version.version = 2;
    assert_eq!(compare(&base, &other_version)[0].table, "schema");
}

#[test]
fn un_schema_incomplet_ou_residuel_est_refuse() {
    let (_dir, path) = fictitious_v2();
    let conn = Connection::open(&path).unwrap();
    assert!(check_schema(&conn, 2).is_ok());
    assert!(check_schema(&conn, 3).unwrap_err().contains("version de schéma v2"));
    conn.execute_batch("DROP TRIGGER tasks_origin_is_immutable").unwrap();
    assert!(check_schema(&conn, 2).unwrap_err().contains("tasks_origin_is_immutable"));
    conn.execute_batch("CREATE TEMP TABLE residuelle (x)").unwrap();
    // L'objet manquant est signalé en premier ; une fois restauré, le résidu temporaire l'est aussi.
    conn.execute_batch(
        "CREATE TRIGGER tasks_origin_is_immutable BEFORE UPDATE OF id ON tasks BEGIN SELECT 1; END;",
    )
    .unwrap();
    assert!(check_schema(&conn, 2).unwrap_err().contains("temporaire"));
}

#[test]
fn la_repetition_v2_v3_ne_depend_d_aucun_chemin_personnel_dans_ses_messages() {
    let (_dir, path) = fictitious_v2_inconsistent();
    let outcome = rehearse(&path, PLAN_V2_V3);
    let error = outcome.result.unwrap_err();
    assert!(!error.contains(&*path.to_string_lossy()), "{error}");
    assert!(!error.contains("Destination perdue"), "{error}");
    assert_fixed_vocabulary(&marker_line(&rehearse(&path, PLAN_V2_V3)));
}

#[test]
fn une_etape_obligatoire_manquante_interdit_status_ok() {
    // Résultat « réussi » et source intacte, mais UNE étape non validée : l'échec est global.
    let (_dir, path) = fictitious_v2();
    let good = rehearse(&path, PLAN_V2_V3);
    assert!(good.stages.all());
    let report = || good.result.as_ref().unwrap();
    for missing in 0..6 {
        let mut stages = good.stages;
        match missing {
            0 => stages.integrity = false,
            1 => stages.schema = false,
            2 => stages.projection = false,
            3 => stages.backup = false,
            4 => stages.round_trip = false,
            _ => stages.restore = false,
        }
        let outcome = Outcome {
            plan: good.plan,
            result: Ok(Report {
                plan: report().plan,
                version_before: report().version_before,
                version_after: report().version_after,
                tables: report().tables.clone(),
                live_captures: report().live_captures,
                round_trip: report().round_trip,
                backup: report().backup.clone(),
                present: report().present.clone(),
            }),
            source: SourceCheck::Unchanged,
            stages,
        };
        let line = marker_line(&outcome);
        assert!(line.starts_with("REHEARSAL-RESULT status=FAIL"), "étape {missing} : {line}");
        assert_fixed_vocabulary(&line);
    }
}
