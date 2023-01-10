use indoc::indoc;
use rusqlite::Connection;

pub fn apply_schema(con: &Connection) {
    let query = indoc!{"
        CREATE TABLE IF NOT EXISTS layers (
            layer INTEGER PRIMARY KEY,
            depth_mined INTEGER DEFAULT 0,
            finished INTEGER NOT NULL DEFAULT 0 CHECK(finished = 0 OR finished = 1),
            FOREIGN KEY(layer) REFERENCES partitions(layer)
        );
        CREATE TABLE IF NOT EXISTS assignments (
            username TEXT PRIMARY KEY,
            layer INTEGER NOT NULL UNIQUE,
            last_update INTEGER NOT NULL,
            FOREIGN KEY(layer) REFERENCES layers(layer),
            FOREIGN KEY(layer) REFERENCES partitions(layer)
        );
        CREATE INDEX IF NOT EXISTS assignments_by_layer ON assignments(layer);

        CREATE TABLE IF NOT EXISTS min_layer (
            id INTEGER PRIMARY KEY CHECK (id = 0), -- only 1 row
            even INTEGER NOT NULL CHECK (even % 2 = 0),
            odd INTEGER NOT NULL CHECK (odd % 2 = 1)
        );
        INSERT OR IGNORE INTO min_layer(id,even,odd) VALUES(0, 0, 1);

        CREATE TABLE IF NOT EXISTS leaderboard (
            username TEXT PRIMARY KEY,
            blocks_mined INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS leaderboard_by_blocks ON leaderboard(blocks_mined DESC);
        CREATE TABLE IF NOT EXISTS leaderboard_aliases (
            account TEXT PRIMARY KEY,
            name TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS partitions (
            layer INTEGER PRIMARY KEY,
            serialized TEXT NOT NULL
        );
    "};
    con.execute_batch(query).unwrap();
}