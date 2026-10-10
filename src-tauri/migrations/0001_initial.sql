-- Migration 1 — brique 001-A : boîte « À organiser » et destinations.
-- Ne jamais modifier une migration déjà livrée : ajouter une nouvelle migration.

-- Destinations : où une capture sera consultée et traitée (ce ne sont PAS des tags).
-- `id` est stable et ne change jamais ; `label` peut être renommé ; une destination
-- n'est jamais supprimée, seulement archivée (`archived_at`).
CREATE TABLE destinations (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('responsibility', 'section')),
    position    INTEGER NOT NULL,
    archived_at INTEGER
) STRICT;

INSERT INTO destinations (id, label, kind, position) VALUES
    ('moi',           'Moi',           'responsibility', 10),
    ('externe',       'Externe',       'responsibility', 20),
    ('administratif', 'Administratif', 'section',        30),
    ('finances',      'Finances',      'section',        40),
    ('inventaire',    'Inventaire',    'section',        50);

-- Captures de la boîte « À organiser ». Horodatages en millisecondes UTC.
-- `deleted_at` est réservé à la corbeille logique (sous-brique 001-B).
CREATE TABLE inbox_items (
    id             TEXT PRIMARY KEY,
    content        TEXT NOT NULL CHECK (length(trim(content)) > 0 AND length(content) <= 10000),
    destination_id TEXT REFERENCES destinations(id),
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER
) STRICT;

CREATE INDEX inbox_items_recent ON inbox_items (deleted_at, created_at DESC);
CREATE INDEX inbox_items_destination ON inbox_items (destination_id, created_at DESC);
