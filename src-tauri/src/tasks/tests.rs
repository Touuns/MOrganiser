//! Tests de la conversion en tâche : bases temporaires ou en mémoire, jamais Dev ni Stable.

use std::collections::HashMap;
use std::sync::{Arc, Barrier};

use super::*;
use crate::inbox::{self, InboxFilter, Scope};
use crate::storage;

const T0: i64 = 1_760_000_000_000;

fn db() -> Connection {
    storage::open_in_memory()
}

fn capture(conn: &Connection, content: &str, destination: Option<&str>, at: i64) -> InboxItem {
    inbox::create_item(conn, content, destination, T0 + at).unwrap()
}

fn convert(conn: &Connection, item: &InboxItem, at: i64) -> Result<Conversion, TaskError> {
    convert_item(conn, &item.id, item.updated_at, None, T0 + at)
}

fn active_box(conn: &Connection) -> Vec<InboxItem> {
    inbox::list_items(conn, &InboxFilter::All, Scope::Active, None, 200).unwrap().items
}

fn converted_box(conn: &Connection) -> Vec<InboxItem> {
    inbox::list_items(conn, &InboxFilter::All, Scope::Converted, None, 200).unwrap().items
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

/// Contenu intégral des deux tables, pour prouver qu'un refus n'a rien modifié.
fn snapshot(conn: &Connection) -> String {
    let mut out = String::new();
    for sql in [
        "SELECT id, content, destination_id, created_at, updated_at, deleted_at, converted_at, converted_task_id
         FROM inbox_items ORDER BY id",
        "SELECT id, title, details, status, destination_id, origin_inbox_item_id, created_at, updated_at, deleted_at
         FROM tasks ORDER BY id",
    ] {
        let mut stmt = conn.prepare(sql).unwrap();
        let columns = stmt.column_count();
        let rows = stmt
            .query_map([], |row| {
                (0..columns)
                    .map(|i| row.get::<_, rusqlite::types::Value>(i).map(|v| format!("{v:?}")))
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap();
        for row in rows {
            out.push_str(&row.unwrap().join("|"));
            out.push('\n');
        }
    }
    out
}

/// Les invariants du module, vérifiés après chaque scénario.
fn assert_invariants(conn: &Connection) {
    assert_eq!(count(conn, "SELECT count(*) FROM pragma_foreign_key_check"), 0, "clés étrangères");
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(
        count(
            conn,
            "SELECT count(*) FROM (SELECT origin_inbox_item_id FROM tasks
             WHERE deleted_at IS NULL AND origin_inbox_item_id IS NOT NULL
             GROUP BY origin_inbox_item_id HAVING count(*) > 1)"
        ),
        0,
        "deux tâches actives pour une même capture"
    );
    // Capture convertie  <=>  une tâche active pointe vers elle ET elle pointe vers cette tâche.
    let mut stmt = conn
        .prepare("SELECT id, converted_at, converted_task_id FROM inbox_items")
        .unwrap();
    let items: Vec<(String, Option<i64>, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for (id, converted_at, task_id) in items {
        let active: Option<String> = conn
            .query_row(
                "SELECT id FROM tasks WHERE origin_inbox_item_id = ?1 AND deleted_at IS NULL",
                [&id],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(converted_at.is_some(), task_id.is_some(), "{id}: converted_at et converted_task_id");
        assert_eq!(active, task_id, "{id}: la tâche active et le pointeur de la capture divergent");
    }
}

// --- Conversion ---

#[test]
fn convertir_cree_une_tache_et_garde_la_capture_intacte() {
    let conn = db();
    let item = capture(&conn, "Appeler la banque\nDemander le relevé de mars", Some("finances"), 1);
    let before = snapshot(&conn);

    let done = convert(&conn, &item, 10).unwrap();

    let task = &done.task;
    assert_eq!(task.title, "Appeler la banque");
    assert_eq!(task.details, "Appeler la banque\nDemander le relevé de mars"); // texte intégral
    assert_eq!(task.status, TaskStatus::Todo);
    assert_eq!(task.destination_id.as_deref(), Some("finances")); // copiée telle quelle
    assert_eq!(task.origin_inbox_item_id.as_deref(), Some(item.id.as_str()));
    assert_eq!((task.created_at, task.updated_at), (T0 + 10, T0 + 10));
    assert_eq!(task.deleted_at, None);
    assert_eq!(uuid::Uuid::parse_str(&task.id).unwrap().get_version_num(), 7);

    // La capture : contenu, destination et surtout `updated_at` inchangés.
    let after = &done.item;
    assert_eq!(after.id, item.id);
    assert_eq!(after.content, item.content);
    assert_eq!(after.destination_id, item.destination_id);
    assert_eq!((after.created_at, after.updated_at), (item.created_at, item.updated_at));
    assert_eq!(after.deleted_at, None);
    assert_eq!(after.converted_at, Some(T0 + 10));
    assert_eq!(after.converted_task_id.as_deref(), Some(task.id.as_str()));
    assert_ne!(before, snapshot(&conn));
    assert_invariants(&conn);
}

#[test]
fn la_capture_quitte_la_boite_et_rejoint_les_traitees() {
    let conn = db();
    let a = capture(&conn, "A", None, 1);
    let b = capture(&conn, "B", None, 2);
    convert(&conn, &a, 10).unwrap();

    let page = inbox::list_items(&conn, &InboxFilter::All, Scope::Active, None, 50).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, b.id);
    let converted = converted_box(&conn);
    assert_eq!(converted.len(), 1);
    assert_eq!(converted[0].id, a.id);
    // Hors corbeille aussi, et le filtre par destination ne la ressort pas.
    assert_eq!(
        inbox::list_items(&conn, &InboxFilter::Unclassified, Scope::Active, None, 50).unwrap().total,
        1
    );
    assert_eq!(inbox::list_items(&conn, &InboxFilter::All, Scope::Trash, None, 50).unwrap().total, 0);
}

#[test]
fn la_destination_est_copiee_sans_interpretation() {
    let conn = db();
    let none = capture(&conn, "Sans destination", None, 1);
    let inv = capture(&conn, "Vendre la console", Some("inventaire"), 2);
    let moi = capture(&conn, "Pour moi", Some("moi"), 3);
    assert_eq!(convert(&conn, &none, 10).unwrap().task.destination_id, None);
    assert_eq!(convert(&conn, &inv, 11).unwrap().task.destination_id.as_deref(), Some("inventaire"));
    assert_eq!(convert(&conn, &moi, 12).unwrap().task.destination_id.as_deref(), Some("moi"));
}

#[test]
fn le_texte_est_conserve_a_l_identique_y_compris_les_cas_difficiles() {
    let conn = db();
    for (i, text) in [
        "'; DROP TABLE tasks; --",
        "Accents éàü, apostrophe d'un jour, 日本語 et 😀",
        "<script>alert(1)</script>",
        &"x".repeat(10_000),
        "Ligne 1\r\nLigne 2\n\n\nLigne 5",
    ]
    .iter()
    .enumerate()
    {
        let item = capture(&conn, text, None, i as i64);
        let done = convert(&conn, &item, 100 + i as i64).unwrap();
        assert_eq!(done.task.details, item.content, "détails");
        assert_eq!(done.item.content, item.content, "capture");
        assert!(done.task.title.chars().count() <= MAX_TITLE_CHARS);
    }
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 5);
    assert_invariants(&conn);
}

#[test]
fn titre_propose_premiere_ligne_120_caracteres_au_plus() {
    assert_eq!(suggest_title("Une idée\nAutre ligne"), "Une idée");
    assert_eq!(suggest_title("   \n\n  Après des lignes vides  \nSuite"), "Après des lignes vides");
    assert_eq!(suggest_title("  Beaucoup   d'espaces\t ici "), "Beaucoup d'espaces ici");
    assert_eq!(suggest_title("Première\r\nSeconde"), "Première");
    let exact = "é".repeat(120);
    assert_eq!(suggest_title(&exact), exact);
    let long = "é".repeat(500);
    let cut = suggest_title(&long);
    assert_eq!(cut.chars().count(), 120);
    assert!(cut.ends_with('…'));
    // Aucun découpage au milieu d'un caractère multi-octets.
    let emoji = "😀".repeat(200);
    assert_eq!(suggest_title(&emoji).chars().count(), 120);
}

#[test]
fn titre_fourni_est_nettoye_et_valide() {
    let conn = db();
    let item = capture(&conn, "Texte long de la capture", None, 1);

    let done = convert_item(&conn, &item.id, item.updated_at, Some("  Mon   titre\n ici  "), T0 + 5).unwrap();
    assert_eq!(done.task.title, "Mon titre ici");
    assert_eq!(done.task.details, "Texte long de la capture"); // le titre modifié ne touche pas aux détails

    let other = capture(&conn, "Autre", None, 2);
    let too_long = "a".repeat(121);
    for (title, expected) in [("", "empty_title"), ("  \n ", "empty_title"), (too_long.as_str(), "title_too_long")] {
        let before = snapshot(&conn);
        let error = convert_item(&conn, &other.id, other.updated_at, Some(title), T0 + 6).unwrap_err();
        assert_eq!(error.code(), expected);
        assert_eq!(before, snapshot(&conn), "un titre refusé ne modifie rien");
    }
    let ok = convert_item(&conn, &other.id, other.updated_at, Some(&"a".repeat(120)), T0 + 7).unwrap();
    assert_eq!(ok.task.title.chars().count(), 120);
}

#[test]
fn une_capture_deja_convertie_est_refusee_sans_doublon() {
    let conn = db();
    let item = capture(&conn, "Une seule tâche", None, 1);
    let first = convert(&conn, &item, 10).unwrap();
    let before = snapshot(&conn);

    match convert(&conn, &item, 20).unwrap_err() {
        TaskError::AlreadyConverted { task_id } => assert_eq!(task_id, first.task.id),
        other => panic!("attendu already_converted, obtenu {other:?}"),
    }
    assert_eq!(before, snapshot(&conn));
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 1);
}

#[test]
fn une_capture_a_la_corbeille_est_refusee_puis_acceptee_apres_restauration() {
    let conn = db();
    let item = capture(&conn, "À la corbeille", None, 1);
    inbox::trash_item(&conn, &item.id, T0 + 5).unwrap();
    let before = snapshot(&conn);
    assert_eq!(convert(&conn, &item, 10).unwrap_err().code(), "trashed");
    assert_eq!(before, snapshot(&conn));

    inbox::restore_item(&conn, &item.id).unwrap();
    assert!(convert(&conn, &item, 20).is_ok());
    assert_invariants(&conn);
}

#[test]
fn une_version_perimee_est_refusee_proprement() {
    let conn = db();
    let item = capture(&conn, "Version 1", None, 1);
    let edited = inbox::update_item(&conn, &item.id, "Version 2", None, item.updated_at, T0 + 5).unwrap();
    let before = snapshot(&conn);

    // L'utilisateur avait ouvert la version 1 : rien n'est converti « à l'aveugle ».
    assert_eq!(convert(&conn, &item, 10).unwrap_err().code(), "version_conflict");
    assert_eq!(before, snapshot(&conn));
    // Avec la version actuelle, la conversion reprend le texte actuel.
    let done = convert(&conn, &edited, 11).unwrap();
    assert_eq!(done.task.details, "Version 2");
}

#[test]
fn une_capture_introuvable_est_refusee() {
    let conn = db();
    let error = convert_item(&conn, "inexistante", 1, None, T0).unwrap_err();
    assert_eq!(error.code(), "not_found");
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 0);
}

