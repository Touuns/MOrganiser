//! Tests de la migration v1 → v2 et de la sauvegarde préalable.
//! Uniquement des bases temporaires fictives : jamais les dossiers de données Dev ou Stable.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::types::Value;
use rusqlite::Connection;

use super::backup::{self, BACKUPS_KEPT, BACKUP_DIR};
use super::*;

/// Base au schéma v1 (VRAIE migration 0001), avec des données variées, laissée ouverte : les
/// écritures restent dans `-wal`, comme après une session normale ou un arrêt brutal.
fn create_v1(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    configure(&conn).unwrap();
    conn.execute_batch(MIGRATIONS[0]).unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    #[allow(clippy::type_complexity)]
    let rows: [(&str, &str, Option<&str>, i64, i64, Option<i64>); 5] = [
        ("c1", "Vendre ma PlayStation 5", None, 1_000, 1_000, None),
        ("c2", "Accents éàü, 日本語, 😀 et l'apostrophe", Some("finances"), 2_000, 2_500, None),
        ("c3", "'; DROP TABLE inbox_items; --", Some("moi"), 3_000, 3_000, None),
        ("c4", "Supprimée mais récupérable", None, 4_000, 4_000, Some(9_000)),
        ("c5", "Ligne 1\r\nLigne 2\n\nLigne 4", Some("externe"), 5_000, 5_000, None),
    ];
    for (id, content, destination, created, updated, deleted) in rows {
        conn.execute(
            "INSERT INTO inbox_items (id, content, destination_id, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, content, destination, created, updated, deleted],
        )
        .unwrap();
    }
    conn.execute("UPDATE destinations SET archived_at = 7 WHERE id = 'administratif'", []).unwrap();
    conn
}

fn version(conn: &Connection) -> u32 {
    conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap()
}

/// Contenu d'une requête, valeur par valeur.
fn dump(conn: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut stmt = conn.prepare(sql).unwrap();
    let columns = stmt.column_count();
    let rows = stmt
        .query_map([], |row| (0..columns).map(|i| row.get::<_, Value>(i)).collect::<Result<Vec<_>, _>>())
        .unwrap();
    rows.map(Result::unwrap).collect()
}

const V1_ITEMS: &str =
    "SELECT id, content, destination_id, created_at, updated_at, deleted_at FROM inbox_items ORDER BY id";
const V1_DESTINATIONS: &str = "SELECT id, label, kind, position, archived_at FROM destinations ORDER BY id";

fn backups_in(db_path: &Path) -> Vec<String> {
    let dir = backup::backup_dir(db_path);
    let mut names: Vec<String> = match fs::read_dir(&dir) {
        Ok(entries) => entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names
}

fn columns_of(conn: &Connection, table: &str) -> Vec<String> {
    dump(conn, &format!("SELECT name FROM pragma_table_info('{table}')"))
        .into_iter()
        .map(|row| match &row[0] {
            Value::Text(name) => name.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    dump(conn, &format!("SELECT name FROM sqlite_master WHERE name = '{name}'")).len() == 1
}

fn data_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(DB_FILE_NAME);
    (dir, path)
}

// --- Migration v1 -> v2 ---

#[test]
fn migration_v1_vers_v2_conserve_toutes_les_donnees() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path); // reste ouverte : les données sont dans -wal, sans checkpoint
    let items_before = dump(&v1, V1_ITEMS);
    let destinations_before = dump(&v1, V1_DESTINATIONS);
    assert_eq!(items_before.len(), 5);

    let conn = open(&path).unwrap();

    assert_eq!(version(&conn), 2);
    assert_eq!(dump(&conn, V1_ITEMS), items_before, "captures et métadonnées identiques");
    assert_eq!(dump(&conn, V1_DESTINATIONS), destinations_before, "destinations identiques");
    // Les nouvelles colonnes existent et sont vides : aucune capture n'est « convertie ».
    assert_eq!(
        dump(&conn, "SELECT count(*) FROM inbox_items WHERE converted_at IS NOT NULL OR converted_task_id IS NOT NULL"),
        vec![vec![Value::Integer(0)]]
    );
    assert_eq!(dump(&conn, "SELECT count(*) FROM tasks"), vec![vec![Value::Integer(0)]]);
    // Intégrité et clés étrangères.
    assert_eq!(dump(&conn, "PRAGMA integrity_check"), vec![vec![Value::Text("ok".into())]]);
    assert!(dump(&conn, "PRAGMA foreign_key_check").is_empty());
    // Index et déclencheurs attendus.
    let objects: Vec<String> = dump(
        &conn,
        "SELECT name FROM sqlite_master WHERE name LIKE 'tasks_%' OR name LIKE 'inbox_items_%' ORDER BY name",
    )
    .into_iter()
    .map(|r| match &r[0] {
        Value::Text(t) => t.clone(),
        _ => unreachable!(),
    })
    .collect();
    for expected in [
        "tasks_one_active_per_origin",
        "tasks_updated_at_must_grow",
        "tasks_origin_is_immutable",
        "inbox_items_converted_task",
        "inbox_items_converted",
    ] {
        assert!(objects.iter().any(|o| o == expected), "{expected} manquant : {objects:?}");
    }
}

