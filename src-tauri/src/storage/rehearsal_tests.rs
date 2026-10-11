//! Répétition de la migration v1 → v2 sur une COPIE ISOLÉE d'une base.
//!
//! Deux usages :
//! - tests ordinaires : le dispositif et ses garde-fous sont vérifiés sur des bases **fictives**
//!   créées à la volée, dans des dossiers temporaires ;
//! - test `#[ignore]` : le propriétaire fournit, par variable d'environnement, le chemin d'une
//!   copie qu'il a faite lui-même (application fermée, `.db` + `-wal` + `-shm` ensemble) :
//!
//!   ```text
//!   $env:MORGANISER_REHEARSAL_DB = "C:\MOrganiserEssais\dev-AAAA-MM-JJ\morganiser.db"
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
/// Préfixe du dossier de travail : permet un nettoyage ciblé.
const WORK_PREFIX: &str = "morganiser-rehearsal-";
/// Fichiers qui forment ensemble la base (`""` = `morganiser.db`).
const SUFFIXES: [&str; 3] = ["", "-wal", "-shm"];

const V1_TABLES: [(&str, &str); 2] = [
    ("destinations", "SELECT id, label, kind, position, archived_at FROM destinations ORDER BY id"),
    (
        "inbox_items",
        "SELECT id, content, destination_id, created_at, updated_at, deleted_at FROM inbox_items ORDER BY id",
    ),
];

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
// Empreintes logiques des lignes SQLite (comparaison avant/après migration)
// ---------------------------------------------------------------------------------------------

/// Empreinte FNV-1a de lignes (valeurs typées) : sert à comparer deux états logiques ; elle n'est
/// ni affichée ni conservée.
fn fingerprint(conn: &Connection, sql: &str) -> (usize, u64) {
    let mut stmt = conn.prepare(sql).unwrap();
    let columns = stmt.column_count();
    let rows = stmt
        .query_map([], |row| (0..columns).map(|i| row.get::<_, Value>(i)).collect::<Result<Vec<_>, _>>())
        .unwrap();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut count = 0;
    for row in rows {
        for byte in format!("{:?}", row.unwrap()).bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        count += 1;
    }
    (count, hash)
}

fn v1_fingerprints(conn: &Connection) -> Vec<(&'static str, usize, u64)> {
    V1_TABLES
        .iter()
        .map(|(name, sql)| {
            let (count, hash) = fingerprint(conn, sql);
            (*name, count, hash)
        })
        .collect()
}

fn single(conn: &Connection, sql: &str) -> String {
    conn.query_row(sql, [], |row| row.get::<_, String>(0)).unwrap()
}

// ---------------------------------------------------------------------------------------------
// La répétition
// ---------------------------------------------------------------------------------------------

/// L'aller-retour conversion/annulation n'est JAMAIS déclaré sans avoir eu lieu : sur une capture
/// existante de la copie, ou, si elle n'en contient aucune, sur une capture FICTIVE créée puis
/// supprimée dans la copie temporaire (le contenu de référence est comparé ensuite).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum RoundTrip {
    ExistingCapture,
    FictitiousCapture,
}

#[derive(Debug)]
struct Report {
    version_before: u32,
    version_after: u32,
    tables: Vec<(&'static str, usize, u64)>,
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
    backup: bool,
    round_trip: bool,
    restore: bool,
}

#[derive(Debug)]
struct Outcome {
    result: Result<Report, String>,
    source: SourceCheck,
    stages: Stages,
}

type Copier<'a> = &'a dyn Fn(&Path, &Path) -> io::Result<u64>;

fn new_work_dir(base: Option<&Path>) -> io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(WORK_PREFIX);
    match base {
        Some(base) => builder.tempdir_in(base),
        None => builder.tempdir(),
    }
}

/// Répétition complète sur une copie temporaire de `source_db`.
fn rehearse(source_db: &Path) -> Outcome {
    rehearse_with(source_db, &|from, to| fs::copy(from, to), None)
}