#[test]
fn echec_au_milieu_de_la_conversion_annule_tout() {
    let conn = db();
    let item = capture(&conn, "Atomicité", None, 1);
    // Fait échouer la SECONDE écriture (marquage de la capture) APRÈS l'insertion de la tâche.
    conn.execute_batch(
        "CREATE TRIGGER test_fail_mark BEFORE UPDATE OF converted_at ON inbox_items
         BEGIN SELECT RAISE(ABORT, 'panne simulée'); END;",
    )
    .unwrap();
    let before = snapshot(&conn);

    let error = convert(&conn, &item, 10).unwrap_err();
    assert_eq!(error.code(), "storage");
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 0, "la tâche insérée a été annulée");
    assert_eq!(before, snapshot(&conn));

    // La connexion reste utilisable, et la conversion réussit une fois la panne levée.
    conn.execute_batch("DROP TRIGGER test_fail_mark").unwrap();
    assert!(convert(&conn, &item, 11).is_ok());
    assert_invariants(&conn);
}

// --- Annulation et reconversion ---

#[test]
fn annuler_remet_la_capture_a_l_identique_et_conserve_la_tache() {
    let conn = db();
    let item = capture(&conn, "À annuler", Some("moi"), 1);
    let done = convert(&conn, &item, 10).unwrap();

    let cancelled = cancel_conversion(&conn, &done.task.id, T0 + 20).unwrap();

    // Capture : exactement l'état d'origine (même updated_at, plus de marque de conversion).
    assert_eq!(cancelled.item, item);
    assert_eq!(active_box(&conn).len(), 1);
    assert!(converted_box(&conn).is_empty());
    // Tâche : conservée, annulée (récupérable dans la base), détails intacts.
    assert_eq!(cancelled.task.deleted_at, Some(T0 + 20));
    assert_eq!(cancelled.task.details, "À annuler");
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 1);
    assert_eq!(list_tasks(&conn, None, 50).unwrap().total, 0);
    assert_invariants(&conn);
}

