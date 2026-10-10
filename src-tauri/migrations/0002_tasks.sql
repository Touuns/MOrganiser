-- Migration 2 — brique 002-A : tâche minimale et conversion d'une capture en tâche.
-- Ne jamais modifier une migration déjà livrée : ajouter une nouvelle migration.
-- Appliquée dans UNE transaction avec `PRAGMA user_version` : tout ou rien.

-- Tâche minimale. Le titre est un extrait modifiable ; `details` reçoit la copie intégrale
-- du texte de la capture, qui reste de toute façon intacte dans `inbox_items`.
-- Les cinq statuts décidés au cahier des charges sont admis dès maintenant ; 002 n'écrit que
-- 'todo'. `deleted_at` : annulation ou corbeille logique, jamais de suppression physique.
CREATE TABLE tasks (
    id                   TEXT PRIMARY KEY,
    title                TEXT NOT NULL CHECK (length(trim(title)) > 0 AND length(title) <= 120),
    details              TEXT NOT NULL CHECK (length(trim(details)) > 0 AND length(details) <= 10000),
    status               TEXT NOT NULL DEFAULT 'todo'
                         CHECK (status IN ('todo', 'in_progress', 'waiting', 'blocked', 'done')),
    destination_id       TEXT REFERENCES destinations(id),
    origin_inbox_item_id TEXT REFERENCES inbox_items(id),
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    deleted_at           INTEGER
) STRICT;

-- Au plus UNE tâche ACTIVE par capture. Une tâche annulée (`deleted_at` renseigné) libère la
-- place : la capture peut être reconvertie, et l'ancienne tâche reste conservée.
CREATE UNIQUE INDEX tasks_one_active_per_origin
    ON tasks (origin_inbox_item_id)
    WHERE origin_inbox_item_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX tasks_recent ON tasks (deleted_at, created_at DESC);

-- Cycle de vie d'une capture : `converted_at` est le changement d'état (la capture quitte
-- « À organiser » et rejoint « Traitées »). `updated_at`, lui, ne désigne toujours que la
-- dernière modification du contenu : la conversion ne le touche pas.
ALTER TABLE inbox_items ADD COLUMN converted_at INTEGER;
ALTER TABLE inbox_items ADD COLUMN converted_task_id TEXT REFERENCES tasks(id);

-- Une tâche ne peut être la conversion active que d'une seule capture.
CREATE UNIQUE INDEX inbox_items_converted_task
    ON inbox_items (converted_task_id)
    WHERE converted_task_id IS NOT NULL;

-- Vue « Traitées » (002-C) : captures converties, de la plus récente à la plus ancienne.
CREATE INDEX inbox_items_converted
    ON inbox_items (converted_at DESC)
    WHERE converted_at IS NOT NULL;

-- Garde de `updated_at` : toute modification EFFECTIVE d'un champ métier doit faire croître
-- strictement `updated_at`. Sans cela, une évolution future (brique 003 et suivantes) pourrait
-- modifier une tâche sans que la garde d'annulation (`updated_at = created_at`) le voie, et une
-- annulation masquerait un travail déjà fait. Une mise à jour qui ne change aucune valeur
-- (même avec le même `updated_at`) reste acceptée ; `deleted_at` n'est pas un champ métier.
CREATE TRIGGER tasks_updated_at_must_grow
BEFORE UPDATE OF title, details, status, destination_id ON tasks
WHEN NEW.updated_at <= OLD.updated_at
 AND (NEW.title IS NOT OLD.title
   OR NEW.details IS NOT OLD.details
   OR NEW.status IS NOT OLD.status
   OR NEW.destination_id IS NOT OLD.destination_id)
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
