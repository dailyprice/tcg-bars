CREATE TABLE bars (
    id                TEXT PRIMARY KEY NOT NULL,
    name              TEXT NOT NULL,
    address           TEXT,
    city              TEXT,
    country           TEXT,
    lat               REAL NOT NULL,
    lng               REAL NOT NULL,
    -- false until someone has placed the pin on the actual venue
    location_verified INTEGER NOT NULL DEFAULT 0,
    -- free text, e.g. "10% off pints on Tuesday game night"
    player_offers     TEXT,
    website           TEXT,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE ratings (
    id          TEXT PRIMARY KEY NOT NULL,
    bar_id      TEXT NOT NULL REFERENCES bars(id) ON DELETE CASCADE,
    lighting    INTEGER NOT NULL CHECK (lighting BETWEEN 1 AND 5),
    table_space INTEGER NOT NULL CHECK (table_space BETWEEN 1 AND 5),
    offers      INTEGER NOT NULL CHECK (offers BETWEEN 1 AND 5),
    comment     TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX ratings_bar_id ON ratings(bar_id);
