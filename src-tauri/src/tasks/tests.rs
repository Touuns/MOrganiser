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
        "SELECT id, title, details, status, responsibility_id, section_id, start_date, due_date,
                completed_on, priority, origin_inbox_item_id, created_at, updated_at, deleted_at
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
    // « finances » est une rubrique : reprise dans sa dimension, l'univers reste vide.
    assert_eq!(task.section_id.as_deref(), Some("finances"));
    assert_eq!(task.responsibility_id, None);
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
fn la_destination_est_repartie_selon_sa_nature_sans_rien_inventer() {
    let conn = db();
    let none = capture(&conn, "Sans destination", None, 1);
    let inv = capture(&conn, "Vendre la console", Some("inventaire"), 2);
    let moi = capture(&conn, "Pour moi", Some("moi"), 3);
    let dims = |t: &Task| (t.responsibility_id.clone(), t.section_id.clone());
    // Jamais « Moi » par défaut pour une capture non classée.
    assert_eq!(dims(&convert(&conn, &none, 10).unwrap().task), (None, None));
    assert_eq!(dims(&convert(&conn, &inv, 11).unwrap().task), (None, Some("inventaire".into())));
    assert_eq!(dims(&convert(&conn, &moi, 12).unwrap().task), (Some("moi".into()), None));
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
    for column in [
        "title = 'X'",
        "details = 'X'",
        "status = 'done'",
        "responsibility_id = 'moi'",
        "section_id = 'finances'",
        "start_date = '2026-01-01'",
        "due_date = '2026-12-31'",
        "completed_on = '2026-10-14'",
        "priority = 2",
    ] {
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
    assert!(insert("T", "", "todo").is_ok(), "description facultative depuis 003-B");
    assert!(insert("T", &"a".repeat(10_001), "todo").is_err());
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
    assert_eq!(json["task"]["responsibilityId"], "moi");
    assert_eq!(json["task"]["sectionId"], serde_json::Value::Null);
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

// --- 003-A : brouillon de tâche (titre, description et destination saisis) ---

fn draft(title: Option<&str>, details: Option<&str>, destination: DestinationChoice) -> ConversionDraft {
    ConversionDraft {
        title: title.map(str::to_owned),
        details: details.map(str::to_owned),
        destination,
        ..ConversionDraft::default()
    }
}

fn convert_draft(
    conn: &Connection,
    item: &InboxItem,
    draft: &ConversionDraft,
    at: i64,
) -> Result<Conversion, TaskError> {
    convert_item_with(conn, &item.id, item.updated_at, draft, T0 + at)
}

#[test]
fn la_description_et_la_destination_saisies_sont_enregistrees_et_la_capture_reste_intacte() {
    let conn = db();
    let item = capture(&conn, "Texte original\nsur deux lignes", Some("finances"), 1);
    let d = draft(
        Some("Mon titre"),
        Some("Description réécrite\n\nAvec un paragraphe"),
        DestinationChoice::Set { id: "inventaire".into() },
    );

    let done = convert_draft(&conn, &item, &d, 10).unwrap();

    assert_eq!(done.task.title, "Mon titre");
    assert_eq!(done.task.details, "Description réécrite\n\nAvec un paragraphe");
    // `set` d'une rubrique : placée dans sa dimension, l'univers reste vide.
    assert_eq!(done.task.section_id.as_deref(), Some("inventaire"));
    assert_eq!(done.task.responsibility_id, None);
    assert_eq!(done.task.status, TaskStatus::Todo);
    // Capture : texte, destination et dates strictement conservés ; seule la conversion est marquée.
    assert_eq!(done.item.content, "Texte original\nsur deux lignes");
    assert_eq!(done.item.destination_id.as_deref(), Some("finances"));
    assert_eq!((done.item.created_at, done.item.updated_at), (item.created_at, item.updated_at));
    assert_eq!(done.item.converted_task_id.as_deref(), Some(done.task.id.as_str()));
    let stored = inbox::get_item(&conn, &item.id).unwrap();
    assert_eq!(stored.content, item.content);
    assert_eq!(stored.destination_id, item.destination_id);
    assert_invariants(&conn);
}

#[test]
fn la_description_est_nettoyee_aux_extremites_seulement() {
    let conn = db();
    let item = capture(&conn, "Original", None, 1);
    let d = draft(None, Some("  \n  Ligne un\n\n   ligne   trois  \n "), DestinationChoice::Keep);
    let done = convert_draft(&conn, &item, &d, 10).unwrap();
    assert_eq!(done.task.details, "Ligne un\n\n   ligne   trois");
    assert_eq!(done.task.title, "Original"); // titre suggéré à partir de la CAPTURE
}

#[test]
fn sans_description_le_texte_integral_de_la_capture_est_repris() {
    let conn = db();
    let item = capture(&conn, "Première ligne\nSuite", Some("moi"), 1);
    let done = convert_draft(&conn, &item, &ConversionDraft::default(), 10).unwrap();
    assert_eq!(done.task.details, "Première ligne\nSuite");
    assert_eq!(done.task.responsibility_id.as_deref(), Some("moi"));
    assert_eq!(done.task.section_id, None);
}

#[test]
fn la_destination_peut_etre_retiree_ou_changee_sans_toucher_la_capture() {
    let conn = db();
    let cleared = capture(&conn, "Sans classement", Some("moi"), 1);
    let done = convert_draft(&conn, &cleared, &draft(None, None, DestinationChoice::Clear), 10).unwrap();
    assert_eq!((done.task.responsibility_id.clone(), done.task.section_id.clone()), (None, None));
    assert_eq!(done.item.destination_id.as_deref(), Some("moi"));

    let unclassified = capture(&conn, "Capture libre", None, 2);
    let done = convert_draft(
        &conn,
        &unclassified,
        &draft(None, None, DestinationChoice::Set { id: "externe".into() }),
        11,
    )
    .unwrap();
    assert_eq!(done.task.responsibility_id.as_deref(), Some("externe"));
    assert_eq!(done.item.destination_id, None);
    assert_invariants(&conn);
}

#[test]
fn une_description_vide_ou_blanche_est_acceptee_depuis_003_b() {
    let conn = db();
    for (n, blank) in ["", "   ", "\n\t \n"].into_iter().enumerate() {
        let item = capture(&conn, "Texte d'origine à conserver", None, n as i64);
        let done =
            convert_draft(&conn, &item, &draft(None, Some(blank), DestinationChoice::Keep), 10).unwrap();
        // Vraiment vide : jamais remplacée par le texte de la capture, qui reste intact.
        assert_eq!(done.task.details, "");
        assert_eq!(done.item.content, "Texte d'origine à conserver");
    }
    assert_invariants(&conn);
}

#[test]
fn les_longueurs_sont_validees_cote_rust() {
    let conn = db();
    let item = capture(&conn, "Longueurs", None, 1);
    let too_long = "é".repeat(inbox::MAX_CONTENT_CHARS + 1);
    let before = snapshot(&conn);
    assert_eq!(
        convert_draft(&conn, &item, &draft(None, Some(&too_long), DestinationChoice::Keep), 10)
            .unwrap_err()
            .code(),
        "details_too_long"
    );
    assert_eq!(
        convert_draft(&conn, &item, &draft(Some(&"a".repeat(121)), None, DestinationChoice::Keep), 10)
            .unwrap_err()
            .code(),
        "title_too_long"
    );
    assert_eq!(
        convert_draft(&conn, &item, &draft(Some("  "), None, DestinationChoice::Keep), 10)
            .unwrap_err()
            .code(),
        "empty_title"
    );
    assert_eq!(before, snapshot(&conn));

    // Les bornes exactes passent (caractères multi-octets comptés en caractères, pas en octets).
    let exact = "é".repeat(inbox::MAX_CONTENT_CHARS);
    let done = convert_draft(
        &conn,
        &item,
        &draft(Some(&"a".repeat(120)), Some(&exact), DestinationChoice::Keep),
        11,
    )
    .unwrap();
    assert_eq!(done.task.details.chars().count(), inbox::MAX_CONTENT_CHARS);
    assert_eq!(done.item.content, "Longueurs");
}

#[test]
fn une_destination_inconnue_ou_archivee_est_refusee_une_destination_inchangee_reste_acceptee() {
    let conn = db();
    let item = capture(&conn, "Classement", Some("inventaire"), 1);
    let before = snapshot(&conn);

    for id in ["inconnue", "", "'; DROP TABLE tasks; --"] {
        let error = convert_draft(
            &conn,
            &item,
            &draft(None, None, DestinationChoice::Set { id: id.into() }),
            10,
        )
        .unwrap_err();
        assert_eq!(error.code(), "unknown_destination", "{id:?}");
        assert!(!error.to_string().contains("DROP"), "le message ne reprend pas la saisie");
    }
    assert_eq!(before, snapshot(&conn));

    // Archivée depuis : refusée comme nouveau choix, mais conservée si c'est celle de la capture.
    conn.execute("UPDATE destinations SET archived_at = 5 WHERE id IN ('finances', 'inventaire')", [])
        .unwrap();
    let error = convert_draft(
        &conn,
        &item,
        &draft(None, None, DestinationChoice::Set { id: "finances".into() }),
        11,
    )
    .unwrap_err();
    assert_eq!(error.code(), "unknown_destination");
    let kept = convert_draft(
        &conn,
        &item,
        &draft(None, None, DestinationChoice::Set { id: "inventaire".into() }),
        12,
    )
    .unwrap();
    assert_eq!(kept.task.section_id.as_deref(), Some("inventaire"));
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 1);
    assert_invariants(&conn);
}

#[test]
fn le_brouillon_respecte_les_refus_de_la_brique_002() {
    let conn = db();
    let item = capture(&conn, "Version 1", None, 1);
    let d = draft(Some("Titre"), Some("Description"), DestinationChoice::Keep);

    // Version périmée.
    let edited = inbox::update_item(&conn, &item.id, "Version 2", None, item.updated_at, T0 + 5).unwrap();
    let before = snapshot(&conn);
    assert_eq!(convert_draft(&conn, &item, &d, 10).unwrap_err().code(), "version_conflict");
    assert_eq!(before, snapshot(&conn));

    // Corbeille.
    inbox::trash_item(&conn, &item.id, T0 + 6).unwrap();
    assert_eq!(convert_draft(&conn, &edited, &d, 11).unwrap_err().code(), "trashed");
    inbox::restore_item(&conn, &item.id).unwrap();

    // Déjà convertie : pas de doublon, la tâche existante est désignée.
    let first = convert_draft(&conn, &edited, &d, 12).unwrap();
    match convert_draft(&conn, &edited, &d, 13).unwrap_err() {
        TaskError::AlreadyConverted { task_id } => assert_eq!(task_id, first.task.id),
        other => panic!("attendu already_converted, obtenu {other:?}"),
    }
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 1);
    assert_invariants(&conn);
}