#[test]
fn reconversion_apres_annulation_cree_une_nouvelle_tache_et_garde_l_ancienne() {
    let conn = db();
    let item = capture(&conn, "Deux essais", None, 1);
    let first = convert(&conn, &item, 10).unwrap();
    cancel_conversion(&conn, &first.task.id, T0 + 20).unwrap();

    let second = convert(&conn, &item, 30).unwrap();
    assert_ne!(second.task.id, first.task.id);
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 2);
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks WHERE deleted_at IS NULL"), 1);
    assert_eq!(get_task(&conn, &first.task.id).unwrap().deleted_at, Some(T0 + 20));
    assert_eq!(second.item.converted_task_id.as_deref(), Some(second.task.id.as_str()));
    assert_invariants(&conn);

    // Et on peut recommencer : annulation de la seconde, troisième tâche.
    cancel_conversion(&conn, &second.task.id, T0 + 40).unwrap();
    let third = convert(&conn, &item, 50).unwrap();
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 3);
    assert_eq!(third.task.details, "Deux essais");
    assert_invariants(&conn);
}

#[test]
fn annulation_refusee_si_la_tache_a_ete_modifiee_ou_avancee() {
    let conn = db();
    let item = capture(&conn, "Travail en cours", None, 1);

    // (a) titre enrichi, `updated_at` correctement avancé
    let a = convert(&conn, &item, 10).unwrap();
    conn.execute(
        "UPDATE tasks SET title = 'Titre enrichi', updated_at = updated_at + 1 WHERE id = ?1",
        [&a.task.id],
    )
    .unwrap();
    let before = snapshot(&conn);
    assert_eq!(cancel_conversion(&conn, &a.task.id, T0 + 20).unwrap_err().code(), "task_modified");
    assert_eq!(before, snapshot(&conn), "rien n'est modifié par le refus");
    assert_eq!(get_task(&conn, &a.task.id).unwrap().deleted_at, None);
    assert_invariants(&conn);

    // (b) statut avancé
    let item_b = capture(&conn, "Avancée", None, 2);
    let b = convert(&conn, &item_b, 11).unwrap();
    conn.execute(
        "UPDATE tasks SET status = 'in_progress', updated_at = updated_at + 1 WHERE id = ?1",
        [&b.task.id],
    )
    .unwrap();
    assert_eq!(cancel_conversion(&conn, &b.task.id, T0 + 21).unwrap_err().code(), "task_modified");

    // La capture, elle, reste convertie et en lecture seule.
    assert_eq!(inbox::get_item(&conn, &item.id).unwrap().converted_task_id.as_deref(), Some(a.task.id.as_str()));
    assert_invariants(&conn);
}

