use futures::lock::Mutex;

use crate::config;
use crate::manifest_walker::ManifestEntry;

pub struct RepoDB {
    /// sqlite databse connection
    db: Mutex<rusqlite::Connection>,
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
                origin  TEXT NOT NULL,
                size    INTEGER NOT NULL,
                blake2b TEXT,
                sha512  TEXT
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

        Ok(Self { db: Mutex::new(db) })
    }

    /// Insert a manifest entry into the database
    pub async fn insert_manifest_entry(&self, entry: ManifestEntry) -> rusqlite::Result<()> {
        self.db.lock().await.execute(
            "INSERT INTO manifest (file, origin, size, blake2b, sha512)
            VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                entry.file,
                entry.origin.to_str().unwrap(),
                entry.size,
                entry.blake2b.unwrap_or(String::from("NULL")),
                entry.sha512.unwrap_or(String::from("NULL")),
            ),
        )?;

        Ok(())
    }

    /// Insert a src_uri entry
    /// foreign key constraints should ensure file exists in manifest table
    pub async fn insert_src_uri(&self, file: String, uri: String) -> rusqlite::Result<()> {
        self.db.lock().await.execute(
            "INSERT INTO src_uri (uri, file)
            VALUES (?1, ?2)",
            (uri, file),
        )?;

        Ok(())
    }

    /// request src_uris for file
    pub async fn get_src_uri(&self, file: &String) -> rusqlite::Result<Vec<String>> {
        let db_locked = self.db.lock().await;
        let mut stmt = db_locked.prepare("SELECT uri FROM src_uri WHERE file = ?1")?;
        let mut rows = stmt.query(rusqlite::params![file])?;

        let mut src_uri: Vec<String> = Vec::new();
        while let Some(row) = rows.next()? {
            src_uri.push(row.get(0)?);
        }

        Ok(src_uri)
    }
}
