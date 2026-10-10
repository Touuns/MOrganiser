//! Sauvegarde cohérente de la base AVANT une migration d'une base existante.
//!
//! Principes :
//! - **Jamais de copie de fichier** : la base est en mode WAL, donc `morganiser.db` seul peut
//!   être incomplet (voir la fiche brique 001, section 7). La copie est produite par SQLite
//!   lui-même (`VACUUM INTO`), depuis un instantané cohérent qui inclut `-wal`, base ouverte
//!   ou non. Le résultat est un fichier autonome, sans `-wal` ni `-shm`.
//! - **Fichier temporaire, puis finalisation** : l'écriture se fait dans `*.db.partial`. Le
//!   fichier n'est renommé qu'après vérification complète. Un `*.partial` trouvé au démarrage
//!   est un reste d'interruption : il est supprimé et n'est jamais compté comme sauvegarde.
//! - **Aucun écrasement** : un nom déjà pris reçoit un suffixe, jamais de remplacement.
//! - **Échec = pas de migration** : l'appelant (`storage::open`) refuse de migrer.
//! - **Conservation** : les trois sauvegardes vérifiées les plus récentes ; le nettoyage
//!   n'a lieu qu'APRÈS le succès de la nouvelle sauvegarde. Un fichier illisible n'est pas
//!   compté et n'est jamais supprimé automatiquement.
//!
//! Restauration (application FERMÉE ; ne jamais copier `.db` seul) :
//! 1. déplacer — pas supprimer — `morganiser.db`, `morganiser.db-wal` et `morganiser.db-shm`
//!    dans un dossier de mise de côté ;
//! 2. copier la sauvegarde choisie (`backups/morganiser-v<N>-<horodatage>.db`) sous le nom
//!    `morganiser.db` dans le dossier de données ;
//! 3. lancer l'application : elle rouvre la base, la migre si besoin (avec une nouvelle
//!    sauvegarde) et la remet en mode WAL.
//!    Le test `restauration_depuis_une_sauvegarde` exécute exactement ces étapes.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags};

/// Sous-dossier des sauvegardes, dans le dossier de données de l'environnement.
pub const BACKUP_DIR: &str = "backups";
/// Nombre de sauvegardes vérifiées conservées.
pub const BACKUPS_KEPT: usize = 3;

const PREFIX: &str = "morganiser-v";
const EXTENSION: &str = ".db";
const PARTIAL_SUFFIX: &str = ".partial";

/// Dossier des sauvegardes associé à un fichier de base.
pub fn backup_dir(db_path: &Path) -> PathBuf {
    db_path.parent().unwrap_or_else(|| Path::new(".")).join(BACKUP_DIR)
}

/// Essais maximum quand une AUTRE connexion modifie la base pendant la copie.
const MAX_ATTEMPTS: u32 = 3;

/// Crée, vérifie et finalise la sauvegarde d'une base de version `from_version`.
/// Renvoie le chemin de la sauvegarde ; en cas d'erreur, aucune sauvegarde n'est créée et
/// celles qui existent sont intactes.
pub fn before_migration(
    source: &Connection,
    db_path: &Path,
    from_version: u32,
    now_ms: i64,
) -> Result<PathBuf, String> {
    before_migration_with(source, db_path, from_version, now_ms, &mut |_| {})
}

/// Comme `before_migration`. `after_copy(essai)` est appelé juste après la copie, avant sa
/// vérification : hors tests, il ne fait rien ; les tests y simulent une écriture concurrente.
pub(super) fn before_migration_with(
    source: &Connection,
    db_path: &Path,
    from_version: u32,
    now_ms: i64,
    after_copy: &mut dyn FnMut(u32),
) -> Result<PathBuf, String> {
    let dir = backup_dir(db_path);
    fs::create_dir_all(&dir).map_err(|e| format!("dossier des sauvegardes : {e}"))?;
    remove_interrupted_leftovers(&dir)?;

    let final_path = unique_final_path(&dir, from_version, now_ms);
    let partial = PathBuf::from(format!("{}{PARTIAL_SUFFIX}", final_path.display()));

    for attempt in 1..=MAX_ATTEMPTS {
        let _ = fs::remove_file(&partial); // cible neuve à chaque essai
        match write_and_verify(source, &partial, from_version, attempt, after_copy) {
            Ok(Verified::Complete) => break,
            // La source a changé PENDANT la copie : la comparaison de contenu n'a pas de sens.
            // On reprend une copie neuve ; au dernier essai, la copie (instantané cohérent par
            // construction, et contrôlée en interne) est acceptée sans comparaison.
            Ok(Verified::SourceChanged) if attempt < MAX_ATTEMPTS => continue,
            Ok(Verified::SourceChanged) => {
                eprintln!(
                    "[M'Organiser] la base a été modifiée pendant la sauvegarde : copie acceptée \
                     après contrôles d'intégrité, sans comparaison ligne à ligne"
                );
                break;
            }
            Err(error) => {
                let _ = fs::remove_file(&partial);
                return Err(error);
            }
        }
    }
    if final_path.exists() {
        // Ne devrait pas arriver (nom unique choisi plus haut) : jamais d'écrasement.
        let _ = fs::remove_file(&partial);
        return Err("une sauvegarde de même nom existe déjà".into());
    }
    fs::rename(&partial, &final_path).map_err(|e| format!("finalisation : {e}"))?;

    // Seulement maintenant que la nouvelle sauvegarde est en place et vérifiée.
    if let Err(error) = prune(&dir, BACKUPS_KEPT) {
        eprintln!("[M'Organiser] nettoyage des anciennes sauvegardes impossible : {error}");
    }
    Ok(final_path)
}