#[test]
fn annulations_refusees_cas_limites() {
    let conn = db();
    assert_eq!(cancel_conversion(&conn, "inconnue", T0).unwrap_err().code(), "task_not_found");

    let item = capture(&conn, "Deux fois", None, 1);
    let done = convert(&conn, &item, 10).unwrap();
    cancel_conversion(&conn, &done.task.id, T0 + 20).unwrap();
    let before = snapshot(&conn);
    assert_eq!(cancel_conversion(&conn, &done.task.id, T0 + 30).unwrap_err().code(), "task_not_active");
    assert_eq!(before, snapshot(&conn));

    // Tâche sans capture d'origine (future création directe) : rien à remettre dans la boîte.
    conn.execute(
        "INSERT INTO tasks (id, title, details, created_at, updated_at) VALUES ('directe', 'T', 'D', 1, 1)",
        [],
    )
    .unwrap();
    assert_eq!(cancel_conversion(&conn, "directe", T0).unwrap_err().code(), "no_origin");
    assert_invariants(&conn);
}

#[test]
fn annulation_atomique_si_la_restauration_de_la_capture_echoue() {
    let conn = db();
    let item = capture(&conn, "Atomicité de l'annulation", None, 1);
    let done = convert(&conn, &item, 10).unwrap();
    conn.execute_batch(
        "CREATE TRIGGER test_fail_restore BEFORE UPDATE OF converted_task_id ON inbox_items
         BEGIN SELECT RAISE(ABORT, 'panne simulée'); END;",
    )
    .unwrap();
    let before = snapshot(&conn);
    assert_eq!(cancel_conversion(&conn, &done.task.id, T0 + 20).unwrap_err().code(), "storage");
    assert_eq!(before, snapshot(&conn), "la tâche est restée active");
    conn.execute_batch("DROP TRIGGER test_fail_restore").unwrap();
    assert_invariants(&conn);
}