#[test]
fn une_base_neuve_ou_deja_migree_ne_cree_pas_de_sauvegarde() {
    let (_dir, path) = data_path();
    drop(open(&path).unwrap()); // création : rien à sauvegarder
    assert!(backups_in(&path).is_empty());
    assert!(!backup::backup_dir(&path).exists(), "pas même le dossier");
    drop(open(&path).unwrap()); // déjà en v2
    assert!(!backup::backup_dir(&path).exists());
}

#[test]
fn la_migration_d_une_base_existante_cree_une_sauvegarde_verifiee_de_la_v1() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);
    let items_before = dump(&v1, V1_ITEMS);

    drop(open(&path).unwrap());

    let names = backups_in(&path);
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].starts_with("morganiser-v1-") && names[0].ends_with(".db"), "{names:?}");
    assert!(!names[0].ends_with(".partial"));

    // La sauvegarde est un fichier autonome (sans -wal) en v1, avec TOUTES les données,
    // y compris celles qui n'étaient que dans -wal au moment de la sauvegarde.
    let file = backup::backup_dir(&path).join(&names[0]);
    let copy = Connection::open(&file).unwrap();
    assert_eq!(version(&copy), 1);
    assert_eq!(dump(&copy, V1_ITEMS), items_before);
    assert_eq!(dump(&copy, "PRAGMA integrity_check"), vec![vec![Value::Text("ok".into())]]);
    assert!(!table_exists(&copy, "tasks"));
    drop(copy);
    assert!(!PathBuf::from(format!("{}-wal", file.display())).exists());
}

#[test]
fn copie_naive_du_fichier_db_est_incomplete_la_sauvegarde_non() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);
    // Les écritures sont dans -wal : copier `.db` seul ne les contient pas.
    let naive = path.with_file_name("copie-naive.db");
    fs::copy(&path, &naive).unwrap();
    let naive_conn = Connection::open(&naive).unwrap();
    let naive_rows: Result<i64, _> =
        naive_conn.query_row("SELECT count(*) FROM inbox_items", [], |r| r.get(0));
    assert!(!matches!(naive_rows, Ok(5)), "la copie naïve ne doit pas être complète : {naive_rows:?}");

    let saved = backup::before_migration(&v1, &path, 1, 1_000).unwrap();
    let copy = Connection::open(&saved).unwrap();
    assert_eq!(dump(&copy, V1_ITEMS), dump(&v1, V1_ITEMS), "la sauvegarde cohérente est complète");
}

// --- Échecs ---

#[test]
fn un_echec_partiel_de_migration_ne_laisse_aucune_modification() {
    let (_dir, path) = data_path();
    let mut conn = create_v1(&path);
    let before = dump(&conn, V1_ITEMS);

    // Vraie migration 0002 suivie d'une instruction qui échoue : tout est annulé.
    let broken = format!("{}\nSELECT * FROM table_inexistante;", MIGRATIONS[1]);
    let error = migrate_with(&mut conn, &[MIGRATIONS[0], broken.as_str()]).unwrap_err();
    assert!(matches!(error, StorageError::Sqlite(_)), "{error:?}");

    assert_eq!(version(&conn), 1, "user_version inchangée");
    assert!(!table_exists(&conn, "tasks"));
    assert!(!table_exists(&conn, "tasks_updated_at_must_grow"));
    assert!(!table_exists(&conn, "inbox_items_converted_task"));
    assert!(!columns_of(&conn, "inbox_items").contains(&"converted_at".to_string()));
    assert_eq!(dump(&conn, V1_ITEMS), before);
    assert_eq!(dump(&conn, "PRAGMA integrity_check"), vec![vec![Value::Text("ok".into())]]);

    // Et la vraie migration réussit ensuite, depuis cet état intact.
    assert_eq!(migrate(&mut conn).unwrap(), SCHEMA_VERSION);
    assert_eq!(dump(&conn, V1_ITEMS), before);
}

