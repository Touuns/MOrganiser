//! Preuve et validation de la reconstruction de `tasks` (migration 0003, v2 → v3).
//!
//! Depuis 003-B1b, ces tests exécutent le VRAI fichier `migrations/0003_task_model.sql` : il est
//! découpé en étapes par ses marqueurs `-- @stage: nom` pour pouvoir injecter une panne avant
//! chacune d'elles. Il n'existe plus de copie « expérimentale » du SQL dans ce module.
//!
//! Isolation : uniquement des bases en mémoire ou des fichiers dans un dossier temporaire créé par
//! le test ; jamais le dossier de données de l'application, jamais une sauvegarde ni une copie.
//! Données entièrement fictives.
//!
//! Le SQL est exécuté par le VRAI exécuteur de migrations (`migrate_with` : transaction
//! `IMMEDIATE`, contrôle des clés étrangères, schéma et `user_version` ensemble), avec
//! `foreign_keys` actif comme en production.

use rusqlite::{types::Value, Connection};

use super::{check_schema_version_against, configure, migrate_with, StorageError, MIGRATIONS};

const T0: i64 = 1_760_000_000_000;

// ---------------------------------------------------------------------------------------------
// La migration de production, découpée en étapes par ses marqueurs.
// ---------------------------------------------------------------------------------------------

/// Le fichier SQL de production, tel qu'embarqué dans l'exécutable.
const PRODUCTION_SQL: &str = MIGRATIONS[2];

/// Étapes `(nom, SQL)` lues dans le fichier de production (lignes `-- @stage: nom`).
fn stages() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in PRODUCTION_SQL.lines() {
        if let Some(name) = line.trim().strip_prefix("-- @stage:") {
            out.push((name.trim().to_string(), String::new()));
        } else if let Some((_, sql)) = out.last_mut() {
            sql.push_str(line);
            sql.push('\n');
        }
    }
    out
}

fn stage(name: &str) -> String {
    stages().into_iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("étape {name}")).1
}

/// Migration complète (étapes recollées), ou interrompue par une panne volontaire AVANT l'étape
/// `fail_before` (`stages().len()` = après la dernière étape, avant la validation).
fn candidate(fail_before: Option<usize>) -> String {
    let all = stages();
    let mut sql = String::new();
    for (index, (_, body)) in all.iter().enumerate() {
        if fail_before == Some(index) {
            sql.push_str("SELECT * FROM panne_volontaire_inexistante;\n");
            return sql;
        }
        sql.push_str(body);
    }
    if fail_before == Some(all.len()) {
        sql.push_str("SELECT * FROM panne_volontaire_inexistante;\n");
    }
    sql
}

/// Passe par le vrai exécuteur : une transaction, schéma et `user_version` ensemble.
fn run_candidate(conn: &mut Connection, sql: &str) -> Result<u32, StorageError> {
    migrate_with(conn, &[MIGRATIONS[0], MIGRATIONS[1], sql])
}

// ---------------------------------------------------------------------------------------------
// Fixture v2 représentative, construite avec le VRAI schéma et les VRAIES fonctions métier.
// ---------------------------------------------------------------------------------------------

fn fresh_v2() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    configure(&conn).unwrap(); // foreign_keys = ON, comme en production
    migrate_with(&mut conn, &MIGRATIONS[..2]).unwrap();
    assert_eq!(user_version(&conn), 2);
    conn
}

fn v2_file(dir: &std::path::Path) -> (Connection, std::path::PathBuf) {
    let path = dir.join("synthetique.db");
    let mut conn = Connection::open(&path).unwrap();
    configure(&conn).unwrap(); // WAL + foreign_keys + synchronous FULL
    migrate_with(&mut conn, &MIGRATIONS[..2]).unwrap();
    (conn, path)
}

fn user_version(conn: &Connection) -> u32 {
    conn.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap()
}

// Depuis 003-B1b le code de production cible la v3 : la fixture v2 est donc construite en SQL
// v2 direct, qui reproduit exactement ce qu'écrivaient la brique 001 (capture) et la brique 002
// (conversion, annulation) sur le schéma v2.

pub(super) fn v2_capture(conn: &Connection, id: &str, content: &str, destination: Option<&str>, at: i64) {
    conn.execute(
        "INSERT INTO inbox_items (id, content, destination_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        rusqlite::params![id, content, destination, T0 + at],
    )
    .unwrap();
}

/// Conversion v2 : tâche minimale « à faire » + marquage de la capture (`updated_at` inchangé).
pub(super) fn v2_convert(conn: &Connection, task_id: &str, item_id: &str, title: &str, at: i64) {
    let (content, destination): (String, Option<String>) = conn
        .query_row(
            "SELECT content, destination_id FROM inbox_items WHERE id = ?1",
            [item_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, title, details, status, destination_id, origin_inbox_item_id,
                            created_at, updated_at)
         VALUES (?1, ?2, ?3, 'todo', ?4, ?5, ?6, ?6)",
        rusqlite::params![task_id, title, content, destination, item_id, T0 + at],
    )
    .unwrap();
    conn.execute(
        "UPDATE inbox_items SET converted_at = ?2, converted_task_id = ?3 WHERE id = ?1",
        rusqlite::params![item_id, T0 + at, task_id],
    )
    .unwrap();
}

/// Annulation v2 : tâche conservée (`deleted_at`), capture remise dans la boîte.
pub(super) fn v2_cancel(conn: &Connection, task_id: &str, at: i64) {
    conn.execute("UPDATE tasks SET deleted_at = ?2 WHERE id = ?1", rusqlite::params![task_id, T0 + at])
        .unwrap();
    conn.execute(
        "UPDATE inbox_items SET converted_at = NULL, converted_task_id = NULL WHERE converted_task_id = ?1",
        [task_id],
    )
    .unwrap();
}