#[test]
fn echec_au_milieu_d_une_conversion_avec_brouillon_annule_tout() {
    let conn = db();
    let item = capture(&conn, "Atomicité du brouillon", Some("moi"), 1);
    conn.execute_batch(
        "CREATE TRIGGER test_fail_mark BEFORE UPDATE OF converted_at ON inbox_items
         BEGIN SELECT RAISE(ABORT, 'panne simulée'); END;",
    )
    .unwrap();
    let before = snapshot(&conn);
    let d = draft(
        Some("Titre"),
        Some("Description saisie"),
        DestinationChoice::Set { id: "externe".into() },
    );

    let error = convert_draft(&conn, &item, &d, 10).unwrap_err();
    assert_eq!(error.code(), "storage");
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 0, "la tâche insérée a été annulée");
    assert_eq!(before, snapshot(&conn));

    conn.execute_batch("DROP TRIGGER test_fail_mark").unwrap();
    assert!(convert_draft(&conn, &item, &d, 11).is_ok());
    assert_invariants(&conn);
}

#[test]
fn annuler_une_conversion_avec_brouillon_remet_la_capture_a_l_identique() {
    let conn = db();
    let item = capture(&conn, "Capture d'origine", Some("moi"), 1);
    let d = draft(
        Some("Titre perso"),
        Some("Description très différente"),
        DestinationChoice::Set { id: "administratif".into() },
    );
    let done = convert_draft(&conn, &item, &d, 10).unwrap();

    // La garde de la brique 002 est inchangée : tâche intacte => annulation possible.
    let cancelled = cancel_conversion(&conn, &done.task.id, T0 + 20).unwrap();
    assert_eq!(cancelled.item, item);
    assert_eq!(cancelled.task.deleted_at, Some(T0 + 20));
    assert_eq!(cancelled.task.details, "Description très différente"); // conservée, annulée
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 1);
    assert_invariants(&conn);

    // Et toujours refusée si la tâche a été modifiée ou avancée.
    let again = convert_draft(&conn, &item, &d, 30).unwrap();
    conn.execute(
        "UPDATE tasks SET status = 'in_progress', updated_at = updated_at + 1 WHERE id = ?1",
        [&again.task.id],
    )
    .unwrap();
    assert_eq!(cancel_conversion(&conn, &again.task.id, T0 + 40).unwrap_err().code(), "task_modified");
    assert_invariants(&conn);
}

