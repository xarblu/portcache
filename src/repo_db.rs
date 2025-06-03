use crate::config;

pub struct RepoDB {
    /// sqlite databse connection
    db: rusqlite::Connection,
}

impl RepoDB {
    pub fn new(config: &config::Config) -> Result<Self, String> {
        let db = match rusqlite::Connection::open(config.storage.location.join("db.sqlite3")) {
            Ok(db) => db,
            Err(e) => return Err(e.to_string()),
        };

        match db.pragma_update(None, "foreign_keys", 1) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        };

        match db.execute(
            "CREATE TABLE IF NOT EXISTS manifest (
                file    TEXT PRIMARY KEY NOT NULL,
                blake2b TEXT NOT NULL,
                sha512  TEXT NOT NULL
            )",
            (),
        ) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        };

        match db.execute(
            "CREATE TABLE IF NOT EXISTS src_uri (
                uri     TEXT PRIMARY KEY NOT NULL,
                file    TEXT REFERENCES manifest(file) ON UPDATE CASCADE ON DELETE CASCADE
            )",
            (),
        ) {
            Ok(_) => (),
            Err(e) => return Err(e.to_string()),
        };

        Ok(Self { db })
    }
}