// --- Interaction avec la boîte (lecture seule des captures converties) ---

#[test]
fn une_capture_convertie_ne_va_pas_a_la_corbeille_et_n_est_pas_modifiable() {
    let conn = db();
    let item = capture(&conn, "Lecture seule", None, 1);
    let done = convert(&conn, &item, 10).unwrap();
    let before = snapshot(&conn);

    assert_eq!(inbox::trash_item(&conn, &item.id, T0 + 20).unwrap_err().code(), "converted");
    assert_eq!(
        inbox::update_item(&conn, &item.id, "Autre texte", None, item.updated_at, T0 + 21)
            .unwrap_err()
            .code(),
        "converted"
    );
    assert_eq!(before, snapshot(&conn));

    // Après annulation, elle redevient une capture ordinaire.
    cancel_conversion(&conn, &done.task.id, T0 + 30).unwrap();
    assert!(inbox::update_item(&conn, &item.id, "Autre texte", None, item.updated_at, T0 + 31).is_ok());
    assert_invariants(&conn);
}

#[test]
fn la_pagination_des_traitees_et_des_taches_est_sans_doublon() {
    let conn = db();
    let items: Vec<InboxItem> = (0..7).map(|i| capture(&conn, &format!("Capture {i}"), None, i)).collect();
    for (i, item) in items.iter().enumerate() {
        convert(&conn, item, 100 + i as i64).unwrap();
    }
    // Traitées : plus récemment traitée d'abord, par curseur.
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = inbox::list_items(&conn, &InboxFilter::All, Scope::Converted, cursor.as_ref(), 3).unwrap();
        assert_eq!(page.total, 7);
        seen.extend(page.items.iter().map(|i| i.content.clone()));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, (0..7).rev().map(|i| format!("Capture {i}")).collect::<Vec<_>>());

    // Tâches : même principe ; une tâche annulée n'y figure pas.
    let first_task = list_tasks(&conn, None, 200).unwrap().items[6].clone();
    cancel_conversion(&conn, &first_task.id, T0 + 500).unwrap();
    let mut titles = Vec::new();
    let mut cursor = None;
    loop {
        let page = list_tasks(&conn, cursor.as_ref(), 2).unwrap();
        assert_eq!(page.total, 6);
        titles.extend(page.items.iter().map(|t| t.title.clone()));
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(titles, (1..7).rev().map(|i| format!("Capture {i}")).collect::<Vec<_>>());
    assert_invariants(&conn);
}

// --- Déclencheurs et index SQL (défense en profondeur) ---