/// `copier` et `base` sont injectables pour simuler des pannes et observer le nettoyage.
fn rehearse_with(source_db: &Path, copier: Copier<'_>, base: Option<&Path>) -> Outcome {
    let before = match SourceSnapshot::capture(source_db) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Outcome {
                result: Err(format!("lecture de la source impossible : {error}")),
                source: SourceCheck::NotChecked,
                stages: Stages::default(),
            }
        }
    };
    let mut stages = Stages::default();
    let result = run(source_db, &before, copier, base, &mut stages);
    // Contrôle final de la source, y compris quand `run` a échoué.
    let source = match SourceSnapshot::capture(source_db) {
        Ok(after) if after == before => SourceCheck::Unchanged,
        Ok(_) => SourceCheck::Different,
        Err(error) => SourceCheck::Unverifiable(error),
    };
    Outcome { result, source, stages }
}

fn run(
    source_db: &Path,
    before: &SourceSnapshot,
    copier: Copier<'_>,
    base: Option<&Path>,
    stages: &mut Stages,
) -> Result<Report, String> {
    // `work` est déclaré en premier : il est supprimé en dernier, après la fermeture des connexions.
    let work = new_work_dir(base).map_err(|e| format!("dossier de travail : {e}"))?;
    let db = work.path().join(DB_FILE_NAME);

    // 1. Copie des fichiers présents (sous le MÊME nom pour que `-wal` reste apparié).
    for (suffix, identity) in &before.files {
        if identity.is_none() {
            continue;
        }
        let target = sibling(&db, suffix);
        copier(&sibling(source_db, suffix), &target)
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

    // 2. État v1 de référence (lu sur la copie ; la fermeture reporte -wal dans la copie).
    let (version_before, reference) = {
        let conn = Connection::open(&db).map_err(|e| e.to_string())?;
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if version != 1 {
            return Err(format!("base en version {version}, la répétition attend une base v1"));
        }
        (version, v1_fingerprints(&conn))
    };

    // 3. Migration réelle (sauvegarde automatique vérifiée, puis v2).
    let conn = open(&db).map_err(|e| format!("migration : {e}"))?;
    let version_after: u32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if v1_fingerprints(&conn) != reference {
        return Err("les captures ou destinations ont changé pendant la migration".into());
    }
    if single(&conn, "PRAGMA integrity_check") != "ok" {
        return Err("integrity_check en échec après migration".into());
    }
    let broken: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if broken != 0 {
        return Err(format!("{broken} clé(s) étrangère(s) cassée(s)"));
    }
    stages.integrity = true;

    // 4. Sauvegarde produite : une seule, en v1, au contenu de référence.
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
        let v: u32 = copy.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
        if v != 1 || v1_fingerprints(&copy) != reference {
            return Err("la sauvegarde ne correspond pas à l'état v1 de référence".into());
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
    for _ in 0..2 {
        let current = inbox::get_item(&conn, &target_id).map_err(|e| e.to_string())?;
        stamp += 1;
        let done = tasks::convert_item(&conn, &target_id, current.updated_at, None, stamp)
            .map_err(|e| format!("conversion : {e}"))?;
        stamp += 1;
        tasks::cancel_conversion(&conn, &done.task.id, stamp).map_err(|e| format!("annulation : {e}"))?;
    }
    if round_trip == RoundTrip::FictitiousCapture {
        // Aucun résidu : tâches d'abord (clé étrangère), puis la capture fictive elle-même.
        conn.execute("DELETE FROM tasks WHERE origin_inbox_item_id = ?1", [&target_id])
            .map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM inbox_items WHERE id = ?1", [&target_id])
            .map_err(|e| e.to_string())?;
    }
    if v1_fingerprints(&conn) != reference {
        return Err("l'aller-retour conversion/annulation a modifié des captures".into());
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
    let restored = open(&db).map_err(|e| format!("réouverture après restauration : {e}"))?;
    if v1_fingerprints(&restored) != reference {
        return Err("la base restaurée diffère de la référence".into());
    }
    stages.restore = true;
    let backup_name = backups[0].file_name().unwrap().to_string_lossy().into_owned();
    drop(restored);

    Ok(Report {
        version_before,
        version_after,
        tables: reference,
        live_captures,
        round_trip,
        backup: backup_name,
        present: before.files.iter().map(|(s, identity)| (*s, identity.is_some())).collect(),
    })
}

/// Ligne-marqueur lue par la procédure PowerShell. Vocabulaire FIXE : aucun chemin, aucune
/// empreinte, aucun texte, aucun message d'erreur ; chaque valeur n'est `OK` que si l'étape
/// correspondante a réellement été vérifiée.
fn marker_line(outcome: &Outcome) -> String {
    let ok_or_fail = |ok: bool| if ok { "OK" } else { "FAIL" };
    let (from, to) = match &outcome.result {
        Ok(report) => (report.version_before.to_string(), report.version_after.to_string()),
        Err(_) => ("?".to_string(), "?".to_string()),
    };
    let source = match &outcome.source {
        SourceCheck::Unchanged => "OK",
        SourceCheck::Different => "DIFFERENT",
        SourceCheck::Unverifiable(_) => "UNVERIFIABLE",
        SourceCheck::NotChecked => "NOTCHECKED",
    };
    let status = outcome.result.is_ok() && outcome.source == SourceCheck::Unchanged;
    let stages = outcome.stages;
    format!(
        "REHEARSAL-RESULT status={} from={from} to={to} integrity={} backup={} roundtrip={} restore={} source={source}",
        ok_or_fail(status),
        ok_or_fail(stages.integrity),
        ok_or_fail(stages.backup),
        ok_or_fail(stages.round_trip),
        ok_or_fail(stages.restore),
    )
}

/// Marqueur d'un échec avant toute répétition (variable, garde-fous ou validation).
const MARKER_NOT_STARTED: &str = "REHEARSAL-RESULT status=FAIL from=? to=? integrity=FAIL backup=FAIL roundtrip=FAIL restore=FAIL source=NOTCHECKED";

/// Validation puis répétition : rien n'est créé ni ouvert si la validation échoue.
fn rehearsal_from_value(raw: Option<OsString>, protected: &Protected) -> Result<Outcome, String> {
    let path = resolve_source(raw, protected)?;
    Ok(rehearse(&path))
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

    let outcome = rehearse(&path);
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

        let outcome = rehearse(&path);
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
            marker_line(&rehearse(&path)),
            "REHEARSAL-RESULT status=OK from=1 to=2 integrity=OK backup=OK roundtrip=OK restore=OK source=OK"
        );
    }
}

#[test]
fn avec_des_captures_actives_l_aller_retour_utilise_une_capture_existante() {
    let (_dir, path) = fictitious_v1();
    let report = rehearse(&path).result.unwrap();
    assert_eq!(report.round_trip, RoundTrip::ExistingCapture);
}

#[test]
fn le_roundtrip_n_est_jamais_declare_quand_il_n_a_pas_eu_lieu() {
    // Échec AVANT l'aller-retour (base déjà migrée) : roundtrip jamais « OK ».
    let (_dir, path) = fictitious_v1_without_active_capture(false);
    drop(open(&path).unwrap());
    let outcome = rehearse(&path);
    assert!(outcome.result.is_err());
    assert!(!outcome.stages.round_trip);
    assert!(marker_line(&outcome).contains("roundtrip=FAIL"));
}

#[test]
fn la_repetition_fonctionne_avec_wal_et_shm_et_ne_les_modifie_pas() {
    let (_dir, path) = fictitious_v1_with_wal();
    let before = SourceSnapshot::capture(&path).unwrap();
    assert!(before.files[1].1.is_some(), "-wal présent");

    let outcome = rehearse(&path);
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

    let outcome = rehearse(&path);
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
    drop(open(&path).unwrap()); // migrée en v2 par l'application fictive
    let before = SourceSnapshot::capture(&path).unwrap();
    let outcome = rehearse(&path);
    assert!(outcome.result.as_ref().unwrap_err().contains("version 2"));
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
    let outcome = rehearse_with(&path, &failing, Some(base.path()));

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
    let outcome = rehearse_with(&path, &racing, Some(base.path()));
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
    let outcome = rehearse_with(&path, &corrupting, Some(base.path()));
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
    let outcome = rehearse_with(&as_dir, &|f, t| fs::copy(f, t), Some(base.path()));
    assert!(outcome.result.is_err());
    assert_eq!(outcome.source, SourceCheck::NotChecked);
    assert!(dir_is_empty(base.path()), "aucun dossier de travail créé");
}

#[test]
fn les_empreintes_logiques_distinguent_deux_contenus_et_reconnaissent_deux_copies() {
    // Ces empreintes (FNV, lignes SQLite) servent à comparer l'état logique avant/après migration ;
    // elles sont distinctes des SHA-256 de fichiers.
    let (_dir_a, a) = fictitious_v1();
    let (_dir_b, b) = fictitious_v1();
    let conn_a = Connection::open(&a).unwrap();
    let conn_b = Connection::open(&b).unwrap();
    assert_eq!(v1_fingerprints(&conn_a), v1_fingerprints(&conn_b));
    conn_b.execute("UPDATE inbox_items SET content = content || ' ' WHERE id = 'f0'", []).unwrap();
    assert_ne!(v1_fingerprints(&conn_a), v1_fingerprints(&conn_b));
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
            "status" | "integrity" | "backup" | "roundtrip" | "restore" => &["OK", "FAIL"],
            "from" | "to" => &["1", "2", "?"],
            "source" => &["OK", "DIFFERENT", "UNVERIFIABLE", "NOTCHECKED"],
            other => panic!("clé inattendue : {other}"),
        };
        assert!(allowed.contains(&value), "{key}={value}");
    }
}

