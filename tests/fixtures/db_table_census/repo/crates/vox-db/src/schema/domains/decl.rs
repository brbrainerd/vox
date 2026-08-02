// Fixture: schema DECLARATION only (mirrors real crates/vox-db/src/schema/domains/*.rs).
// Files under schema/domains/ are declarations, not usage — the census tool must
// exclude this file from the usage signal for every table it declares.
CREATE TABLE IF NOT EXISTS fixture_live_table (
    _id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fixture_dormant_table (
    _id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fixture_dead_table (
    _id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fixture_excluded_table (
    _id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);