#[test]
fn le_declencheur_exige_un_updated_at_strictement_croissant() {
    let conn = db();
    let item = capture(&conn, "Garde", None, 1);
    let task = convert(&conn, &item, 10).unwrap().task;
    let t = T0 + 10;
    let attempt = |sql: &str| conn.execute(sql, [&task.id]).map(|_| ());
    let message = |r: rusqlite::Result<()>| r.unwrap_err().to_string();

    // Champ métier modifié SANS toucher updated_at : refusé, pour chaque champ.
    for column in ["title = 'X'", "details = 'X'", "status = 'done'", "destination_id = 'moi'"] {
        let error = message(attempt(&format!("UPDATE tasks SET {column} WHERE id = ?1")));
        assert!(error.contains("updated_at doit croitre"), "{column}: {error}");
    }
    // updated_at identique ou en recul : refusé.
    for value in [t, t - 1] {
        let sql = format!("UPDATE tasks SET title = 'X', updated_at = {value} WHERE id = ?1");
        assert!(message(attempt(&sql)).contains("updated_at doit croitre"), "{value}");
    }
    assert_eq!(get_task(&conn, &task.id).unwrap(), task, "aucune des tentatives n'a eu d'effet");

    // Avec une valeur strictement supérieure : accepté.
    attempt(&format!("UPDATE tasks SET title = 'Nouveau', updated_at = {} WHERE id = ?1", t + 1)).unwrap();
    assert_eq!(get_task(&conn, &task.id).unwrap().title, "Nouveau");
    // Et la valeur doit encore croître : retour au même updated_at refusé.
    let sql = format!("UPDATE tasks SET title = 'Encore', updated_at = {} WHERE id = ?1", t + 1);
    assert!(message(attempt(&sql)).contains("updated_at doit croitre"));
}

#[test]
fn le_declencheur_laisse_passer_ce_qui_n_est_pas_un_champ_metier() {
    let conn = db();
    let item = capture(&conn, "Souple", None, 1);
    let task = convert(&conn, &item, 10).unwrap().task;

    // Mise à jour sans changement réel de valeur : acceptée.
    conn.execute("UPDATE tasks SET title = title, status = status WHERE id = ?1", [&task.id]).unwrap();
    // deleted_at n'est pas un champ métier : l'annulation passe sans toucher updated_at.
    conn.execute("UPDATE tasks SET deleted_at = 5 WHERE id = ?1", [&task.id]).unwrap();
    assert_eq!(get_task(&conn, &task.id).unwrap().updated_at, task.updated_at);
}

#[test]
fn la_provenance_et_l_identifiant_sont_immuables() {
    let conn = db();
    let a = capture(&conn, "A", None, 1);
    let b = capture(&conn, "B", None, 2);
    let task = convert(&conn, &a, 10).unwrap().task;
    for sql in [
        format!("UPDATE tasks SET origin_inbox_item_id = '{}' WHERE id = ?1", b.id),
        "UPDATE tasks SET origin_inbox_item_id = NULL WHERE id = ?1".to_string(),
        "UPDATE tasks SET created_at = created_at + 1 WHERE id = ?1".to_string(),
        "UPDATE tasks SET id = 'autre' WHERE id = ?1".to_string(),
    ] {
        let error = conn.execute(&sql, [&task.id]).unwrap_err().to_string();
        assert!(error.contains("immuables"), "{sql}: {error}");
    }
    assert_eq!(get_task(&conn, &task.id).unwrap(), task);
}

#[test]
fn l_index_partiel_refuse_deux_taches_actives_mais_pas_apres_annulation() {
    let conn = db();
    let item = capture(&conn, "Index", None, 1);
    let insert = |id: &str| {
        conn.execute(
            "INSERT INTO tasks (id, title, details, origin_inbox_item_id, created_at, updated_at)
             VALUES (?1, 'T', 'D', ?2, 1, 1)",
            params![id, item.id],
        )
    };
    insert("t1").unwrap();
    assert!(insert("t2").is_err(), "deuxième tâche active pour la même capture");
    conn.execute("UPDATE tasks SET deleted_at = 9 WHERE id = 't1'", []).unwrap();
    insert("t2").unwrap(); // l'ancienne est annulée : la place est libre
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks WHERE origin_inbox_item_id IS NOT NULL"), 2);
}