#[test]
fn un_echec_au_milieu_d_une_migration_a_plusieurs_etapes_garde_les_etapes_validees() {
    let (_dir, path) = data_path();
    let mut conn = create_v1(&path);
    let bad = "CREATE TABLE a (x INTEGER); INSERT INTO inexistante VALUES (1);";
    migrate_with(&mut conn, &[MIGRATIONS[0], MIGRATIONS[1], bad]).unwrap_err();
    // 0002 (validée dans sa propre transaction) reste ; la tentative suivante est annulée.
    assert_eq!(version(&conn), 2);
    assert!(table_exists(&conn, "tasks"));
    assert!(!table_exists(&conn, "a"));
}

#[test]
fn l_echec_de_la_sauvegarde_empeche_la_migration() {
    let (dir, path) = data_path();
    drop(create_v1(&path)); // fermée : plus de -wal, le fichier est complet
    // Un FICHIER occupe l'emplacement du dossier de sauvegarde : impossible de sauvegarder.
    fs::write(dir.path().join(BACKUP_DIR), b"je ne suis pas un dossier").unwrap();
    let bytes_before = fs::read(&path).unwrap();

    let error = open(&path).unwrap_err();
    assert!(matches!(error, StorageError::Backup(_)), "{error:?}");
    assert!(error.to_string().contains("la base n'a pas été modifiée"), "{error}");

    assert_eq!(fs::read(&path).unwrap(), bytes_before, "fichier de base strictement intact");
    let conn = Connection::open(&path).unwrap();
    assert_eq!(version(&conn), 1);
    assert!(!table_exists(&conn, "tasks"));
}

#[test]
fn une_base_d_une_version_future_est_refusee_sans_sauvegarde_ni_modification() {
    let (_dir, path) = data_path();
    {
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
        conn.execute_batch("CREATE TABLE futur (x TEXT); INSERT INTO futur VALUES ('a');").unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1).unwrap();
    }
    let bytes_before = fs::read(&path).unwrap();
    let error = open(&path).unwrap_err();
    assert!(matches!(error, StorageError::NewerSchema { found: 3, supported: 2 }), "{error:?}");
    assert_eq!(fs::read(&path).unwrap(), bytes_before);
    assert!(backups_in(&path).is_empty());
    assert!(!backup::backup_dir(&path).exists());
}

// --- Sauvegarde : fichiers temporaires, écrasement, conservation ---

#[test]
fn un_reste_d_interruption_est_nettoye_et_jamais_compte_comme_sauvegarde() {
    let (_dir, path) = data_path();
    drop(create_v1(&path));
    let dir = backup::backup_dir(&path);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("morganiser-v1-0000000000001.db.partial"), b"troncature").unwrap();
    fs::write(dir.join("notes.txt"), b"a moi").unwrap(); // étranger : ne doit pas être touché

    drop(open(&path).unwrap());

    let names = backups_in(&path);
    assert!(!names.iter().any(|n| n.ends_with(".partial")), "{names:?}");
    assert!(names.contains(&"notes.txt".to_string()));
    let valid: Vec<_> = names.iter().filter(|n| n.starts_with("morganiser-v1-")).collect();
    assert_eq!(valid.len(), 1, "{names:?}");
    let copy = Connection::open(dir.join(valid[0])).unwrap();
    assert_eq!(version(&copy), 1);
}

