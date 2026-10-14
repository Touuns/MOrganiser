-- Migration 3 — brique 003-B1b : modèle de tâche enrichi.
-- Ne jamais modifier une migration déjà livrée : ajouter une nouvelle migration.
-- Appliquée dans UNE transaction avec `PRAGMA user_version` : tout ou rien.
--
-- Ce que fait cette migration :
--   * description facultative (`details` peut être vide) ;
--   * l'ancien `destination_id` unique est réparti selon la NATURE de la destination :
--     `responsibility` -> `responsibility_id` (univers), `section` -> `section_id` (rubrique) ;
--     une tâche sans destination garde les deux à NULL (jamais « Moi » par défaut) ;
--   * dates calendaires facultatives : `start_date`, `due_date`, `completed_on` ;
--   * priorité facultative (1 à 5 admis par le schéma, 1 à 3 utilisés) ;
--   * la table `tasks` est reconstruite (SQLite ne modifie pas un CHECK).
--
-- Stratégie PROUVÉE par `storage/rebuild_proof_tests.rs` (SQLite 3.53.2, clés étrangères actives) :
-- créer `tasks_new`, copier et VÉRIFIER, détacher les liens `inbox_items.converted_task_id`,
-- supprimer `tasks`, renommer `tasks_new`, recréer les objets, restaurer les liens, contrôler.
-- NE PAS renommer l'ancienne table (`tasks_old`) : SQLite réécrirait les clés étrangères de
-- `inbox_items` vers l'ancien nom. NE PAS désactiver les clés étrangères.
-- Les gardes sont des `CHECK` nommés sur tables temporaires : une anomalie annule TOUT.
--
-- Les lignes `-- @stage: nom` sont lues par les tests (injection de pannes étape par étape).

-- @stage: guard
CREATE TEMP TABLE _guard_no_orphan_destination (
    ok INTEGER NOT NULL CONSTRAINT orphan_destination CHECK (ok = 1));
INSERT INTO _guard_no_orphan_destination
SELECT NOT EXISTS (
    SELECT 1 FROM tasks t LEFT JOIN destinations d ON d.id = t.destination_id
    WHERE t.destination_id IS NOT NULL AND d.id IS NULL);

CREATE TEMP TABLE _guard_fk_clean (
    ok INTEGER NOT NULL CONSTRAINT fk_violations_before CHECK (ok = 1));
INSERT INTO _guard_fk_clean SELECT (SELECT count(*) FROM pragma_foreign_key_check) = 0;

-- @stage: save_links
CREATE TEMP TABLE _links AS
SELECT id AS item_id, converted_task_id AS task_id
FROM inbox_items WHERE converted_task_id IS NOT NULL;

-- @stage: create_new
-- `IS` et non `=` pour les dates : date(x) vaut NULL pour un format invalide, et un CHECK laisse
-- passer NULL (prouvé par les tests). `date(x) IS x` impose le format canonique AAAA-MM-JJ ET un
-- vrai jour du calendrier.
CREATE TABLE tasks_new (
    id                   TEXT PRIMARY KEY,
    title                TEXT NOT NULL CHECK (length(trim(title)) > 0 AND length(title) <= 120),
    details              TEXT NOT NULL DEFAULT '' CHECK (length(details) <= 10000),
    status               TEXT NOT NULL DEFAULT 'todo'
                         CHECK (status IN ('todo', 'in_progress', 'waiting', 'blocked', 'done')),
    responsibility_id    TEXT REFERENCES destinations(id),
    section_id           TEXT REFERENCES destinations(id),
    start_date           TEXT CHECK (start_date IS NULL OR date(start_date) IS start_date),
    due_date             TEXT CHECK (due_date IS NULL OR date(due_date) IS due_date),
    completed_on         TEXT CHECK (completed_on IS NULL OR date(completed_on) IS completed_on),
    priority             INTEGER CHECK (priority IS NULL OR priority BETWEEN 1 AND 5),
    origin_inbox_item_id TEXT REFERENCES inbox_items(id),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    deleted_at           INTEGER,
    CHECK (start_date IS NULL OR due_date IS NULL OR start_date <= due_date)
) STRICT;