#[test]
fn le_schema_refuse_les_valeurs_invalides() {
    let conn = db();
    let insert = |title: &str, details: &str, status: &str| {
        conn.execute(
            "INSERT INTO tasks (id, title, details, status, created_at, updated_at)
             VALUES (hex(randomblob(8)), ?1, ?2, ?3, 1, 1)",
            params![title, details, status],
        )
    };
    assert!(insert("T", "D", "todo").is_ok());
    assert!(insert("", "D", "todo").is_err());
    assert!(insert("   ", "D", "todo").is_err());
    assert!(insert(&"a".repeat(121), "D", "todo").is_err());
    assert!(insert("T", "", "todo").is_err());
    assert!(insert("T", "D", "inconnu").is_err());
    for status in ["in_progress", "waiting", "blocked", "done"] {
        assert!(insert("T", "D", status).is_ok(), "{status}");
    }
}

// --- Format JSON ---

#[test]
fn le_format_json_correspond_a_ce_qu_attend_l_interface() {
    let conn = db();
    let item = capture(&conn, "Format", Some("moi"), 1);
    let done = convert(&conn, &item, 10).unwrap();
    let json = serde_json::to_value(&done).unwrap();
    assert_eq!(json["task"]["status"], "todo");
    assert_eq!(json["task"]["originInboxItemId"], item.id);
    assert_eq!(json["task"]["destinationId"], "moi");
    assert_eq!(json["task"]["deletedAt"], serde_json::Value::Null);
    assert_eq!(json["item"]["convertedTaskId"], json["task"]["id"]);
    assert_eq!(json["item"]["convertedAt"], T0 + 10);

    let error = serde_json::to_value(convert(&conn, &item, 20).unwrap_err()).unwrap();
    assert_eq!(error["code"], "already_converted");
    assert_eq!(error["taskId"], json["task"]["id"]);
    assert!(!error["message"].as_str().unwrap().contains("Format"), "le texte n'est jamais dans l'erreur");
    let other = serde_json::to_value(TaskError::Modified).unwrap();
    assert_eq!(other["code"], "task_modified");
    assert!(other.get("taskId").is_none());
}

// --- Séquences aléatoires et concurrence ---

#[test]
fn sequences_aleatoires_conservent_les_invariants_et_ne_perdent_rien() {
    let conn = db();
    let items: Vec<InboxItem> = (0..4).map(|i| capture(&conn, &format!("Capture {i}"), None, i)).collect();
    // Texte attendu de chaque capture (suivi des modifications légitimes).
    let mut expected: HashMap<String, String> =
        items.iter().map(|i| (i.id.clone(), i.content.clone())).collect();
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut clock = 1_000;
    let mut conversions = 0;
    let mut cancellations = 0;

    for step in 0..400 {
        clock += 1;
        let item = &items[(next() % items.len() as u64) as usize];
        let current = inbox::get_item(&conn, &item.id).unwrap();
        match next() % 6 {
            0 | 1 => {
                if convert_item(&conn, &item.id, current.updated_at, None, T0 + clock).is_ok() {
                    conversions += 1;
                }
            }
            2 => {
                if let Some(task_id) = current.converted_task_id.clone() {
                    if cancel_conversion(&conn, &task_id, T0 + clock).is_ok() {
                        cancellations += 1;
                    }
                }
            }
            3 => {
                let _ = inbox::trash_item(&conn, &item.id, T0 + clock);
            }
            4 => {
                let _ = inbox::restore_item(&conn, &item.id);
            }
            _ => {
                let text = format!("Capture {} v{step}", item.id.len());
                if inbox::update_item(&conn, &item.id, &text, None, current.updated_at, T0 + clock).is_ok() {
                    expected.insert(item.id.clone(), text);
                }
            }
        }
        // Parfois, une tâche active est « enrichie » (comme le fera la brique 003).
        if next() % 7 == 0 {
            let _ = conn.execute(
                "UPDATE tasks SET status = 'in_progress', updated_at = updated_at + 1
                 WHERE id = (SELECT id FROM tasks WHERE deleted_at IS NULL ORDER BY id LIMIT 1)",
                [],
            );
        }
        assert_invariants(&conn);
        for item in &items {
            let now = inbox::get_item(&conn, &item.id).unwrap(); // jamais supprimée physiquement
            assert_eq!(now.content, expected[&item.id], "texte de la capture altéré");
        }
    }
    assert_eq!(count(&conn, "SELECT count(*) FROM inbox_items"), 4);
    assert!(conversions > 10 && cancellations > 3, "scénario trop pauvre : {conversions}/{cancellations}");
    // Une tâche (même annulée) garde ses détails d'origine.
    assert_eq!(
        count(&conn, "SELECT count(*) FROM tasks WHERE length(trim(details)) = 0"),
        0
    );
}