#[test]
fn une_sauvegarde_existante_n_est_jamais_ecrasee() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);

    let first = backup::before_migration(&v1, &path, 1, 42).unwrap();
    let first_bytes = fs::read(&first).unwrap();
    // Même instant : un autre nom, la première sauvegarde est strictement intacte.
    v1.execute("UPDATE inbox_items SET content = 'Modifiée entre-temps' WHERE id = 'c1'", []).unwrap();
    let second = backup::before_migration(&v1, &path, 1, 42).unwrap();

    assert_ne!(first, second);
    assert_eq!(fs::read(&first).unwrap(), first_bytes);
    let second_conn = Connection::open(&second).unwrap();
    assert_eq!(
        dump(&second_conn, "SELECT content FROM inbox_items WHERE id = 'c1'"),
        vec![vec![Value::Text("Modifiée entre-temps".into())]]
    );
}

#[test]
fn seules_les_trois_plus_recentes_sont_conservees_et_les_fichiers_etrangers_intacts() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);
    let dir = backup::backup_dir(&path);
    fs::create_dir_all(&dir).unwrap();
    // Un fichier ILLISIBLE au nom de sauvegarde (très ancien) : ni compté, ni supprimé.
    fs::write(dir.join("morganiser-v1-0000000000000.db"), b"pas une base SQLite").unwrap();
    fs::write(dir.join("autre.txt"), b"x").unwrap();

    for stamp in 1..=5 {
        backup::before_migration(&v1, &path, 1, stamp).unwrap();
    }

    let names = backups_in(&path);
    let kept: Vec<_> = names.iter().filter(|n| n.starts_with("morganiser-v1-00000000000")).collect();
    assert_eq!(BACKUPS_KEPT, 3);
    // 3 valides (3, 4, 5) + le fichier illisible laissé en place.
    assert_eq!(
        kept.iter().map(|n| n.as_str()).collect::<Vec<_>>(),
        [
            "morganiser-v1-0000000000000.db",
            "morganiser-v1-0000000000003.db",
            "morganiser-v1-0000000000004.db",
            "morganiser-v1-0000000000005.db",
        ]
    );
    assert!(names.contains(&"autre.txt".to_string()));
}

#[test]
fn l_echec_d_une_nouvelle_sauvegarde_ne_supprime_aucune_ancienne() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);
    for stamp in 1..=3 {
        backup::before_migration(&v1, &path, 1, stamp).unwrap();
    }
    let before = backups_in(&path);
    assert_eq!(before.len(), 3);

    // Vérification qui échoue : on annonce une version 7 alors que la base est en v1.
    let error = backup::before_migration(&v1, &path, 7, 6).unwrap_err();
    assert!(error.contains("version"), "{error}");

    assert_eq!(backups_in(&path), before, "ni suppression, ni reste .partial");
}

#[test]
fn la_verification_refuse_une_copie_qui_differe_de_la_source() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);
    let target = path.with_file_name("copie.db");
    backup::vacuum_into_for_tests(&v1, &target).unwrap();
    assert!(backup::verify_for_tests(&v1, &target, 1).is_ok(), "copie fidèle");

    // Mauvaise version annoncée.
    assert!(backup::verify_for_tests(&v1, &target, 2).unwrap_err().contains("version"));
    // Contenu qui diverge : une ligne modifiée, puis une ligne en plus dans la source.
    v1.execute("UPDATE inbox_items SET content = 'Autre' WHERE id = 'c2'", []).unwrap();
    let error = backup::verify_for_tests(&v1, &target, 1).unwrap_err();
    assert!(error.contains("inbox_items") && error.contains("diffère"), "{error}");
    v1.execute("UPDATE inbox_items SET content = 'Accents éàü, 日本語, 😀 et l''apostrophe' WHERE id = 'c2'", [])
        .unwrap();
    assert!(backup::verify_for_tests(&v1, &target, 1).is_ok(), "retour à l'identique");
    v1.execute(
        "INSERT INTO inbox_items (id, content, created_at, updated_at) VALUES ('c6', 'En plus', 1, 1)",
        [],
    )
    .unwrap();
    let error = backup::verify_for_tests(&v1, &target, 1).unwrap_err();
    assert!(error.contains("nombre de lignes"), "{error}");
}