#[test]
fn le_marqueur_de_reussite_est_exact_et_a_vocabulaire_fixe() {
    let (_dir, path) = fictitious_v1_with_wal();
    let line = marker_line(&rehearse(&path));
    assert_eq!(
        line,
        "REHEARSAL-RESULT status=OK from=1 to=2 integrity=OK backup=OK roundtrip=OK restore=OK source=OK"
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
    let line = marker_line(&rehearse_with(&path, &failing, Some(base.path())));
    assert_eq!(
        line,
        "REHEARSAL-RESULT status=FAIL from=? to=? integrity=FAIL backup=FAIL roundtrip=FAIL restore=FAIL source=OK"
    );
    assert!(!line.contains("secret") && !line.contains('\\'), "{line}");
    assert_fixed_vocabulary(&line);

    // Base déjà migrée : refus avant la migration, source intacte.
    let (_d2, migrated) = fictitious_v1();
    drop(open(&migrated).unwrap());
    let line = marker_line(&rehearse(&migrated));
    assert!(line.starts_with("REHEARSAL-RESULT status=FAIL from=? to=? integrity=FAIL"), "{line}");
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
    let line = marker_line(&rehearse_with(&racing_path, &racing, Some(base.path())));
    assert!(line.starts_with("REHEARSAL-RESULT status=FAIL"), "{line}");
    assert!(line.ends_with("source=DIFFERENT"), "{line}");
    assert_fixed_vocabulary(&line);

    // Source illisible : rien n'a démarré.
    let dir = tempfile::tempdir().unwrap();
    let as_dir = dir.path().join(DB_FILE_NAME);
    fs::create_dir(&as_dir).unwrap();
    let line = marker_line(&rehearse(&as_dir));
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
    let outcome = rehearse_with(&path, &|f, t| fs::copy(f, t), Some(base.path()));
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
#[ignore = "répétition sur une copie isolée : définir MORGANISER_REHEARSAL_DB (voir l'en-tête du fichier)"]
fn repetition_sur_copie_isolee() {
    // Avec `--nocapture`, la première ligne suivrait « test … ... » : on la passe, pour que
    // chaque ligne utile (dont le marqueur) soit seule sur sa ligne.
    println!();
    let protected = Protected::from_env().unwrap_or_else(|reason| {
        println!("{MARKER_NOT_STARTED}");
        panic!("garde-fous indisponibles, aucun essai : {reason}");
    });
    let outcome = match rehearsal_from_value(std::env::var_os(ENV_VAR), &protected) {
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
            println!("  version : v{} -> v{}", report.version_before, report.version_after);
            for (table, count, _) in &report.tables {
                println!("  {table}: {count} ligne(s), contenu identique à la référence après migration");
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
            println!("  conversion/annulation/reconversion : OK ; restauration : OK");
        }
        Err(error) => println!("Répétition EN ÉCHEC : {error}"),
    }
    // Écrit AVANT les assertions ; `status=OK` seulement si tout a été vérifié.
    println!("{}", marker_line(&outcome));
    assert!(outcome.result.is_ok(), "répétition en échec (voir le message ci-dessus)");
    assert_eq!(outcome.source, SourceCheck::Unchanged, "les fichiers source ne doivent pas changer");
}