#[test]
fn la_provenance_et_le_declencheur_restent_inchanges_apres_un_brouillon() {
    let conn = db();
    let item = capture(&conn, "Provenance", None, 1);
    let d = draft(Some("T"), Some("D"), DestinationChoice::Keep);
    let done = convert_draft(&conn, &item, &d, 10).unwrap();
    assert_eq!(done.task.origin_inbox_item_id.as_deref(), Some(item.id.as_str()));
    assert_eq!((done.task.created_at, done.task.updated_at), (T0 + 10, T0 + 10));
    // Le déclencheur de la brique 002 s'applique : modifier la description sans faire croître
    // updated_at est refusé, comme réécrire la provenance.
    assert!(conn
        .execute("UPDATE tasks SET details = 'autre' WHERE id = ?1", [&done.task.id])
        .is_err());
    assert!(conn
        .execute("UPDATE tasks SET origin_inbox_item_id = NULL WHERE id = ?1", [&done.task.id])
        .is_err());
}

#[test]
fn conversions_simultanees_avec_brouillons_differents_une_seule_tache() {
    let (_dir, path) = file_db();
    let item = {
        let conn = storage::open(&path).unwrap();
        capture(&conn, "Course avec brouillons", Some("moi"), 1)
    };
    const THREADS: usize = 8;
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|n| {
            let (path, id, barrier) = (path.clone(), item.id.clone(), barrier.clone());
            std::thread::spawn(move || {
                let conn = storage::open(&path).unwrap();
                let d = ConversionDraft {
                    title: Some(format!("Titre {n}")),
                    details: Some(format!("Description {n}")),
                    destination: DestinationChoice::Set { id: "externe".into() },
                    ..ConversionDraft::default()
                };
                barrier.wait();
                convert_item_with(&conn, &id, item.updated_at, &d, T0 + 10 + n as i64).map(|c| c.task)
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
    // La tâche stockée est exactement celle du gagnant, avec SON brouillon complet.
    let stored = get_task(&conn, &winners[0].id).unwrap();
    assert_eq!(stored, *winners[0]);
    assert!(stored.title.starts_with("Titre ") && stored.details.starts_with("Description "));
    assert_eq!(stored.title.trim_start_matches("Titre "), stored.details.trim_start_matches("Description "));
    assert_eq!(stored.responsibility_id.as_deref(), Some("externe"));
    assert_eq!(inbox::get_item(&conn, &item.id).unwrap().content, "Course avec brouillons");
    assert_invariants(&conn);
}

#[test]
fn le_choix_de_destination_se_lit_au_format_de_l_interface() {
    let keep: DestinationChoice = serde_json::from_str(r#"{"type":"keep"}"#).unwrap();
    let clear: DestinationChoice = serde_json::from_str(r#"{"type":"clear"}"#).unwrap();
    let set: DestinationChoice = serde_json::from_str(r#"{"type":"set","id":"moi"}"#).unwrap();
    assert_eq!(keep, DestinationChoice::Keep);
    assert_eq!(clear, DestinationChoice::Clear);
    assert_eq!(set, DestinationChoice::Set { id: "moi".into() });
    assert_eq!(DestinationChoice::default(), DestinationChoice::Keep);
    // Un type inconnu ou un « set » sans identifiant ne devient jamais une destination.
    assert!(serde_json::from_str::<DestinationChoice>(r#"{"type":"other"}"#).is_err());
    assert!(serde_json::from_str::<DestinationChoice>(r#"{"type":"set"}"#).is_err());
}

#[test]
fn les_nouveaux_messages_d_erreur_ne_reprennent_pas_le_texte_saisi() {
    let conn = db();
    let item = capture(&conn, "Secret de la capture", None, 1);
    let secret = format!("MOT-DE-PASSE-{}", "x".repeat(inbox::MAX_CONTENT_CHARS));
    let error = convert_draft(&conn, &item, &draft(None, Some(&secret), DestinationChoice::Keep), 10)
        .unwrap_err();
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "details_too_long");
    let message = json["message"].as_str().unwrap();
    assert!(!message.contains("MOT-DE-PASSE") && !message.contains("Secret"));
    let unknown = serde_json::to_value(TaskError::UnknownDestination).unwrap();
    assert_eq!(unknown["code"], "unknown_destination");
}

// --- 003-B1b : modèle enrichi, création directe, modification --------------------------------

fn fields(title: &str) -> TaskFields {
    TaskFields { title: title.to_owned(), ..TaskFields::default() }
}

fn create(conn: &Connection, f: &TaskFields, at: i64) -> Result<Task, TaskError> {
    create_task(conn, f, T0 + at)
}

#[test]
fn creation_directe_sans_capture_ni_provenance_inventee() {
    let conn = db();
    let before_items = count(&conn, "SELECT count(*) FROM inbox_items");
    let task = create(&conn, &fields("Appeler le plombier"), 10).unwrap();

    assert_eq!(task.title, "Appeler le plombier");
    assert_eq!(task.details, ""); // description facultative
    assert_eq!(task.status, TaskStatus::Todo);
    assert_eq!(task.origin_inbox_item_id, None, "aucune capture inventée");
    assert_eq!((task.created_at, task.updated_at, task.deleted_at), (T0 + 10, T0 + 10, None));
    assert_eq!(
        (&task.responsibility_id, &task.section_id, &task.start_date, &task.due_date, &task.completed_on, task.priority),
        (&None, &None, &None, &None, &None, None),
        "aucune valeur par défaut inventée (ni « Moi », ni priorité normale)"
    );
    assert_eq!(uuid::Uuid::parse_str(&task.id).unwrap().get_version_num(), 7);
    assert_eq!(count(&conn, "SELECT count(*) FROM inbox_items"), before_items, "aucune capture créée");
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks WHERE origin_inbox_item_id IS NOT NULL"), 0);
    assert_invariants(&conn);
}

#[test]
fn une_tache_creee_directement_n_est_pas_annulable_comme_une_conversion() {
    let conn = db();
    let task = create(&conn, &fields("Directe"), 10).unwrap();
    // Pas de capture à remettre dans la boîte : le mécanisme d'annulation de conversion la refuse.
    assert_eq!(cancel_conversion(&conn, &task.id, T0 + 20).unwrap_err().code(), "no_origin");
}

#[test]
fn univers_et_rubrique_sont_independants_et_facultatifs() {
    let conn = db();
    let both = TaskFields {
        responsibility_id: Some("externe".into()),
        section_id: Some("administratif".into()),
        ..fields("Externe + Administratif")
    };
    let task = create(&conn, &both, 10).unwrap();
    assert_eq!(task.responsibility_id.as_deref(), Some("externe"));
    assert_eq!(task.section_id.as_deref(), Some("administratif"));

    let only_section = TaskFields { section_id: Some("finances".into()), ..fields("Rubrique seule") };
    let t2 = create(&conn, &only_section, 11).unwrap();
    assert_eq!((t2.responsibility_id, t2.section_id.as_deref()), (None, Some("finances")));
    let only_resp = TaskFields { responsibility_id: Some("moi".into()), ..fields("Univers seul") };
    let t3 = create(&conn, &only_resp, 12).unwrap();
    assert_eq!((t3.responsibility_id.as_deref(), t3.section_id), (Some("moi"), None));
    // Une chaîne vide équivaut à « aucun » (jamais un identifiant « » à résoudre).
    let blank = TaskFields { responsibility_id: Some(String::new()), ..fields("Vide") };
    assert_eq!(create(&conn, &blank, 13).unwrap().responsibility_id, None);
}

#[test]
fn la_nature_des_destinations_est_verifiee_cote_rust_et_cote_sql() {
    let conn = db();
    let before = snapshot(&conn);
    // Rubrique dans l'univers, univers dans la rubrique, destination inconnue ou archivée.
    for (f, code) in [
        (TaskFields { responsibility_id: Some("finances".into()), ..fields("T") }, "destination_kind"),
        (TaskFields { section_id: Some("moi".into()), ..fields("T") }, "destination_kind"),
        (TaskFields { section_id: Some("inconnue".into()), ..fields("T") }, "unknown_destination"),
    ] {
        assert_eq!(create(&conn, &f, 10).unwrap_err().code(), code);
    }
    conn.execute("UPDATE destinations SET archived_at = 1 WHERE id = 'externe'", []).unwrap();
    let archived = TaskFields { responsibility_id: Some("externe".into()), ..fields("T") };
    assert_eq!(create(&conn, &archived, 11).unwrap_err().code(), "unknown_destination");
    assert_eq!(before, snapshot(&conn), "un refus ne modifie rien");

    // Défense en profondeur : le schéma refuse aussi, même en contournant Rust.
    let direct = |col: &str, value: &str| {
        conn.execute(
            &format!(
                "INSERT INTO tasks (id, title, {col}, created_at, updated_at) VALUES ('x{col}', 'T', ?1, 1, 1)"
            ),
            [value],
        )
    };
    assert!(direct("responsibility_id", "finances").is_err());
    assert!(direct("section_id", "moi").is_err());
    assert!(direct("section_id", "inconnue").is_err());
}

#[test]
fn les_dates_sont_validees_format_calendrier_et_coherence() {
    let conn = db();
    let with = |start: Option<&str>, due: Option<&str>| TaskFields {
        start_date: start.map(str::to_owned),
        due_date: due.map(str::to_owned),
        ..fields("Dates")
    };
    // Valides, y compris le 29 février d'une année bissextile et début = échéance.
    assert!(create(&conn, &with(Some("2026-10-01"), Some("2026-10-14")), 1).is_ok());
    assert!(create(&conn, &with(None, Some("2028-02-29")), 2).is_ok());
    assert!(create(&conn, &with(Some("2026-10-14"), Some("2026-10-14")), 3).is_ok());
    assert!(create(&conn, &with(Some("2026-10-14"), None), 4).is_ok());

    let before = snapshot(&conn);
    for bad in [
        "2026-02-30", "2027-02-29", "2026-13-01", "2026-00-10", "2026-10-00", "2026-10-32", "2026-2-3",
        "26-10-14", "14/10/2026", "2026-10-14T10:00", "2026-10-14 ", " 2026-10-14", "", "aaaa-bb-cc",
        "２０２６-10-14", "0000-01-01", "2026-10-1４",
    ] {
        assert_eq!(create(&conn, &with(None, Some(bad)), 5).unwrap_err().code(), "invalid_date", "{bad:?}");
        assert_eq!(create(&conn, &with(Some(bad), None), 5).unwrap_err().code(), "invalid_date", "{bad:?}");
    }
    // Début postérieur à l'échéance : interdit (Rust) ; le schéma l'interdit aussi.
    assert_eq!(
        create(&conn, &with(Some("2026-10-15"), Some("2026-10-14")), 6).unwrap_err().code(),
        "dates_incoherent"
    );
    assert_eq!(before, snapshot(&conn));
    assert!(conn
        .execute(
            "INSERT INTO tasks (id, title, start_date, due_date, created_at, updated_at)
             VALUES ('inco', 'T', '2026-10-15', '2026-10-14', 1, 1)",
            [],
        )
        .is_err());
}

#[test]
fn rust_et_sqlite_s_accordent_sur_les_dates_valides() {
    // Tout ce que Rust accepte, le CHECK du schéma l'accepte ; et inversement pour les dates réelles.
    let conn = db();
    let sqlite_accepts = |value: &str| -> bool {
        conn.execute(
            "INSERT INTO tasks (id, title, due_date, created_at, updated_at) VALUES (hex(randomblob(8)), 'T', ?1, 1, 1)",
            [value],
        )
        .is_ok()
    };
    for value in [
        "2026-10-14", "2024-02-29", "2100-02-28", "2100-02-29", "2000-02-29", "1999-12-31", "9999-12-31",
        "2026-04-31", "2026-06-31", "2026-02-30", "2026-1-01", "abcd-ef-gh", "2026-10-14 00:00:00",
    ] {
        if is_valid_calendar_date(value) {
            assert!(sqlite_accepts(value), "Rust accepte {value:?} mais pas SQLite");
        } else if value.len() == 10 && !value.contains(char::is_alphabetic) {
            assert!(!sqlite_accepts(value), "SQLite accepte {value:?} que Rust refuse");
        }
    }
}

#[test]
fn la_priorite_est_absente_par_defaut_et_limitee_aux_niveaux_disponibles() {
    let conn = db();
    let none = create(&conn, &fields("Sans priorité"), 1).unwrap();
    assert_eq!(none.priority, None); // jamais « normale » par défaut
    for level in [1, 2, 3] {
        let f = TaskFields { priority: Some(level), ..fields("P") };
        assert_eq!(create(&conn, &f, 2).unwrap().priority, Some(level));
    }
    // Niveaux réservés (4, 5) et hors bornes : refusés côté Rust.
    for level in [0, 4, 5, 6, -1, 100] {
        let f = TaskFields { priority: Some(level), ..fields("P") };
        assert_eq!(create(&conn, &f, 3).unwrap_err().code(), "invalid_priority", "{level}");
    }
    // Le schéma garde 1 à 5 (évolution future sans reconstruction).
    for (level, ok) in [(4, true), (5, true), (6, false), (0, false)] {
        let r = conn.execute(
            "INSERT INTO tasks (id, title, priority, created_at, updated_at)
             VALUES (hex(randomblob(8)), 'T', ?1, 1, 1)",
            [level],
        );
        assert_eq!(r.is_ok(), ok, "{level}");
    }
}

#[test]
fn titre_et_description_sont_valides_a_la_creation() {
    let conn = db();
    for (title, code) in [("", "empty_title"), ("   \n ", "empty_title")] {
        assert_eq!(create(&conn, &fields(title), 1).unwrap_err().code(), code);
    }
    assert_eq!(create(&conn, &fields(&"a".repeat(121)), 1).unwrap_err().code(), "title_too_long");
    let long = TaskFields { details: "é".repeat(10_001), ..fields("T") };
    assert_eq!(create(&conn, &long, 1).unwrap_err().code(), "details_too_long");
    let exact = TaskFields { details: "é".repeat(10_000), ..fields("T") };
    assert_eq!(create(&conn, &exact, 2).unwrap().details.chars().count(), 10_000);
    // Espaces et retours normalisés dans le titre ; description nettoyée aux extrémités seulement.
    let messy = TaskFields { details: "  a\n\n  b  ".into(), ..fields("  Mon   titre\n ici ") };
    let task = create(&conn, &messy, 3).unwrap();
    assert_eq!((task.title.as_str(), task.details.as_str()), ("Mon titre ici", "a\n\n  b"));
}

#[test]
fn modifier_une_tache_applique_les_champs_et_fait_croitre_updated_at() {
    let conn = db();
    let task = create(&conn, &fields("Avant"), 10).unwrap();
    let f = TaskFields {
        title: "Après".into(),
        details: "Nouvelle description".into(),
        responsibility_id: Some("moi".into()),
        section_id: Some("finances".into()),
        start_date: Some("2026-10-01".into()),
        due_date: Some("2026-10-31".into()),
        priority: Some(3),
    };
    let updated = update_task(&conn, &task.id, task.updated_at, &f, T0 + 20).unwrap();
    assert_eq!(updated.title, "Après");
    assert_eq!(updated.details, "Nouvelle description");
    assert_eq!((updated.responsibility_id.as_deref(), updated.section_id.as_deref()), (Some("moi"), Some("finances")));
    assert_eq!((updated.start_date.as_deref(), updated.due_date.as_deref(), updated.priority), (Some("2026-10-01"), Some("2026-10-31"), Some(3)));
    assert_eq!(updated.updated_at, T0 + 20);
    // Inchangés : identité, provenance, statut, date de réalisation, création, suppression.
    assert_eq!(updated.id, task.id);
    assert_eq!(updated.status, TaskStatus::Todo);
    assert_eq!(updated.origin_inbox_item_id, None);
    assert_eq!((updated.completed_on, updated.created_at, updated.deleted_at), (None, task.created_at, None));

    // Horloge qui n'avance pas : `updated_at` croît quand même strictement.
    let again = TaskFields { title: "Encore".into(), ..f };
    let third = update_task(&conn, &task.id, updated.updated_at, &again, T0 + 5).unwrap();
    assert_eq!(third.updated_at, updated.updated_at + 1);
    assert_invariants(&conn);
}

#[test]
fn une_modification_sans_changement_n_ecrit_rien() {
    let conn = db();
    let f = TaskFields {
        details: "Texte".into(),
        responsibility_id: Some("moi".into()),
        priority: Some(2),
        ..fields("Stable")
    };
    let task = create(&conn, &f, 10).unwrap();
    let before = snapshot(&conn);
    let same = update_task(&conn, &task.id, task.updated_at, &f, T0 + 99).unwrap();
    assert_eq!(same, task);
    assert_eq!(before, snapshot(&conn), "aucune écriture, updated_at inchangé");
    // Même après normalisation (espaces autour) : toujours aucun changement.
    let spaced = TaskFields { title: "  Stable  ".into(), details: "  Texte  ".into(), ..f.clone() };
    assert_eq!(update_task(&conn, &task.id, task.updated_at, &spaced, T0 + 100).unwrap(), task);
    assert_eq!(before, snapshot(&conn));
}

#[test]
fn le_verrou_optimiste_refuse_une_version_perimee_sans_rien_modifier() {
    let conn = db();
    let task = create(&conn, &fields("Version 1"), 10).unwrap();
    let theirs = update_task(&conn, &task.id, task.updated_at, &fields("Version 2 ailleurs"), T0 + 20).unwrap();
    let before = snapshot(&conn);

    // L'utilisateur avait ouvert la version 1 : conflit, jamais d'écrasement silencieux.
    let error = update_task(&conn, &task.id, task.updated_at, &fields("Ma version"), T0 + 30).unwrap_err();
    assert_eq!(error.code(), "task_version_conflict");
    assert_eq!(before, snapshot(&conn));
    assert_eq!(get_task(&conn, &task.id).unwrap().title, "Version 2 ailleurs");
    // Avec la version actuelle, l'enregistrement réussit.
    assert!(update_task(&conn, &task.id, theirs.updated_at, &fields("Ma version"), T0 + 30).is_ok());
}

#[test]
fn une_tache_annulee_ou_inconnue_ne_se_modifie_pas() {
    let conn = db();
    let item = capture(&conn, "À annuler", None, 1);
    let done = convert(&conn, &item, 10).unwrap();
    let cancelled = cancel_conversion(&conn, &done.task.id, T0 + 20).unwrap().task;
    let before = snapshot(&conn);
    assert_eq!(
        update_task(&conn, &cancelled.id, cancelled.updated_at, &fields("Nouveau"), T0 + 30)
            .unwrap_err()
            .code(),
        "task_not_active"
    );
    assert_eq!(
        update_task(&conn, "inconnue", 1, &fields("X"), T0 + 30).unwrap_err().code(),
        "task_not_found"
    );
    assert_eq!(before, snapshot(&conn));
}

#[test]
fn modifier_une_tache_convertie_garde_sa_provenance_et_bloque_l_annulation() {
    let conn = db();
    let item = capture(&conn, "Capture d'origine", Some("finances"), 1);
    let task = convert(&conn, &item, 10).unwrap().task;
    let edited = update_task(&conn, &task.id, task.updated_at, &fields("Retouchée"), T0 + 20).unwrap();
    assert_eq!(edited.origin_inbox_item_id.as_deref(), Some(item.id.as_str()));
    // Le travail déjà fait n'est pas masqué : la garde de la brique 002 refuse l'annulation.
    assert_eq!(cancel_conversion(&conn, &task.id, T0 + 30).unwrap_err().code(), "task_modified");
    // La capture est intacte et toujours « convertie ».
    let stored = inbox::get_item(&conn, &item.id).unwrap();
    assert_eq!((stored.content.as_str(), stored.updated_at), ("Capture d'origine", item.updated_at));
    assert_eq!(stored.converted_task_id.as_deref(), Some(task.id.as_str()));
    assert_invariants(&conn);
}

#[test]
fn modifier_ne_change_jamais_le_statut_ni_la_date_de_realisation() {
    let conn = db();
    let task = create(&conn, &fields("Avancée"), 10).unwrap();
    conn.execute(
        "UPDATE tasks SET status = 'in_progress', completed_on = '2026-10-01', updated_at = updated_at + 1
         WHERE id = ?1",
        [&task.id],
    )
    .unwrap();
    let current = get_task(&conn, &task.id).unwrap();
    let edited = update_task(&conn, &task.id, current.updated_at, &fields("Avancée (titre)"), T0 + 50).unwrap();
    assert_eq!(edited.status, TaskStatus::InProgress);
    assert_eq!(edited.completed_on.as_deref(), Some("2026-10-01"));
}

#[test]
fn une_destination_deja_portee_reste_valide_apres_archivage_mais_pas_une_nouvelle() {
    let conn = db();
    let f = TaskFields {
        responsibility_id: Some("externe".into()),
        section_id: Some("inventaire".into()),
        ..fields("Classée")
    };
    let task = create(&conn, &f, 10).unwrap();
    conn.execute("UPDATE destinations SET archived_at = 5 WHERE id IN ('externe', 'inventaire', 'finances')", [])
        .unwrap();
    // Titre modifié, classement inchangé (archivé) : accepté.
    let keep = TaskFields { title: "Renommée".into(), ..f.clone() };
    let updated = update_task(&conn, &task.id, task.updated_at, &keep, T0 + 20).unwrap();
    assert_eq!(updated.responsibility_id.as_deref(), Some("externe"));
    assert_eq!(updated.section_id.as_deref(), Some("inventaire"));
    // Passer à une autre destination archivée : refusé.
    let other = TaskFields { section_id: Some("finances".into()), ..keep };
    assert_eq!(
        update_task(&conn, &task.id, updated.updated_at, &other, T0 + 30).unwrap_err().code(),
        "unknown_destination"
    );
}

#[test]
fn la_conversion_accepte_dates_priorite_et_classement_en_deux_dimensions() {
    let conn = db();
    let item = capture(&conn, "Dossier à compléter", Some("administratif"), 1);
    let d = ConversionDraft {
        title: Some("Compléter le dossier".into()),
        details: Some("Pièces à fournir".into()),
        destination: DestinationChoice::Split {
            responsibility_id: Some("externe".into()),
            section_id: Some("administratif".into()),
        },
        start_date: Some("2026-10-14".into()),
        due_date: Some("2026-11-01".into()),
        priority: Some(3),
    };
    let done = convert_item_with(&conn, &item.id, item.updated_at, &d, T0 + 10).unwrap();
    let t = &done.task;
    assert_eq!((t.responsibility_id.as_deref(), t.section_id.as_deref()), (Some("externe"), Some("administratif")));
    assert_eq!((t.start_date.as_deref(), t.due_date.as_deref(), t.priority), (Some("2026-10-14"), Some("2026-11-01"), Some(3)));
    assert_eq!(t.status, TaskStatus::Todo);
    // La capture garde sa destination unique, son texte et sa version.
    assert_eq!(done.item.destination_id.as_deref(), Some("administratif"));
    assert_eq!((done.item.content.as_str(), done.item.updated_at), ("Dossier à compléter", item.updated_at));
    // Créée avec dates et priorité : toujours annulable (updated_at = created_at).
    assert_eq!(cancel_conversion(&conn, &t.id, T0 + 20).unwrap().item, item);
    assert_invariants(&conn);
}

#[test]
fn la_conversion_refuse_dates_priorite_et_classement_invalides_sans_rien_ecrire() {
    let conn = db();
    let item = capture(&conn, "Capture", Some("moi"), 1);
    let before = snapshot(&conn);
    let cases = [
        (ConversionDraft { due_date: Some("2026-02-30".into()), ..ConversionDraft::default() }, "invalid_date"),
        (
            ConversionDraft {
                start_date: Some("2026-10-15".into()),
                due_date: Some("2026-10-14".into()),
                ..ConversionDraft::default()
            },
            "dates_incoherent",
        ),
        (ConversionDraft { priority: Some(4), ..ConversionDraft::default() }, "invalid_priority"),
        (
            ConversionDraft {
                destination: DestinationChoice::Split { responsibility_id: Some("finances".into()), section_id: None },
                ..ConversionDraft::default()
            },
            "destination_kind",
        ),
        (
            ConversionDraft {
                destination: DestinationChoice::Split { responsibility_id: None, section_id: Some("moi".into()) },
                ..ConversionDraft::default()
            },
            "destination_kind",
        ),
    ];
    for (d, code) in cases {
        assert_eq!(convert_draft(&conn, &item, &d, 10).unwrap_err().code(), code);
        assert_eq!(before, snapshot(&conn));
    }
}

#[test]
fn le_choix_de_classement_split_se_lit_au_format_de_l_interface() {
    let split: DestinationChoice =
        serde_json::from_str(r#"{"type":"split","responsibilityId":"moi","sectionId":null}"#).unwrap();
    assert_eq!(
        split,
        DestinationChoice::Split { responsibility_id: Some("moi".into()), section_id: None }
    );
    let partial: DestinationChoice = serde_json::from_str(r#"{"type":"split","sectionId":"finances"}"#).unwrap();
    assert_eq!(partial, DestinationChoice::Split { responsibility_id: None, section_id: Some("finances".into()) });
    // Les trois formes de 003-A restent lisibles telles quelles.
    for json in [r#"{"type":"keep"}"#, r#"{"type":"clear"}"#, r#"{"type":"set","id":"moi"}"#] {
        assert!(serde_json::from_str::<DestinationChoice>(json).is_ok(), "{json}");
    }
}

#[test]
fn le_format_json_de_la_tache_enrichie_correspond_a_l_interface() {
    let conn = db();
    let f = TaskFields {
        details: "d".into(),
        responsibility_id: Some("moi".into()),
        section_id: Some("finances".into()),
        start_date: Some("2026-10-01".into()),
        due_date: Some("2026-10-31".into()),
        priority: Some(2),
        ..fields("JSON")
    };
    let task = create(&conn, &f, 10).unwrap();
    let json = serde_json::to_value(&task).unwrap();
    assert_eq!(json["responsibilityId"], "moi");
    assert_eq!(json["sectionId"], "finances");
    assert_eq!(json["startDate"], "2026-10-01");
    assert_eq!(json["dueDate"], "2026-10-31");
    assert_eq!(json["completedOn"], serde_json::Value::Null);
    assert_eq!(json["priority"], 2);
    assert_eq!(json["originInboxItemId"], serde_json::Value::Null);
    assert!(json.get("destinationId").is_none(), "l'ancien champ n'existe plus");
    // Entrée : camelCase, champs facultatifs absents acceptés.
    let parsed: TaskFields = serde_json::from_str(r#"{"title":"T","dueDate":"2026-10-31"}"#).unwrap();
    assert_eq!((parsed.details.as_str(), parsed.due_date.as_deref()), ("", Some("2026-10-31")));
}

#[test]
fn les_nouveaux_messages_d_erreur_ne_contiennent_aucune_saisie() {
    let conn = db();
    let secret = "SECRET-PERSONNEL-9f3";
    let f = TaskFields {
        title: secret.into(),
        section_id: Some(secret.into()),
        due_date: Some(format!("{secret}-date")),
        ..TaskFields::default()
    };
    let json = |e: TaskError| serde_json::to_string(&e).unwrap();
    for error in [
        create(&conn, &f, 1).unwrap_err(),
        create(&conn, &TaskFields { due_date: None, ..f.clone() }, 1).unwrap_err(),
        update_task(&conn, secret, 1, &f, 1).unwrap_err(),
    ] {
        assert!(!json(error).contains(secret));
    }
    let f2 = TaskFields { title: "T".into(), priority: Some(99), ..TaskFields::default() };
    assert!(!json(create(&conn, &f2, 1).unwrap_err()).contains("99"));
}

#[test]
fn modifications_simultanees_une_seule_gagne_et_aucune_n_est_perdue_en_silence() {
    let (_dir, path) = file_db();
    let task = {
        let conn = storage::open(&path).unwrap();
        create(&conn, &fields("Départ"), 1).unwrap()
    };
    const THREADS: usize = 6;
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles: Vec<_> = (0..THREADS)
        .map(|n| {
            let (path, id, barrier) = (path.clone(), task.id.clone(), barrier.clone());
            std::thread::spawn(move || {
                let conn = storage::open(&path).unwrap();
                let f = TaskFields { title: format!("Titre {n}"), ..TaskFields::default() };
                barrier.wait();
                update_task(&conn, &id, task.updated_at, &f, T0 + 10 + n as i64).map(|t| t.title)
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let winners: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    assert_eq!(winners.len(), 1, "exactement une modification : {results:?}");
    for result in &results {
        if let Err(error) = result {
            assert_eq!(error.code(), "task_version_conflict", "{error:?}");
        }
    }
    let conn = storage::open(&path).unwrap();
    assert_eq!(&get_task(&conn, &task.id).unwrap().title, winners[0]);
    assert_invariants(&conn);
}

#[test]
fn les_sequences_aleatoires_avec_creation_et_edition_conservent_les_invariants() {
    let conn = db();
    let items: Vec<InboxItem> = (0..3).map(|i| capture(&conn, &format!("Capture {i}"), Some("moi"), i)).collect();
    let mut direct: Vec<Task> = Vec::new();
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut clock = 100;
    for step in 0..300 {
        clock += 1;
        match next() % 5 {
            0 => {
                if let Ok(t) = create(&conn, &fields(&format!("Directe {step}")), clock) {
                    direct.push(t);
                }
            }
            1 if !direct.is_empty() => {
                let i = (next() % direct.len() as u64) as usize;
                let f = TaskFields {
                    title: format!("Édité {step}"),
                    due_date: Some(format!("2026-{:02}-{:02}", 1 + step % 12, 1 + step % 28)),
                    priority: Some(1 + (step % 3) as i64),
                    ..TaskFields::default()
                };
                if let Ok(t) = update_task(&conn, &direct[i].id, direct[i].updated_at, &f, T0 + clock) {
                    direct[i] = t;
                }
            }
            2 => {
                let it = &items[(next() % 3) as usize];
                let cur = inbox::get_item(&conn, &it.id).unwrap();
                let _ = convert_item(&conn, &it.id, cur.updated_at, None, T0 + clock);
            }
            3 => {
                let it = &items[(next() % 3) as usize];
                if let Some(id) = inbox::get_item(&conn, &it.id).unwrap().converted_task_id {
                    let _ = cancel_conversion(&conn, &id, T0 + clock);
                }
            }
            _ => {
                let it = &items[(next() % 3) as usize];
                let cur = inbox::get_item(&conn, &it.id).unwrap();
                if let Some(id) = cur.converted_task_id {
                    let t = get_task(&conn, &id).unwrap();
                    let f = TaskFields { title: format!("Retouche {step}"), ..TaskFields::default() };
                    let _ = update_task(&conn, &id, t.updated_at, &f, T0 + clock);
                }
            }
        }
        assert_invariants(&conn);
    }
    // Les captures ne sont jamais altérées par les éditions de tâches.
    for it in &items {
        assert_eq!(inbox::get_item(&conn, &it.id).unwrap().content, it.content);
    }
    assert_eq!(
        count(&conn, "SELECT count(*) FROM tasks WHERE origin_inbox_item_id IS NULL"),
        direct.len() as i64
    );
}