#[test]
fn sauvegarde_coherente_pendant_des_ecritures_concurrentes() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let (_dir, path) = data_path();
    let conn = open(&path).unwrap();
    for i in 0..200 {
        conn.execute(
            "INSERT INTO inbox_items (id, content, created_at, updated_at) VALUES (?1, 'base', 1, 1)",
            [format!("b{i:04}")],
        )
        .unwrap();
    }

    let stop = Arc::new(AtomicBool::new(false));
    let writer = {
        let (path, stop) = (path.clone(), stop.clone());
        std::thread::spawn(move || {
            let conn = Connection::open(&path).unwrap();
            conn.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
            let mut written = 0u32;
            while !stop.load(Ordering::Relaxed) {
                conn.execute(
                    "INSERT INTO inbox_items (id, content, created_at, updated_at) VALUES (?1, 'concurrent', 1, 1)",
                    [format!("w{written:06}")],
                )
                .unwrap();
                written += 1;
            }
            written
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(50));
    let target = path.with_file_name("sous-charge.db");
    backup::vacuum_into_for_tests(&conn, &target).unwrap();
    stop.store(true, Ordering::Relaxed);
    let written = writer.join().unwrap();

    let copy = Connection::open(&target).unwrap();
    assert_eq!(dump(&copy, "PRAGMA integrity_check"), vec![vec![Value::Text("ok".into())]]);
    let total: i64 = copy.query_row("SELECT count(*) FROM inbox_items", [], |r| r.get(0)).unwrap();
    let base: i64 = copy
        .query_row("SELECT count(*) FROM inbox_items WHERE id LIKE 'b%'", [], |r| r.get(0))
        .unwrap();
    let concurrent = total - base;
    assert_eq!(base, 200, "toutes les données validées avant la copie y sont");
    assert!(concurrent >= 0 && concurrent <= written as i64);
    // Instantané cohérent : les écritures concurrentes présentes forment un préfixe sans trou.
    let ids: Vec<String> = dump(&copy, "SELECT id FROM inbox_items WHERE id LIKE 'w%' ORDER BY id")
        .into_iter()
        .map(|r| match &r[0] {
            Value::Text(t) => t.clone(),
            _ => unreachable!(),
        })
        .collect();
    let expected: Vec<String> = (0..concurrent).map(|i| format!("w{i:06}")).collect();
    assert_eq!(ids, expected, "aucun trou : copie prise en un seul instant");
}

// --- Source modifiée par une autre connexion pendant la sauvegarde ---

fn write_elsewhere(path: &Path, id: &str) {
    let other = Connection::open(path).unwrap();
    other.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
    other
        .execute(
            "INSERT INTO inbox_items (id, content, created_at, updated_at) VALUES (?1, 'écrite pendant la sauvegarde', 1, 1)",
            [id],
        )
        .unwrap();
}

#[test]
fn une_ecriture_concurrente_ne_fait_pas_rejeter_une_sauvegarde_valide() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);
    let mut attempts = Vec::new();

    // Une écriture concurrente survient APRÈS la copie du premier essai : la source a évolué.
    let saved = backup::before_migration_with_for_tests(&v1, &path, 1, 100, &mut |attempt| {
        attempts.push(attempt);
        if attempt == 1 {
            write_elsewhere(&path, "concurrente-1");
        }
    })
    .unwrap();

    assert_eq!(attempts, vec![1, 2], "le premier essai est repris, pas rejeté");
    let copy = Connection::open(&saved).unwrap();
    // La sauvegarde finale est un instantané complet de l'état courant, comparé ligne à ligne.
    assert_eq!(dump(&copy, V1_ITEMS), dump(&v1, V1_ITEMS));
    assert_eq!(
        dump(&copy, "SELECT count(*) FROM inbox_items WHERE id = 'concurrente-1'"),
        vec![vec![Value::Integer(1)]]
    );
    assert_eq!(dump(&copy, "PRAGMA integrity_check"), vec![vec![Value::Text("ok".into())]]);
    let names = backups_in(&path);
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(!names[0].ends_with(".partial"));
}

