//! Répétition de la migration v1 → v2 sur une COPIE ISOLÉE d'une base.
//!
//! Deux usages :
//! - tests ordinaires : le dispositif est vérifié sur une base **fictive** créée à la volée ;
//! - test `#[ignore]` : le propriétaire fournit, par variable d'environnement, le chemin d'une
//!   copie qu'il a faite lui-même (application fermée, `.db` + `-wal` + `-shm` ensemble) :
//!
//!   ```text
//!   $env:MORGANISER_REHEARSAL_DB = "C:\\Essais\\copie-dev\\morganiser.db"
//!   cargo test --lib repetition_sur_copie -- --ignored --nocapture
//!   ```
//!
//! Garde-fous : le fichier fourni n'est JAMAIS ouvert par SQLite (on travaille sur une copie
//! temporaire de plus) ; les chemins situés dans `%LOCALAPPDATA%` ou dans un dossier
//! `com.morganiser*` (données Dev ou Stable réelles) sont refusés ; le rapport ne contient que
//! des comptes et des empreintes, jamais le texte des captures ; les fichiers source ne doivent
//! pas changer (taille et date vérifiées avant/après).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rusqlite::types::Value;
use rusqlite::Connection;

use super::*;
use crate::{inbox, tasks};

const ENV_VAR: &str = "MORGANISER_REHEARSAL_DB";
const V1_TABLES: [(&str, &str); 2] = [
    ("destinations", "SELECT id, label, kind, position, archived_at FROM destinations ORDER BY id"),
    (
        "inbox_items",
        "SELECT id, content, destination_id, created_at, updated_at, deleted_at FROM inbox_items ORDER BY id",
    ),
];

/// Refuse tout chemin pouvant désigner des données réelles de l'application.
fn check_path(path: &Path, forbidden_roots: &[PathBuf]) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("le chemin doit être absolu".into());
    }
    if path.file_name().and_then(|n| n.to_str()) != Some(DB_FILE_NAME) {
        return Err(format!("le fichier doit s'appeler {DB_FILE_NAME}"));
    }
    if !path.is_file() {
        return Err("fichier introuvable".into());
    }
    let lowered = path.to_string_lossy().to_lowercase();
    if lowered.split(['\\', '/']).any(|part| part.starts_with("com.morganiser")) {
        return Err("chemin dans un dossier de données de l'application (com.morganiser*)".into());
    }
    let canonical = fs::canonicalize(path).map_err(|e| e.to_string())?;
    for root in forbidden_roots {
        if let Ok(root) = fs::canonicalize(root) {
            if canonical.starts_with(&root) {
                return Err(format!("chemin sous {}", root.display()));
            }
        }
    }
    Ok(())
}

fn forbidden_roots() -> Vec<PathBuf> {
    ["LOCALAPPDATA", "APPDATA"]
        .iter()
        .filter_map(|name| std::env::var_os(name).map(PathBuf::from))
        .collect()
}

/// Empreinte FNV-1a de lignes (valeurs typées) : aucune donnée n'est conservée ni affichée.
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

struct SourceState(Vec<(PathBuf, Option<(u64, SystemTime)>)>);

impl SourceState {
    fn read(db: &Path) -> Self {
        let files = ["", "-wal", "-shm"]
            .iter()
            .map(|suffix| PathBuf::from(format!("{}{suffix}", db.display())))
            .map(|path| {
                let state = fs::metadata(&path).ok().map(|m| (m.len(), m.modified().unwrap()));
                (path, state)
            })
            .collect();
        SourceState(files)
    }
}