/// Ce que la vérification a pu établir.
enum Verified {
    /// Copie intègre ET identique à la source (qui est restée stable pendant la copie).
    Complete,
    /// Copie intègre ; la source a été modifiée par une autre connexion pendant l'opération :
    /// la comparaison avec la source n'est alors pas applicable.
    SourceChanged,
}

fn data_version(conn: &Connection) -> Result<i64, String> {
    // Change quand une AUTRE connexion (ou un autre processus) valide une écriture.
    conn.query_row("PRAGMA data_version", [], |row| row.get(0))
        .map_err(|e| format!("data_version : {e}"))
}

fn write_and_verify(
    source: &Connection,
    partial: &Path,
    from_version: u32,
    attempt: u32,
    after_copy: &mut dyn FnMut(u32),
) -> Result<Verified, String> {
    let before = data_version(source)?;
    vacuum_into(source, partial)?;
    after_copy(attempt);

    // 1. Contrôles propres à la copie : valables quoi qu'il arrive à la source.
    let copy = verify_copy(partial, from_version)?;

    // 2. Comparaison avec la source : seulement si celle-ci n'a pas évolué depuis l'instantané.
    if data_version(source)? != before {
        return Ok(Verified::SourceChanged);
    }
    match compare_content(source, &copy) {
        Ok(()) => Ok(Verified::Complete),
        // Une écriture concurrente survenue PENDANT la comparaison explique l'écart.
        Err(_) if data_version(source)? != before => Ok(Verified::SourceChanged),
        Err(error) => Err(error),
    }
}

/// Copie cohérente produite par SQLite. La cible ne doit pas exister (SQLite refuse sinon).
fn vacuum_into(source: &Connection, target: &Path) -> Result<(), String> {
    let target = target.to_str().ok_or("chemin de sauvegarde non UTF-8")?;
    source
        .execute("VACUUM INTO ?1", [target])
        .map(|_| ())
        .map_err(|e| format!("copie SQLite : {e}"))
}

/// Contrôles : intégrité, clés étrangères, version du schéma, puis contenu identique à la
/// source table par table, ligne par ligne. (Utilisé tel quel quand la source est au repos.)
#[cfg(test)]
fn verify(source: &Connection, backup: &Path, expected_version: u32) -> Result<(), String> {
    let copy = verify_copy(backup, expected_version)?;
    compare_content(source, &copy)
}

/// Contrôles propres à la copie : intégrité, clés étrangères, version du schéma.
fn verify_copy(backup: &Path, expected_version: u32) -> Result<Connection, String> {
    let copy = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("ouverture de la copie : {e}"))?;

    let integrity: String = copy
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|e| format!("integrity_check : {e}"))?;
    if integrity != "ok" {
        return Err(format!("integrity_check de la copie : {integrity}"));
    }
    let broken_links: u32 = copy
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| row.get(0))
        .map_err(|e| format!("foreign_key_check : {e}"))?;
    if broken_links > 0 {
        return Err(format!("{broken_links} référence(s) cassée(s) dans la copie"));
    }
    let version: u32 = copy
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|e| format!("user_version : {e}"))?;
    if version != expected_version {
        return Err(format!("version de la copie {version}, attendue {expected_version}"));
    }
    Ok(copy)
}

/// Mêmes tables, mêmes lignes (même ordre, mêmes valeurs, mêmes types) dans la copie.
fn compare_content(source: &Connection, copy: &Connection) -> Result<(), String> {
    let source_tables = table_names(source)?;
    if source_tables != table_names(copy)? {
        return Err("la liste des tables de la copie diffère de la source".into());
    }
    for table in &source_tables {
        compare_table(source, copy, table)?;
    }
    Ok(())
}

fn table_names(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(|e| e.to_string())?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(names)
}