#[test]
fn une_source_qui_change_a_chaque_essai_donne_quand_meme_une_sauvegarde_controlee() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);
    let mut attempts = Vec::new();

    let saved = backup::before_migration_with_for_tests(&v1, &path, 1, 200, &mut |attempt| {
        attempts.push(attempt);
        write_elsewhere(&path, &format!("concurrente-{attempt}"));
    })
    .unwrap();

    assert_eq!(attempts, vec![1, 2, 3], "trois essais, puis acceptation après contrôles internes");
    let copy = Connection::open(&saved).unwrap();
    assert_eq!(version(&copy), 1);
    assert_eq!(dump(&copy, "PRAGMA integrity_check"), vec![vec![Value::Text("ok".into())]]);
    assert!(dump(&copy, "PRAGMA foreign_key_check").is_empty());
    // Les cinq captures d'origine y sont, intactes (la copie est un instantané cohérent).
    assert_eq!(
        dump(&copy, "SELECT count(*) FROM inbox_items WHERE id LIKE 'c_'"),
        vec![vec![Value::Integer(5)]]
    );
    assert!(backups_in(&path).iter().all(|n| !n.ends_with(".partial")));
}

#[test]
fn une_vraie_anomalie_est_toujours_rejetee_quand_la_source_est_stable() {
    let (_dir, path) = data_path();
    let v1 = create_v1(&path);
    // Mauvaise version annoncée, source stable : refus (aucune indulgence sans écriture concurrente).
    let error = backup::before_migration_with_for_tests(&v1, &path, 5, 300, &mut |_| {}).unwrap_err();
    assert!(error.contains("version"), "{error}");
    assert!(backups_in(&path).is_empty());
}

// --- Restauration ---

#[test]
fn restauration_depuis_une_sauvegarde() {
    let (dir, path) = data_path();
    let v1 = create_v1(&path);
    let original_items = dump(&v1, V1_ITEMS);
    let original_destinations = dump(&v1, V1_DESTINATIONS);
    drop(v1);

    // 1. L'application migre (sauvegarde automatique), puis l'utilisateur crée une tâche.
    let migrated = open(&path).unwrap();
    assert_eq!(version(&migrated), 2);
    crate::tasks::convert_item(&migrated, "c1", 1_000, None, 20_000).unwrap();
    assert_eq!(dump(&migrated, "SELECT count(*) FROM tasks"), vec![vec![Value::Integer(1)]]);
    drop(migrated); // application fermée

    // 2. PROCÉDURE DE RESTAURATION (documentée dans backup.rs), application fermée :
    //    a) mettre de côté `.db`, `-wal`, `-shm` (déplacer, ne pas supprimer) ;
    //    b) copier la sauvegarde sous le nom `morganiser.db`.
    let backups = backups_in(&path);
    assert_eq!(backups.len(), 1);
    let chosen = backup::backup_dir(&path).join(&backups[0]);
    let aside = dir.path().join("mis-de-cote");
    fs::create_dir_all(&aside).unwrap();
    for suffix in ["", "-wal", "-shm"] {
        let file = path.with_file_name(format!("{DB_FILE_NAME}{suffix}"));
        if file.exists() {
            fs::rename(&file, aside.join(file.file_name().unwrap())).unwrap();
        }
    }
    assert!(aside.join(DB_FILE_NAME).exists(), "l'ancienne base est conservée, pas détruite");
    fs::copy(&chosen, &path).unwrap();

    // 3. Relance : la base restaurée (v1) s'ouvre, est remigrée avec une NOUVELLE sauvegarde.
    let restored = open(&path).unwrap();
    assert_eq!(version(&restored), 2);
    assert_eq!(dump(&restored, V1_ITEMS), original_items, "captures d'origine intégralement restaurées");
    assert_eq!(dump(&restored, V1_DESTINATIONS), original_destinations);
    assert_eq!(dump(&restored, "SELECT count(*) FROM tasks"), vec![vec![Value::Integer(0)]]);
    assert_eq!(dump(&restored, "PRAGMA integrity_check"), vec![vec![Value::Text("ok".into())]]);
    assert_eq!(backups_in(&path).len(), 2, "la restauration a elle-même été sauvegardée avant migration");

    // L'ancienne base (avec sa tâche) reste récupérable dans le dossier mis de côté.
    let old = Connection::open(aside.join(DB_FILE_NAME)).unwrap();
    assert_eq!(dump(&old, "SELECT count(*) FROM tasks"), vec![vec![Value::Integer(1)]]);
}