#[derive(Debug)]
struct Report {
    version_before: u32,
    version_after: u32,
    tables: Vec<(&'static str, usize, u64)>,
    live_captures: usize,
    backup: String,
}

/// Exécute toute la répétition sur une copie TEMPORAIRE de `source_db`.
fn rehearse(source_db: &Path) -> Result<Report, String> {
    let before_state = SourceState::read(source_db);

    // 1. Copie des fichiers (sous le MÊME nom de base pour que `-wal` reste apparié).
    let work = tempfile::tempdir().map_err(|e| e.to_string())?;
    let db = work.path().join(DB_FILE_NAME);
    for suffix in ["", "-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{suffix}", source_db.display()));
        if from.is_file() {
            let to = work.path().join(format!("{DB_FILE_NAME}{suffix}"));
            fs::copy(&from, &to).map_err(|e| format!("copie de {suffix}: {e}"))?;
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
    let mut conn = open(&db).map_err(|e| format!("migration : {e}"))?;
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

    // 5. Aller-retour métier : conversion, annulation, reconversion, annulation.
    let live = inbox::list_items(&conn, &inbox::InboxFilter::All, inbox::Scope::Active, None, 1)
        .map_err(|e| e.to_string())?;
    let live_captures = live.total as usize;
    if let Some(first) = live.items.first() {
        let mut stamp = 4_000_000_000_000i64;
        for _ in 0..2 {
            let current = inbox::get_item(&conn, &first.id).map_err(|e| e.to_string())?;
            stamp += 1;
            let done = tasks::convert_item(&conn, &first.id, current.updated_at, None, stamp)
                .map_err(|e| format!("conversion : {e}"))?;
            stamp += 1;
            tasks::cancel_conversion(&conn, &done.task.id, stamp).map_err(|e| format!("annulation : {e}"))?;
        }
        if v1_fingerprints(&conn) != reference {
            return Err("l'aller-retour conversion/annulation a modifié des captures".into());
        }
    }

    // 6. Restauration depuis la sauvegarde (procédure documentée), puis remigration.
    drop(conn.transaction()); // aucune transaction ouverte
    drop(conn);
    let aside = work.path().join("mis-de-cote");
    fs::create_dir_all(&aside).map_err(|e| e.to_string())?;
    for suffix in ["", "-wal", "-shm"] {
        let file = work.path().join(format!("{DB_FILE_NAME}{suffix}"));
        if file.is_file() {
            fs::rename(&file, aside.join(file.file_name().unwrap())).map_err(|e| e.to_string())?;
        }
    }
    fs::copy(&backups[0], &db).map_err(|e| e.to_string())?;
    let restored = open(&db).map_err(|e| format!("réouverture après restauration : {e}"))?;
    if v1_fingerprints(&restored) != reference {
        return Err("la base restaurée diffère de la référence".into());
    }
    let backup_name = backups[0].file_name().unwrap().to_string_lossy().into_owned();
    drop(restored);

    // 7. Les fichiers source n'ont pas bougé.
    if SourceState::read(source_db).0 != before_state.0 {
        return Err("les fichiers source ont été modifiés (taille ou date)".into());
    }
    Ok(Report { version_before, version_after, tables: reference, live_captures, backup: backup_name })
}

/// Crée une base v1 fictive complète (WAL non vidé) et renvoie le chemin de son dossier.
fn fictitious_v1() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    {
        let conn = Connection::open(&path).unwrap();
        configure(&conn).unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
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
    (dir, path)
}

// --- Garde-fous ---

#[test]
fn les_chemins_de_donnees_reelles_sont_refuses() {
    let (_dir, path) = fictitious_v1();
    // Un chemin ordinaire hors dossiers protégés passe.
    assert!(check_path(&path, &[]).is_ok());

    // Dossier nommé comme un environnement réel (Dev ou Stable) : refusé.
    for identifier in ["com.morganiser.desktop.dev", "com.morganiser.desktop", "COM.MORGANISER.DESKTOP"] {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join(identifier).join("data");
        fs::create_dir_all(&data).unwrap();
        let file = data.join(DB_FILE_NAME);
        fs::write(&file, b"x").unwrap();
        assert!(check_path(&file, &[]).unwrap_err().contains("com.morganiser"), "{identifier}");
    }

    // Racine interdite (ex. LOCALAPPDATA) : tout ce qui est dessous est refusé.
    let parent = path.parent().unwrap().to_path_buf();
    assert!(check_path(&path, &[parent]).unwrap_err().contains("chemin sous"));

    // Relatif, mauvais nom, inexistant.
    assert!(check_path(Path::new("morganiser.db"), &[]).is_err());
    assert!(check_path(&path.with_file_name("autre.db"), &[]).is_err());
    assert!(check_path(&path.with_file_name(DB_FILE_NAME).with_extension("x"), &[]).is_err());
}

// --- Dispositif vérifié sur une base fictive ---

#[test]
fn la_repetition_fonctionne_sur_une_base_fictive_sans_toucher_la_source() {
    let (_dir, path) = fictitious_v1();
    let before = SourceState::read(&path);

    let report = rehearse(&path).unwrap();

    assert_eq!((report.version_before, report.version_after), (1, 2));
    assert_eq!(report.tables.iter().find(|t| t.0 == "inbox_items").unwrap().1, 3);
    assert_eq!(report.tables.iter().find(|t| t.0 == "destinations").unwrap().1, 5);
    assert_eq!(report.live_captures, 2); // la 3e est à la corbeille
    assert!(report.backup.starts_with("morganiser-v1-"));
    assert_eq!(SourceState::read(&path).0, before.0, "la source n'a pas bougé");
}

#[test]
fn la_repetition_refuse_une_base_qui_n_est_pas_en_v1() {
    let (_dir, path) = fictitious_v1();
    drop(open(&path).unwrap()); // migrée en v2
    let error = rehearse(&path).unwrap_err();
    assert!(error.contains("version 2"), "{error}");
}

#[test]
fn les_empreintes_distinguent_deux_contenus_et_reconnaissent_deux_copies() {
    // Le rapport s'appuie sur ces empreintes : identiques pour deux copies fidèles, différentes
    // dès qu'une seule valeur change.
    let (_dir_a, a) = fictitious_v1();
    let (_dir_b, b) = fictitious_v1();
    let conn_a = Connection::open(&a).unwrap();
    let conn_b = Connection::open(&b).unwrap();
    assert_eq!(v1_fingerprints(&conn_a), v1_fingerprints(&conn_b));
    conn_b.execute("UPDATE inbox_items SET content = content || ' ' WHERE id = 'f0'", []).unwrap();
    assert_ne!(v1_fingerprints(&conn_a), v1_fingerprints(&conn_b));
}

// --- Répétition sur une copie fournie par le propriétaire (jamais lancée par la suite ordinaire) ---

#[test]
#[ignore = "répétition sur une copie isolée : définir MORGANISER_REHEARSAL_DB (voir l'en-tête du fichier)"]
fn repetition_sur_copie_isolee() {
    let Ok(raw) = std::env::var(ENV_VAR) else {
        println!("{ENV_VAR} non définie : rien à faire.");
        return;
    };
    let path = PathBuf::from(raw);
    if let Err(reason) = check_path(&path, &forbidden_roots()) {
        panic!("chemin refusé par les garde-fous : {reason}");
    }
    match rehearse(&path) {
        Ok(report) => {
            println!("Répétition réussie (aucun texte affiché).");
            println!("  version : v{} -> v{}", report.version_before, report.version_after);
            for (table, count, hash) in &report.tables {
                println!("  {table}: {count} ligne(s), empreinte {hash:016x}");
            }
            println!("  captures actives : {}", report.live_captures);
            println!("  sauvegarde vérifiée : {}", report.backup);
            println!("  conversion/annulation/reconversion : OK ; restauration : OK ; source inchangée : OK");
        }
        Err(error) => panic!("répétition en échec : {error}"),
    }
}