/// Données entièrement fictives : aucune information personnelle.
pub(super) fn seed(conn: &Connection) {
    // t1 : rubrique, tâche intacte.
    v2_capture(conn, "c1", "Appeler la banque\nDemander le relevé de mars", Some("finances"), 1);
    v2_convert(conn, "t1", "c1", "Appeler la banque", 100);
    // t2 : univers, texte Unicode difficile, tâche ensuite avancée (modifiée).
    v2_capture(
        conn,
        "c2",
        "Prévenir ma sœur 🎂 — 東京 « test » \u{202e}fin\r\nLigne 2\tTab",
        Some("externe"),
        2,
    );
    v2_convert(conn, "t2", "c2", "Prévenir ma sœur 🎂 — 東京 « test » \u{202e}fin", 101);
    conn.execute("UPDATE tasks SET status = 'in_progress', updated_at = updated_at + 1 WHERE id = 't2'", [])
        .unwrap();
    // t3 : description de la taille maximale, retours à la ligne.
    let long = format!("{}Ligne éé", "Ligne é\n".repeat(1249)); // 10 000 caractères exactement
    v2_capture(conn, "c3", &long, Some("administratif"), 3);
    v2_convert(conn, "t3", "c3", "Ligne é", 102);
    // t4 : sans destination.
    v2_capture(conn, "c4", "Idée sans classement", None, 4);
    v2_convert(conn, "t4", "c4", "Idée sans classement", 103);
    // t5 (annulée) puis t6 (active) pour la même capture.
    v2_capture(conn, "c5", "Vendre la console", Some("inventaire"), 5);
    v2_convert(conn, "t5", "c5", "Vendre la console", 104);
    v2_cancel(conn, "t5", 105);
    v2_convert(conn, "t6", "c5", "Vendre la console", 106);
    // t7 : univers « moi », titre retouché avec croissance de updated_at.
    v2_capture(conn, "c6", "Renouveler l'abonnement", Some("moi"), 6);
    v2_convert(conn, "t7", "c6", "Renouveler l'abonnement", 107);
    conn.execute(
        "UPDATE tasks SET title = 'Abonnement (retouché)', updated_at = updated_at + 5 WHERE id = 't7'",
        [],
    )
    .unwrap();
    // Captures non converties : l'une en corbeille, l'autre à organiser.
    v2_capture(conn, "c7", "Capture à la corbeille", Some("finances"), 7);
    conn.execute("UPDATE inbox_items SET deleted_at = ?1 WHERE id = 'c7'", [T0 + 108]).unwrap();
    v2_capture(conn, "c8", "Capture à organiser", Some("moi"), 8);

    // Tâche « directe » (sans capture d'origine) : permise par le schéma v2.
    conn.execute(
        "INSERT INTO tasks (id, title, details, status, destination_id, created_at, updated_at)
         VALUES ('directe-1', 'Tâche directe', 'Description directe', 'waiting', 'moi', ?1, ?1)",
        [T0 + 200],
    )
    .unwrap();

    // Destinations archivées APRÈS usage : t2 (univers) et t6 (rubrique).
    conn.execute(
        "UPDATE destinations SET archived_at = ?1 WHERE id IN ('externe', 'inventaire')",
        [T0 + 300],
    )
    .unwrap();
}

// ---------------------------------------------------------------------------------------------
// Instantané complet : schéma (y compris temporaire), données, version, intégrité.
// ---------------------------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
struct State {
    schema: Vec<(String, String, String, Option<String>)>,
    temp_objects: Vec<String>,
    destinations: Vec<Vec<Value>>,
    inbox_items: Vec<Vec<Value>>,
    tasks: Vec<Vec<Value>>,
    user_version: u32,
    fk_violations: i64,
    integrity: String,
}