-- @stage: copy
INSERT INTO tasks_new (id, title, details, status, responsibility_id, section_id,
                       origin_inbox_item_id, created_at, updated_at, deleted_at)
SELECT t.id, t.title, t.details, t.status,
       CASE WHEN d.kind = 'responsibility' THEN t.destination_id END,
       CASE WHEN d.kind = 'section'        THEN t.destination_id END,
       t.origin_inbox_item_id, t.created_at, t.updated_at, t.deleted_at
FROM tasks t LEFT JOIN destinations d ON d.id = t.destination_id;

-- @stage: verify_copy
CREATE TEMP TABLE _guard_copy_count (
    ok INTEGER NOT NULL CONSTRAINT copy_count CHECK (ok = 1));
INSERT INTO _guard_copy_count
SELECT (SELECT count(*) FROM tasks) = (SELECT count(*) FROM tasks_new);

CREATE TEMP TABLE _guard_copy_fields (
    ok INTEGER NOT NULL CONSTRAINT copy_fields CHECK (ok = 1));
INSERT INTO _guard_copy_fields
SELECT NOT EXISTS (
    SELECT id, title, details, status, origin_inbox_item_id, created_at, updated_at, deleted_at FROM tasks
    EXCEPT
    SELECT id, title, details, status, origin_inbox_item_id, created_at, updated_at, deleted_at FROM tasks_new)
AND NOT EXISTS (
    SELECT id, title, details, status, origin_inbox_item_id, created_at, updated_at, deleted_at FROM tasks_new
    EXCEPT
    SELECT id, title, details, status, origin_inbox_item_id, created_at, updated_at, deleted_at FROM tasks);

CREATE TEMP TABLE _guard_copy_destinations (
    ok INTEGER NOT NULL CONSTRAINT copy_destinations CHECK (ok = 1));
INSERT INTO _guard_copy_destinations
SELECT NOT EXISTS (
    SELECT 1 FROM tasks t JOIN tasks_new n ON n.id = t.id
    WHERE t.destination_id IS NOT COALESCE(n.responsibility_id, n.section_id)
       OR (n.responsibility_id IS NOT NULL AND n.section_id IS NOT NULL));

-- @stage: detach
-- Provisoire (restauré plus bas dans la MÊME transaction) : `tasks` n'a plus d'enfant qui la
-- référence, ce qui permet de la supprimer clés étrangères actives.
UPDATE inbox_items SET converted_task_id = NULL WHERE converted_task_id IS NOT NULL;

-- @stage: swap
DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

-- @stage: objects
-- Au plus UNE tâche ACTIVE par capture ; une tâche annulée (`deleted_at`) libère la place.
CREATE UNIQUE INDEX tasks_one_active_per_origin
    ON tasks (origin_inbox_item_id)
    WHERE origin_inbox_item_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX tasks_recent ON tasks (deleted_at, created_at DESC);

-- Garde de `updated_at` : toute modification EFFECTIVE d'un champ métier (nouveaux champs
-- compris) doit faire croître strictement `updated_at`. Sans cela, la garde d'annulation
-- (`updated_at = created_at`) ne verrait pas un travail déjà fait. `deleted_at` n'est pas un
-- champ métier ; une mise à jour sans changement de valeur reste acceptée.
CREATE TRIGGER tasks_updated_at_must_grow
BEFORE UPDATE OF title, details, status, responsibility_id, section_id,
                 start_date, due_date, completed_on, priority ON tasks
WHEN NEW.updated_at <= OLD.updated_at
 AND (NEW.title IS NOT OLD.title
   OR NEW.details IS NOT OLD.details
   OR NEW.status IS NOT OLD.status
   OR NEW.responsibility_id IS NOT OLD.responsibility_id
   OR NEW.section_id IS NOT OLD.section_id
   OR NEW.start_date IS NOT OLD.start_date
   OR NEW.due_date IS NOT OLD.due_date
   OR NEW.completed_on IS NOT OLD.completed_on
   OR NEW.priority IS NOT OLD.priority)