/// Compare toutes les lignes de `table` (même ordre, mêmes valeurs, mêmes types).
fn compare_table(left: &Connection, right: &Connection, table: &str) -> Result<(), String> {
    // Nom issu de `sqlite_master` (jamais d'une saisie) ; guillemets doublés par prudence.
    let sql = format!("SELECT * FROM \"{}\" ORDER BY rowid", table.replace('"', "\"\""));
    let mut left_stmt = left.prepare(&sql).map_err(|e| format!("{table} : {e}"))?;
    let mut right_stmt = right.prepare(&sql).map_err(|e| format!("{table} : {e}"))?;
    let columns = left_stmt.column_count();
    if columns != right_stmt.column_count() {
        return Err(format!("{table} : nombre de colonnes différent"));
    }
    let mut left_rows = left_stmt.query([]).map_err(|e| e.to_string())?;
    let mut right_rows = right_stmt.query([]).map_err(|e| e.to_string())?;
    let mut count = 0u64;
    loop {
        let l = left_rows.next().map_err(|e| e.to_string())?;
        let r = right_rows.next().map_err(|e| e.to_string())?;
        match (l, r) {
            (None, None) => return Ok(()),
            (Some(l), Some(r)) => {
                count += 1;
                for column in 0..columns {
                    let a: Value = l.get(column).map_err(|e| e.to_string())?;
                    let b: Value = r.get(column).map_err(|e| e.to_string())?;
                    if a != b {
                        return Err(format!("{table} : la ligne {count} diffère dans la copie"));
                    }
                }
            }
            _ => return Err(format!("{table} : nombre de lignes différent dans la copie")),
        }
    }
}

/// Nom `morganiser-v<version>-<ms>.db`, suffixé `-1`, `-2`… si déjà pris.
fn unique_final_path(dir: &Path, version: u32, now_ms: i64) -> PathBuf {
    let base = format!("{PREFIX}{version}-{now_ms:013}");
    let mut candidate = dir.join(format!("{base}{EXTENSION}"));
    let mut suffix = 0u32;
    while candidate.exists() || partial_of(&candidate).exists() {
        suffix += 1;
        candidate = dir.join(format!("{base}-{suffix}{EXTENSION}"));
    }
    candidate
}

fn partial_of(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}{PARTIAL_SUFFIX}", path.display()))
}

/// Supprime les `*.db.partial` laissés par une interruption (seuls des FICHIERS de notre
/// motif ; tout le reste du dossier est laissé tel quel).
fn remove_interrupted_leftovers(dir: &Path) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(PREFIX)
            && name.ends_with(&format!("{EXTENSION}{PARTIAL_SUFFIX}"))
            && entry.path().is_file()
        {
            fs::remove_file(entry.path()).map_err(|e| format!("reste d'interruption : {e}"))?;
        }
    }
    Ok(())
}

/// `(version, horodatage, rang)` d'un nom de sauvegarde finalisée, `None` si le nom n'est pas
/// le nôtre (fichier laissé intact).
fn parse_name(name: &str) -> Option<(u32, i64, u32)> {
    let stem = name.strip_prefix(PREFIX)?.strip_suffix(EXTENSION)?;
    let mut parts = stem.split('-');
    let version = parts.next()?.parse().ok()?;
    let stamp = parts.next()?.parse().ok()?;
    let rank = match parts.next() {
        Some(rank) => rank.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((version, stamp, rank))
}

/// Lecture seule : la sauvegarde s'ouvre, est intègre et porte une version de schéma.
fn is_usable(path: &Path) -> bool {
    let Ok(conn) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return false;
    };
    let ok = conn
        .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .map(|result| result == "ok")
        .unwrap_or(false);
    let version = conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
        .unwrap_or(0);
    ok && version > 0
}

/// Garde les `keep` sauvegardes utilisables les plus récentes ; supprime les plus anciennes
/// utilisables. Un fichier illisible n'est ni compté ni supprimé.
fn prune(dir: &Path, keep: usize) -> Result<(), String> {
    let mut backups: Vec<((i64, u32), PathBuf)> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let (_, stamp, rank) = parse_name(&name)?;
            entry.path().is_file().then(|| ((stamp, rank), entry.path()))
        })
        .collect();
    backups.sort_by_key(|(key, _)| std::cmp::Reverse(*key)); // plus récentes d'abord

    let mut kept = 0;
    for (_, path) in backups {
        if !is_usable(&path) {
            eprintln!("[M'Organiser] sauvegarde illisible laissée intacte : {}", path.display());
            continue;
        }
        if kept < keep {
            kept += 1;
        } else {
            fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn vacuum_into_for_tests(source: &Connection, target: &Path) -> Result<(), String> {
    vacuum_into(source, target)
}

#[cfg(test)]
pub(super) fn verify_for_tests(source: &Connection, backup: &Path, expected_version: u32) -> Result<(), String> {
    verify(source, backup, expected_version)
}

#[cfg(test)]
pub(super) fn before_migration_with_for_tests(
    source: &Connection,
    db_path: &Path,
    from_version: u32,
    now_ms: i64,
    after_copy: &mut dyn FnMut(u32),
) -> Result<PathBuf, String> {
    before_migration_with(source, db_path, from_version, now_ms, after_copy)
}