fn file_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(storage::DB_FILE_NAME);
    (dir, path)
}

#[test]
fn conversions_simultanees_de_la_meme_capture_une_seule_tache() {
    let (_dir, path) = file_db();
    let item = {
        let conn = storage::open(&path).unwrap();
        capture(&conn, "Course", None, 1)
    };
    const THREADS: usize = 8;
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|n| {
            let (path, id, barrier) = (path.clone(), item.id.clone(), barrier.clone());
            std::thread::spawn(move || {
                let conn = storage::open(&path).unwrap(); // une connexion par fil, comme deux fenêtres
                barrier.wait();
                convert_item(&conn, &id, item.updated_at, None, T0 + 10 + n as i64).map(|c| c.task.id)
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    assert_eq!(winners.len(), 1, "exactement une conversion : {results:?}");
    for result in &results {
        if let Err(error) = result {
            assert_eq!(error.code(), "already_converted", "{error:?}");
        }
    }
    let conn = storage::open(&path).unwrap();
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 1);
    assert_invariants(&conn);
}

#[test]
fn conversion_et_annulation_simultanees_restent_coherentes() {
    let (_dir, path) = file_db();
    let (item, task_id) = {
        let conn = storage::open(&path).unwrap();
        let item = capture(&conn, "Va-et-vient", None, 1);
        let done = convert(&conn, &item, 10).unwrap();
        (item, done.task.id)
    };
    let barrier = Arc::new(Barrier::new(2));
    let canceller = {
        let (path, task_id, barrier) = (path.clone(), task_id.clone(), barrier.clone());
        std::thread::spawn(move || {
            let conn = storage::open(&path).unwrap();
            barrier.wait();
            cancel_conversion(&conn, &task_id, T0 + 20).map(|_| ())
        })
    };
    let converter = {
        let (path, id, barrier) = (path.clone(), item.id.clone(), barrier.clone());
        std::thread::spawn(move || {
            let conn = storage::open(&path).unwrap();
            barrier.wait();
            convert_item(&conn, &id, item.updated_at, None, T0 + 21).map(|_| ())
        })
    };
    let cancelled = canceller.join().unwrap();
    let converted = converter.join().unwrap();
    // Quel que soit l'ordre : l'annulation réussit, et la reconversion réussit seulement si
    // elle passe après ; jamais deux tâches actives, jamais d'état incohérent.
    assert!(cancelled.is_ok(), "{cancelled:?}");
    if let Err(error) = &converted {
        assert_eq!(error.code(), "already_converted");
    }
    let conn = storage::open(&path).unwrap();
    assert_invariants(&conn);
    assert!(count(&conn, "SELECT count(*) FROM tasks WHERE deleted_at IS NULL") <= 1);
}

#[test]
fn conversions_simultanees_de_captures_differentes_reussissent_toutes() {
    let (_dir, path) = file_db();
    let items: Vec<InboxItem> = {
        let conn = storage::open(&path).unwrap();
        (0..6).map(|i| capture(&conn, &format!("Capture {i}"), None, i)).collect()
    };
    let barrier = Arc::new(Barrier::new(items.len()));
    let handles: Vec<_> = items
        .into_iter()
        .enumerate()
        .map(|(n, item)| {
            let (path, barrier) = (path.clone(), barrier.clone());
            std::thread::spawn(move || {
                let conn = storage::open(&path).unwrap();
                barrier.wait();
                convert_item(&conn, &item.id, item.updated_at, None, T0 + 10 + n as i64).map(|_| ())
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    let conn = storage::open(&path).unwrap();
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks WHERE deleted_at IS NULL"), 6);
    assert_invariants(&conn);
}