BEGIN
    SELECT RAISE(ABORT, 'tasks.updated_at doit croitre strictement quand un champ metier change');
END;

-- La provenance est la base des invariants de conversion : elle ne se réécrit jamais.
CREATE TRIGGER tasks_origin_is_immutable
BEFORE UPDATE OF id, origin_inbox_item_id, created_at ON tasks
WHEN NEW.id IS NOT OLD.id
  OR NEW.origin_inbox_item_id IS NOT OLD.origin_inbox_item_id
  OR NEW.created_at IS NOT OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'id, origine et date de creation d''une tache sont immuables');
END;

-- Nature des destinations : un univers n'est pas une rubrique, et inversement (un CHECK ne peut
-- pas interroger une autre table). `IS NOT` : une destination introuvable est aussi refusée.
CREATE TRIGGER tasks_responsibility_kind_insert
BEFORE INSERT ON tasks
WHEN NEW.responsibility_id IS NOT NULL
 AND (SELECT kind FROM destinations WHERE id = NEW.responsibility_id) IS NOT 'responsibility'
BEGIN
    SELECT RAISE(ABORT, 'responsibility_id doit designer une destination de nature responsibility');
END;

CREATE TRIGGER tasks_responsibility_kind_update
BEFORE UPDATE OF responsibility_id ON tasks
WHEN NEW.responsibility_id IS NOT NULL
 AND (SELECT kind FROM destinations WHERE id = NEW.responsibility_id) IS NOT 'responsibility'
BEGIN
    SELECT RAISE(ABORT, 'responsibility_id doit designer une destination de nature responsibility');
END;

CREATE TRIGGER tasks_section_kind_insert
BEFORE INSERT ON tasks
WHEN NEW.section_id IS NOT NULL
 AND (SELECT kind FROM destinations WHERE id = NEW.section_id) IS NOT 'section'
BEGIN
    SELECT RAISE(ABORT, 'section_id doit designer une destination de nature section');
END;

CREATE TRIGGER tasks_section_kind_update
BEFORE UPDATE OF section_id ON tasks
WHEN NEW.section_id IS NOT NULL
 AND (SELECT kind FROM destinations WHERE id = NEW.section_id) IS NOT 'section'
BEGIN
    SELECT RAISE(ABORT, 'section_id doit designer une destination de nature section');
END;

-- @stage: restore
UPDATE inbox_items
SET converted_task_id = (SELECT task_id FROM _links WHERE item_id = inbox_items.id)
WHERE id IN (SELECT item_id FROM _links);

-- @stage: final
CREATE TEMP TABLE _guard_links_restored (
    ok INTEGER NOT NULL CONSTRAINT links_restored CHECK (ok = 1));
INSERT INTO _guard_links_restored
SELECT (SELECT count(*) FROM inbox_items WHERE converted_task_id IS NOT NULL)
     = (SELECT count(*) FROM _links)
   AND NOT EXISTS (SELECT 1 FROM _links l JOIN inbox_items i ON i.id = l.item_id
                   WHERE i.converted_task_id IS NOT l.task_id);

CREATE TEMP TABLE _guard_fk_after (
    ok INTEGER NOT NULL CONSTRAINT fk_violations_after CHECK (ok = 1));
INSERT INTO _guard_fk_after SELECT (SELECT count(*) FROM pragma_foreign_key_check) = 0;

DROP TABLE _guard_no_orphan_destination;
DROP TABLE _guard_fk_clean;
DROP TABLE _guard_copy_count;
DROP TABLE _guard_copy_fields;
DROP TABLE _guard_copy_destinations;
DROP TABLE _guard_links_restored;
DROP TABLE _guard_fk_after;
DROP TABLE _links;