fn dump(conn: &Connection, table: &str) -> Vec<Vec<Value>> {
    let mut stmt = conn.prepare(&format!("SELECT * FROM {table} ORDER BY id")).unwrap();
    let columns = stmt.column_count();
    stmt.query_map([], |row| (0..columns).map(|i| row.get::<_, Value>(i)).collect())
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn state(conn: &Connection) -> State {
    let schema = {
        let mut stmt = conn
            .prepare("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY type, name")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let temp_objects = {
        let mut stmt = conn.prepare("SELECT name FROM sqlite_temp_master ORDER BY name").unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
    };
    State {
        schema,
        temp_objects,
        destinations: dump(conn, "destinations"),
        inbox_items: dump(conn, "inbox_items"),
        tasks: dump(conn, "tasks"),
        user_version: user_version(conn),
        fk_violations: conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
            .unwrap(),
        integrity: conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap(),
    }
}

/// Une tâche v2 lue colonne par colonne (comparaison indépendante du SQL du candidat).
#[derive(Debug, Clone, PartialEq)]
struct OldTask {
    id: String,
    title: String,
    details: String,
    status: String,
    destination_id: Option<String>,
    origin: Option<String>,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
}

fn old_tasks(conn: &Connection) -> Vec<OldTask> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, details, status, destination_id, origin_inbox_item_id,
                    created_at, updated_at, deleted_at FROM tasks ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |r| {
        Ok(OldTask {
            id: r.get(0)?,
            title: r.get(1)?,
            details: r.get(2)?,
            status: r.get(3)?,
            destination_id: r.get(4)?,
            origin: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
            deleted_at: r.get(8)?,
        })
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

#[derive(Debug, PartialEq)]
struct NewTask {
    id: String,
    title: String,
    details: String,
    status: String,
    responsibility: Option<String>,
    section: Option<String>,
    start_date: Option<String>,
    due_date: Option<String>,
    completed_on: Option<String>,
    priority: Option<i64>,
    origin: Option<String>,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
}

fn new_tasks(conn: &Connection) -> Vec<NewTask> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, details, status, responsibility_id, section_id, start_date, due_date,
                    completed_on, priority, origin_inbox_item_id, created_at, updated_at, deleted_at
             FROM tasks ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |r| {
        Ok(NewTask {
            id: r.get(0)?,
            title: r.get(1)?,
            details: r.get(2)?,
            status: r.get(3)?,
            responsibility: r.get(4)?,
            section: r.get(5)?,
            start_date: r.get(6)?,
            due_date: r.get(7)?,
            completed_on: r.get(8)?,
            priority: r.get(9)?,
            origin: r.get(10)?,
            created_at: r.get(11)?,
            updated_at: r.get(12)?,
            deleted_at: r.get(13)?,
        })
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn kinds(conn: &Connection) -> std::collections::HashMap<String, String> {
    let mut stmt = conn.prepare("SELECT id, kind FROM destinations").unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
}

/// Vérifie, tâche par tâche, que la v3 est la projection exacte de la v2.
fn assert_projection(before_tasks: &[OldTask], kinds: &std::collections::HashMap<String, String>, conn: &Connection) {
    let after = new_tasks(conn);
    assert_eq!(after.len(), before_tasks.len(), "nombre de tâches");
    for (old, new) in before_tasks.iter().zip(&after) {
        assert_eq!(new.id, old.id);
        assert_eq!(new.title, old.title);
        assert_eq!(new.details, old.details, "description de {}", old.id);
        assert_eq!(new.status, old.status);
        assert_eq!(new.origin, old.origin);
        assert_eq!((new.created_at, new.updated_at, new.deleted_at), (old.created_at, old.updated_at, old.deleted_at));
        // Répartition attendue, calculée ICI en Rust, indépendamment du SQL du candidat.
        let (want_resp, want_section) = match old.destination_id.as_deref() {
            None => (None, None),
            Some(id) => match kinds[id].as_str() {
                "responsibility" => (Some(id.to_string()), None),
                "section" => (None, Some(id.to_string())),
                other => panic!("nature inattendue {other}"),
            },
        };
        assert_eq!(new.responsibility, want_resp, "univers de {}", old.id);
        assert_eq!(new.section, want_section, "rubrique de {}", old.id);
        // Colonnes nouvelles : jamais inventées.
        assert_eq!((&new.start_date, &new.due_date, &new.completed_on, new.priority), (&None, &None, &None, None));
    }
}

fn exec_err(conn: &Connection, sql: &str) -> String {
    conn.execute_batch(sql).expect_err(&format!("devait échouer : {sql}")).to_string()
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[test]
fn version_de_sqlite_testee() {
    let conn = Connection::open_in_memory().unwrap();
    let version: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0)).unwrap();
    eprintln!("SQLITE_VERSION_TESTEE={version}");
    let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap());
    let (major, minor) = (parts.next().unwrap(), parts.next().unwrap());
    assert!((major, minor) >= (3, 35), "version trop ancienne : {version}");
}

#[test]
fn la_fixture_v2_est_representative() {
    let conn = fresh_v2();
    seed(&conn);
    let tasks = old_tasks(&conn);
    assert_eq!(tasks.len(), 8);
    assert!(tasks.iter().any(|t| t.deleted_at.is_some()), "une tâche annulée");
    assert!(tasks.iter().any(|t| t.destination_id.is_none()), "une tâche sans destination");
    assert!(tasks.iter().any(|t| t.origin.is_none()), "une tâche sans origine");
    assert!(tasks.iter().any(|t| t.updated_at > t.created_at), "une tâche modifiée");
    assert!(tasks.iter().any(|t| t.details.chars().count() == 10_000), "description maximale");
    assert!(tasks.iter().any(|t| t.details.contains('🎂') && t.details.contains("\r\n")), "Unicode");
    let archived: i64 = conn
        .query_row("SELECT count(*) FROM destinations WHERE archived_at IS NOT NULL", [], |r| r.get(0))
        .unwrap();
    assert_eq!(archived, 2);
    let converted: i64 = conn
        .query_row("SELECT count(*) FROM inbox_items WHERE converted_task_id IS NOT NULL", [], |r| r.get(0))
        .unwrap();
    assert_eq!(converted, 6);
    let trashed: i64 = conn
        .query_row("SELECT count(*) FROM inbox_items WHERE deleted_at IS NOT NULL", [], |r| r.get(0))
        .unwrap();
    assert_eq!(trashed, 1);
    // Le schéma v2 réel : clé étrangère de la capture vers la tâche, déclencheurs, index partiels.
    let s = state(&conn);
    assert_eq!(s.fk_violations, 0);
    assert_eq!(s.integrity, "ok");
    let names: Vec<&str> = s.schema.iter().map(|(_, n, _, _)| n.as_str()).collect();
    for expected in [
        "tasks_updated_at_must_grow",
        "tasks_origin_is_immutable",
        "tasks_one_active_per_origin",
        "inbox_items_converted_task",
    ] {
        assert!(names.contains(&expected), "{expected} absent de la fixture v2");
    }
}

#[test]
fn la_reconstruction_conserve_donnees_liens_et_captures() {
    let mut conn = fresh_v2();
    seed(&conn);
    let before = state(&conn);
    let before_tasks = old_tasks(&conn);
    let destination_kinds = kinds(&conn);

    let version = run_candidate(&mut conn, &candidate(None)).unwrap();
    assert_eq!(version, 3);

    let after = state(&conn);
    assert_eq!(after.user_version, 3);
    assert_eq!(after.integrity, "ok");
    assert_eq!(after.fk_violations, 0, "clés étrangères");
    assert!(after.temp_objects.is_empty(), "aucune table temporaire résiduelle : {:?}", after.temp_objects);
    // Captures (y compris liens de conversion) et destinations : strictement identiques.
    assert_eq!(after.inbox_items, before.inbox_items, "captures et liens de provenance");
    assert_eq!(after.destinations, before.destinations, "destinations, archivées comprises");
    // Tâches : projection exacte, champ par champ.
    assert_projection(&before_tasks, &destination_kinds, &conn);
}

#[test]
fn la_repartition_des_destinations_respecte_la_nature_et_les_archives() {
    let mut conn = fresh_v2();
    seed(&conn);
    run_candidate(&mut conn, &candidate(None)).unwrap();

    let row = |label: &str| -> (Option<String>, Option<String>) {
        conn.query_row(
            "SELECT responsibility_id, section_id FROM tasks WHERE title = ?1",
            [label],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    };
    assert_eq!(row("Appeler la banque"), (None, Some("finances".into())));
    assert_eq!(row("Prévenir ma sœur 🎂 — 東京 « test » \u{202e}fin"), (Some("externe".into()), None)); // univers archivé
    assert_eq!(row("Idée sans classement"), (None, None)); // jamais « Moi » par défaut
    assert_eq!(row("Tâche directe"), (Some("moi".into()), None));
    // Rubrique archivée : conservée sur les deux tâches (annulée et active) de la capture 5.
    let inventory: i64 = conn
        .query_row("SELECT count(*) FROM tasks WHERE section_id = 'inventaire'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(inventory, 2);
    // Aucune tâche n'a les deux dimensions en même temps à cause de la migration.
    let both: i64 = conn
        .query_row(
            "SELECT count(*) FROM tasks WHERE responsibility_id IS NOT NULL AND section_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(both, 0);
}

#[test]
fn la_reconstruction_fonctionne_sur_un_fichier_wal_et_survit_a_la_reouverture() {
    let dir = tempfile::tempdir().unwrap();
    let (mut conn, path) = v2_file(dir.path());
    seed(&conn);
    let before = state(&conn);
    let before_tasks = old_tasks(&conn);
    let destination_kinds = kinds(&conn);

    run_candidate(&mut conn, &candidate(None)).unwrap();
    drop(conn);

    let reopened = Connection::open(&path).unwrap();
    configure(&reopened).unwrap();
    let after = state(&reopened);
    assert_eq!(after.user_version, 3);
    assert_eq!(after.integrity, "ok");
    assert_eq!(after.fk_violations, 0);
    assert_eq!(after.inbox_items, before.inbox_items);
    assert_projection(&before_tasks, &destination_kinds, &reopened);
}

#[test]
fn une_base_vide_ou_sans_lien_se_reconstruit_aussi() {
    // Aucune ligne du tout.
    let mut empty = fresh_v2();
    run_candidate(&mut empty, &candidate(None)).unwrap();
    assert_eq!(state(&empty).integrity, "ok");
    assert_eq!(new_tasks(&empty).len(), 0);

    // Captures seules, aucune tâche.
    let mut only_items = fresh_v2();
    v2_capture(&only_items, "solo", "Seule capture", Some("moi"), 0);
    let before = state(&only_items);
    run_candidate(&mut only_items, &candidate(None)).unwrap();
    assert_eq!(state(&only_items).inbox_items, before.inbox_items);
}

#[test]
fn les_objets_du_schema_v3_sont_ceux_attendus() {
    let mut conn = fresh_v2();
    seed(&conn);
    run_candidate(&mut conn, &candidate(None)).unwrap();
    let s = state(&conn);
    let names: Vec<&str> = s.schema.iter().map(|(_, n, _, _)| n.as_str()).collect();
    for expected in [
        "tasks",
        "tasks_one_active_per_origin",
        "tasks_recent",
        "tasks_updated_at_must_grow",
        "tasks_origin_is_immutable",
        "tasks_responsibility_kind_insert",
        "tasks_responsibility_kind_update",
        "tasks_section_kind_insert",
        "tasks_section_kind_update",
        // Index d'`inbox_items` : jamais touchés.
        "inbox_items_converted_task",
        "inbox_items_converted",
        "inbox_items_recent",
    ] {
        assert!(names.contains(&expected), "{expected} absent");
    }
    assert!(!names.contains(&"tasks_new"), "la table de travail doit avoir disparu");
    // L'ancienne colonne n'existe plus ; la table reste STRICT.
    let sql: String =
        conn.query_row("SELECT sql FROM sqlite_master WHERE name = 'tasks'", [], |r| r.get(0)).unwrap();
    assert!(!sql.contains("destination_id"));
    assert!(sql.contains("STRICT"));
    // La clé étrangère de la capture désigne bien `tasks` (et non une table de travail).
    let items_sql: String =
        conn.query_row("SELECT sql FROM sqlite_master WHERE name = 'inbox_items'", [], |r| r.get(0)).unwrap();
    assert!(items_sql.contains("REFERENCES tasks(id)"), "{items_sql}");
}

#[test]
fn les_contraintes_et_declencheurs_fonctionnent_apres_reconstruction() {
    let mut conn = fresh_v2();
    seed(&conn);
    run_candidate(&mut conn, &candidate(None)).unwrap();
    let id: String = conn
        .query_row("SELECT id FROM tasks WHERE title = 'Appeler la banque'", [], |r| r.get(0))
        .unwrap();
    let insert = |extra_cols: &str, extra_vals: &str| {
        conn.execute_batch(&format!(
            "INSERT INTO tasks (id, title, created_at, updated_at{extra_cols})
             VALUES ('x-{}', 'T', 1, 1{extra_vals})",
            uuid::Uuid::now_v7()
        ))
    };

    // Description vide acceptée ; 10 001 caractères refusés ; 10 000 acceptés.
    assert!(insert(", details", ", ''").is_ok());
    assert!(insert("", "").is_ok()); // défaut : description vide
    assert!(insert(", details", &format!(", '{}'", "a".repeat(10_000))).is_ok());
    assert!(insert(", details", &format!(", '{}'", "a".repeat(10_001))).is_err());
    // Titre vide refusé.
    assert!(conn
        .execute_batch("INSERT INTO tasks (id, title, created_at, updated_at) VALUES ('t', '   ', 1, 1)")
        .is_err());

    // Nature des destinations.
    assert!(insert(", section_id", ", 'moi'").is_err(), "un univers n'est pas une rubrique");
    assert!(insert(", responsibility_id", ", 'finances'").is_err(), "une rubrique n'est pas un univers");
    assert!(insert(", responsibility_id", ", 'inconnu'").is_err(), "destination inconnue");
    assert!(insert(", responsibility_id, section_id", ", 'externe', 'administratif'").is_ok());
    assert!(exec_err(&conn, &format!("UPDATE tasks SET section_id = 'moi', updated_at = updated_at + 1 WHERE id = '{id}'"))
        .contains("section_id"));
    assert!(exec_err(&conn, &format!("UPDATE tasks SET responsibility_id = 'finances', updated_at = updated_at + 1 WHERE id = '{id}'"))
        .contains("responsibility_id"));

    // Dates calendaires : format et existence.
    assert!(insert(", due_date", ", '2026-10-14'").is_ok());
    for bad in ["2026-02-30", "2026-2-3", "14/10/2026", "2026-10-14T10:00", ""] {
        assert!(insert(", due_date", &format!(", '{bad}'")).is_err(), "date {bad:?} aurait dû être refusée");
    }
    assert!(insert(", start_date, due_date", ", '2026-10-01', '2026-10-14'").is_ok());
    assert!(insert(", start_date, due_date", ", '2026-10-14', '2026-10-14'").is_ok());
    assert!(insert(", start_date, due_date", ", '2026-10-15', '2026-10-14'").is_err(), "début après échéance");
    assert!(insert(", completed_on", ", '2026-10-14'").is_ok());

    // Priorité : absente par défaut, 1 à 5 admis (trois niveaux utilisés), hors bornes refusé.
    let default_priority: Option<i64> = conn
        .query_row("SELECT priority FROM tasks WHERE id = ?1", [&id], |r| r.get(0))
        .unwrap();
    assert_eq!(default_priority, None);
    for ok in [1, 2, 3, 5] {
        assert!(insert(", priority", &format!(", {ok}")).is_ok());
    }
    for bad in [0, 6, -1] {
        assert!(insert(", priority", &format!(", {bad}")).is_err());
    }

    // `updated_at` doit croître pour toute modification métier, y compris des champs NOUVEAUX.
    for column_change in [
        "title = 'Autre titre'",
        "details = 'Autre'",
        "status = 'done'",
        "responsibility_id = 'moi'",
        "section_id = 'inventaire'",
        "start_date = '2026-01-01'",
        "due_date = '2026-12-31'",
        "completed_on = '2026-10-14'",
        "priority = 3",
    ] {
        let err = exec_err(&conn, &format!("UPDATE tasks SET {column_change} WHERE id = '{id}'"));
        assert!(err.contains("updated_at"), "{column_change} : {err}");
    }
    conn.execute_batch(&format!(
        "UPDATE tasks SET due_date = '2026-12-31', priority = 3, updated_at = updated_at + 1 WHERE id = '{id}'"
    ))
    .unwrap();
    // `deleted_at` n'est pas un champ métier ; une mise à jour sans changement de valeur passe.
    conn.execute_batch(&format!("UPDATE tasks SET deleted_at = 9 WHERE id = '{id}'")).unwrap();
    conn.execute_batch(&format!("UPDATE tasks SET title = title WHERE id = '{id}'")).unwrap();

    // Provenance immuable.
    assert!(exec_err(&conn, &format!("UPDATE tasks SET origin_inbox_item_id = NULL WHERE id = '{id}'"))
        .contains("immuables"));
    assert!(exec_err(&conn, &format!("UPDATE tasks SET created_at = 5 WHERE id = '{id}'")).contains("immuables"));
    assert!(exec_err(&conn, &format!("UPDATE tasks SET id = 'autre' WHERE id = '{id}'")).contains("immuables"));

    // Une tâche active par capture ; une annulée libère la place.
    let origin: String = conn
        .query_row("SELECT origin_inbox_item_id FROM tasks WHERE title = 'Idée sans classement'", [], |r| r.get(0))
        .unwrap();
    let dup = format!(
        "INSERT INTO tasks (id, title, origin_inbox_item_id, created_at, updated_at)
         VALUES ('dup', 'Doublon', '{origin}', 1, 1)"
    );
    assert!(exec_err(&conn, &dup).contains("UNIQUE"));
    conn.execute_batch(&format!(
        "UPDATE tasks SET deleted_at = 9 WHERE origin_inbox_item_id = '{origin}' AND deleted_at IS NULL"
    ))
    .unwrap();
    conn.execute_batch(&dup).unwrap();

    // Clés étrangères actives dans les deux sens.
    assert!(exec_err(
        &conn,
        "INSERT INTO tasks (id, title, origin_inbox_item_id, created_at, updated_at) VALUES ('o', 'T', 'inconnue', 1, 1)"
    )
    .contains("FOREIGN KEY"));
    assert!(exec_err(&conn, "UPDATE inbox_items SET converted_task_id = 'inconnue' WHERE id = (SELECT id FROM inbox_items LIMIT 1)")
        .contains("FOREIGN KEY"));
    assert_eq!(state(&conn).fk_violations, 0);
}

#[test]
fn la_garde_d_annulation_de_la_brique_002_reste_applicable_apres_reconstruction() {
    let mut conn = fresh_v2();
    seed(&conn);
    run_candidate(&mut conn, &candidate(None)).unwrap();

    // Même SQL que `tasks::cancel_conversion` (garde « non modifiée » incluse), sur une tâche
    // migrée intacte puis sur une tâche migrée déjà avancée.
    let cancel = |title: &str| -> usize {
        let (task_id, origin): (String, String) = conn
            .query_row(
                "SELECT id, origin_inbox_item_id FROM tasks WHERE title = ?1 AND deleted_at IS NULL",
                [title],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let cancelled = conn
            .execute(
                "UPDATE tasks SET deleted_at = ?2
                 WHERE id = ?1 AND deleted_at IS NULL AND updated_at = created_at AND status = 'todo'",
                rusqlite::params![task_id, T0 + 999],
            )
            .unwrap();
        if cancelled == 1 {
            let restored = conn
                .execute(
                    "UPDATE inbox_items SET converted_at = NULL, converted_task_id = NULL
                     WHERE id = ?1 AND converted_task_id = ?2",
                    rusqlite::params![origin, task_id],
                )
                .unwrap();
            assert_eq!(restored, 1, "la capture est remise dans la boîte");
        }
        cancelled
    };
    assert_eq!(cancel("Appeler la banque"), 1, "tâche migrée intacte : annulable");
    assert_eq!(cancel("Abonnement (retouché)"), 0, "tâche modifiée : refusée");
    assert_eq!(state(&conn).fk_violations, 0);
}

// --- Échecs : retour exact à l'état v2 -------------------------------------------------------

#[test]
fn une_panne_avant_chaque_etape_ne_laisse_aucune_trace() {
    // `fail_before` va de 0 (avant tout) à STAGES.len() (après la dernière étape, avant la validation).
    for fail_before in 0..=stages().len() {
        let mut conn = fresh_v2();
        seed(&conn);
        let before = state(&conn);

        let error = run_candidate(&mut conn, &candidate(Some(fail_before))).unwrap_err();
        assert!(error.to_string().contains("panne_volontaire_inexistante"), "{error}");

        let after = state(&conn);
        let all = stages();
        let stage = all.get(fail_before).map_or("validation", |(name, _)| name.as_str());
        assert_eq!(after, before, "état altéré par une panne avant l'étape « {stage} »");
        assert_eq!(after.user_version, 2);
        assert!(after.temp_objects.is_empty(), "table temporaire résiduelle avant « {stage} »");
    }
}

#[test]
fn une_incoherence_detectee_au_depart_est_un_refus_sans_mutation() {
    let mut conn = fresh_v2();
    seed(&conn);
    // Base « abîmée » : une tâche référence une destination qui n'existe pas (clés étrangères
    // désactivées le temps de la corrompre, comme une base ancienne ou réparée à la main).
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute(
        "INSERT INTO tasks (id, title, details, destination_id, created_at, updated_at)
         VALUES ('orpheline', 'Destination perdue', 'x', 'disparue', 1, 1)",
        [],
    )
    .unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    let before = state(&conn);
    assert!(before.fk_violations > 0);

    let error = run_candidate(&mut conn, &candidate(None)).unwrap_err().to_string();
    assert!(error.contains("CHECK constraint failed"), "{error}");
    assert_eq!(state(&conn), before, "aucune mutation durable");
    assert_eq!(user_version(&conn), 2);
}

#[test]
fn une_violation_de_cle_etrangere_preexistante_est_aussi_refusee() {
    let mut conn = fresh_v2();
    seed(&conn);
    // Violation « de capture » : une capture pointe vers une tâche inexistante.
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute(
        "UPDATE inbox_items SET converted_task_id = 'fantome'
         WHERE id = (SELECT id FROM inbox_items WHERE converted_task_id IS NULL AND deleted_at IS NULL LIMIT 1)",
        [],
    )
    .unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    let before = state(&conn);

    let error = run_candidate(&mut conn, &candidate(None)).unwrap_err().to_string();
    assert!(error.contains("fk_violations_before"), "{error}");
    assert_eq!(state(&conn), before);
}

#[test]
fn un_echec_reel_lors_de_la_restauration_des_liens_annule_toute_la_reconstruction() {
    let mut conn = fresh_v2();
    seed(&conn);
    // Panne RÉALISTE (et non injectée dans le script) : un déclencheur de la base refuse
    // précisément le moment où un lien est réécrit.
    conn.execute_batch(
        "CREATE TRIGGER test_refuse_restauration BEFORE UPDATE OF converted_task_id ON inbox_items
         WHEN NEW.converted_task_id IS NOT NULL
         BEGIN SELECT RAISE(ABORT, 'restauration refusée par le test'); END;",
    )
    .unwrap();
    let before = state(&conn);

    let error = run_candidate(&mut conn, &candidate(None)).unwrap_err().to_string();
    assert!(error.contains("restauration refusée par le test"), "{error}");
    let after = state(&conn);
    assert_eq!(after, before, "ni table partiellement reconstruite, ni lien détaché");
    assert_eq!(after.user_version, 2);
    // Les liens d'origine sont toujours là.
    let links: i64 = conn
        .query_row("SELECT count(*) FROM inbox_items WHERE converted_task_id IS NOT NULL", [], |r| r.get(0))
        .unwrap();
    assert_eq!(links, 6);
}

#[test]
fn apres_un_echec_la_base_reste_une_v2_utilisable_puis_la_reprise_reussit() {
    let mut conn = fresh_v2();
    seed(&conn);
    let before = state(&conn);
    run_candidate(&mut conn, &candidate(Some(7))).unwrap_err(); // panne juste avant les déclencheurs
    assert_eq!(state(&conn), before);

    // La base fonctionne toujours en v2 (SQL v2 : le code de production cible désormais la v3).
    v2_capture(&conn, "apres-echec", "Créée après l'échec", Some("finances"), 500);
    v2_convert(&conn, "t-apres-1", "apres-echec", "Première conversion", 501);
    v2_cancel(&conn, "t-apres-1", 502);
    v2_convert(&conn, "t-apres-2", "apres-echec", "Titre repris", 503);
    assert_eq!(state(&conn).fk_violations, 0);
    assert_eq!(user_version(&conn), 2);

    // Reprise : la migration réussit et conserve aussi ce qui a été écrit entre-temps.
    let kinds_now = kinds(&conn);
    let tasks_now = old_tasks(&conn);
    let items_now = state(&conn).inbox_items;
    assert_eq!(run_candidate(&mut conn, &candidate(None)).unwrap(), 3);
    assert_projection(&tasks_now, &kinds_now, &conn);
    assert_eq!(state(&conn).inbox_items, items_now);
    assert_eq!(state(&conn).integrity, "ok");
}

#[test]
fn rejouer_le_candidat_sur_une_base_v3_ne_fait_rien() {
    let mut conn = fresh_v2();
    seed(&conn);
    run_candidate(&mut conn, &candidate(None)).unwrap();
    let after_first = state(&conn);
    // L'exécuteur ne rejoue que les migrations au-delà de `user_version` : idempotent.
    assert_eq!(run_candidate(&mut conn, &candidate(None)).unwrap(), 3);
    assert_eq!(state(&conn), after_first);
}

#[test]
fn une_ancienne_version_de_l_application_refuse_une_base_v3_sans_la_modifier() {
    let dir = tempfile::tempdir().unwrap();
    let (mut conn, path) = v2_file(dir.path());
    seed(&conn);
    run_candidate(&mut conn, &candidate(None)).unwrap();
    drop(conn);
    let before = std::fs::read(&path).unwrap();

    let old_app = Connection::open(&path).unwrap();
    // Version supportée par l'application actuelle : 2.
    let refused = check_schema_version_against(&old_app, 2);
    assert!(matches!(refused, Err(StorageError::NewerSchema { found: 3, supported: 2 })), "{refused:?}");
    drop(old_app);
    assert_eq!(std::fs::read(&path).unwrap(), before, "le refus ne modifie pas le fichier");
}

// --- Variantes explorées ---------------------------------------------------------------------

/// Variante naïve : supprimer `tasks` sans détacher les liens, clés étrangères actives.
#[test]
fn variante_naive_drop_direct_est_refusee_et_ne_laisse_rien() {
    let mut conn = fresh_v2();
    seed(&conn);
    let before = state(&conn);
    let sql = [stage("save_links"), stage("create_new"), stage("copy"), stage("swap")].concat();
    let error = run_candidate(&mut conn, &sql).unwrap_err().to_string();
    eprintln!("VARIANTE_NAIVE_ERREUR={error}");
    assert!(error.contains("FOREIGN KEY"), "{error}");
    assert_eq!(state(&conn), before);
}

/// Variante « renommer d'abord » : `ALTER TABLE tasks RENAME TO tasks_old` réécrit les références.
#[test]
fn variante_renommer_d_abord_reecrit_les_references_des_captures() {
    let conn = fresh_v2();
    seed(&conn);
    conn.execute_batch("ALTER TABLE tasks RENAME TO tasks_old;").unwrap();
    let items_sql: String =
        conn.query_row("SELECT sql FROM sqlite_master WHERE name = 'inbox_items'", [], |r| r.get(0)).unwrap();
    eprintln!("VARIANTE_RENOMMER_SQL_INBOX_ITEMS={items_sql}");
    assert!(
        items_sql.contains("tasks_old"),
        "la référence de la capture suit le renommage : renommer d'abord serait dangereux"
    );
}

/// Variante `defer_foreign_keys` : comportement observé, sans détachement des liens.
#[test]
fn variante_defer_foreign_keys_comportement_observe() {
    let mut conn = fresh_v2();
    seed(&conn);
    let before = state(&conn);
    let sql = [
        "PRAGMA defer_foreign_keys = ON;\n".to_string(),
        stage("create_new"),
        stage("copy"),
        stage("swap"),
        stage("objects"),
        "CREATE TEMP TABLE _g (ok INTEGER NOT NULL CHECK (ok = 1));
         INSERT INTO _g SELECT (SELECT count(*) FROM pragma_foreign_key_check) = 0;
         DROP TABLE _g;"
            .to_string(),
    ]
    .concat();
    let outcome = run_candidate(&mut conn, &sql);
    eprintln!("VARIANTE_DEFER_RESULTAT={:?}", outcome.as_ref().map(|v| *v).map_err(|e| e.to_string()));
    match outcome {
        Err(_) => assert_eq!(state(&conn), before, "échec : retour exact à la v2"),
        Ok(3) => {
            assert_eq!(state(&conn).fk_violations, 0);
            assert_eq!(state(&conn).integrity, "ok");
        }
        Ok(other) => panic!("version inattendue {other}"),
    }
}

// --- Migration de production : fichier complet et chemins de version -------------------------

#[test]
fn le_fichier_de_production_complet_migre_v2_vers_v3_par_le_vrai_chemin() {
    let mut conn = fresh_v2();
    seed(&conn);
    let before = state(&conn);
    let before_tasks = old_tasks(&conn);
    let destination_kinds = kinds(&conn);

    // `migrate` : l'exécuteur de production, avec la liste réelle des migrations.
    assert_eq!(super::migrate(&mut conn).unwrap(), super::SCHEMA_VERSION);
    assert_eq!(super::SCHEMA_VERSION, 3);

    let after = state(&conn);
    assert_eq!(after.user_version, 3);
    assert_eq!(after.integrity, "ok");
    assert_eq!(after.fk_violations, 0);
    assert!(after.temp_objects.is_empty());
    assert_eq!(after.inbox_items, before.inbox_items);
    assert_eq!(after.destinations, before.destinations);
    assert_projection(&before_tasks, &destination_kinds, &conn);
}

#[test]
fn les_etapes_recollees_equivalent_au_fichier_complet() {
    // Le découpage des tests ne perd ni n'ajoute aucune instruction.
    let all = stages();
    assert_eq!(
        all.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        ["guard", "save_links", "create_new", "copy", "verify_copy", "detach", "swap", "objects", "restore", "final"]
    );
    let strip = |text: &str| -> String {
        text.lines()
            .filter(|l| !l.trim_start().starts_with("--") && !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&candidate(None)), strip(PRODUCTION_SQL));
}

#[test]
fn une_base_v1_atteint_la_v3_en_conservant_les_captures() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure(&conn).unwrap();
    migrate_with(&mut conn, &MIGRATIONS[..1]).unwrap();
    assert_eq!(user_version(&conn), 1);
    for (id, content, destination, created, deleted) in [
        ("c1", "Première capture\navec retour", Some("finances"), 10, None),
        ("c2", "Capture 🎂 sans classement", None, 20, None),
        ("c3", "Capture supprimée", Some("externe"), 30, Some(31)),
    ] {
        conn.execute(
            "INSERT INTO inbox_items (id, content, destination_id, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
            rusqlite::params![id, content, destination, created, deleted],
        )
        .unwrap();
    }
    let before = dump(&conn, "inbox_items");

    assert_eq!(super::migrate(&mut conn).unwrap(), 3);

    assert_eq!(user_version(&conn), 3);
    assert_eq!(state(&conn).integrity, "ok");
    assert_eq!(state(&conn).fk_violations, 0);
    // Colonnes d'origine strictement identiques ; colonnes de conversion vides.
    let after = dump(&conn, "inbox_items");
    assert_eq!(after.len(), before.len());
    for (old, new) in before.iter().zip(&after) {
        assert_eq!(&new[..6], &old[..6]);
        assert_eq!(&new[6..], &[Value::Null, Value::Null]);
    }
    assert_eq!(new_tasks(&conn).len(), 0);
}

#[test]
fn une_base_deja_en_v3_n_est_pas_modifiee() {
    let mut conn = fresh_v2();
    seed(&conn);
    super::migrate(&mut conn).unwrap();
    let first = state(&conn);
    assert_eq!(super::migrate(&mut conn).unwrap(), 3);
    assert_eq!(state(&conn), first);
}

#[test]
fn une_migration_qui_laisse_une_violation_de_cle_etrangere_est_annulee_par_l_executeur() {
    // Garde-fou de l'exécuteur lui-même (indépendant des gardes du fichier SQL) : une migration
    // qui crée une violation est annulée AVANT la validation.
    let mut conn = fresh_v2();
    seed(&conn);
    let before = state(&conn);
    let breaking = "PRAGMA defer_foreign_keys = ON;
        UPDATE inbox_items SET converted_task_id = 'inexistante'
        WHERE id = (SELECT id FROM inbox_items WHERE converted_task_id IS NOT NULL LIMIT 1);";
    let error = migrate_with(&mut conn, &[MIGRATIONS[0], MIGRATIONS[1], breaking]).unwrap_err();
    assert!(matches!(error, StorageError::ForeignKeys { migration: 3, .. }), "{error:?}");
    assert_eq!(state(&conn), before);
}

#[test]
fn une_migration_deja_appliquee_par_un_autre_processus_n_est_pas_rejouee() {
    // Deux connexions sur le même fichier : la seconde « croit » devoir migrer (version lue avant
    // le verrou) mais relit la version sous le verrou IMMEDIATE et ne rejoue rien.
    let dir = tempfile::tempdir().unwrap();
    let (mut first, path) = v2_file(dir.path());
    seed(&first);
    let mut second = Connection::open(&path).unwrap();
    configure(&second).unwrap();
    assert_eq!(user_version(&second), 2);

    super::migrate(&mut first).unwrap();
    assert_eq!(super::migrate(&mut second).unwrap(), 3);
    assert_eq!(state(&second).integrity, "ok");
    assert_eq!(new_tasks(&second).len(), 8);
}
