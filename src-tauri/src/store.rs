use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Hold,
    Toggle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub input_device: Option<String>,
    pub shortcut: String,
    pub mode: Mode,
    pub sounds: bool,
    #[serde(default = "default_max_minutes")]
    pub max_minutes: u32,
}

fn default_max_minutes() -> u32 {
    5
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-transcribe".into(),
            languages: Vec::new(),
            keywords: Vec::new(),
            input_device: None,
            shortcut: "Shift+Super+Semicolon".into(),
            mode: Mode::Hold,
            sounds: true,
            max_minutes: default_max_minutes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub text: String,
    pub created_at: i64,
    /// USD, when the endpoint reported it.
    pub cost: Option<f64>,
    /// Seconds of recorded audio; unknown for entries from older versions.
    pub duration: Option<f64>,
    /// Bytes uploaded; unknown for entries from older versions.
    pub size: Option<i64>,
    /// Seconds spent encoding FLAC; unknown for entries from older versions.
    pub encode_time: Option<f64>,
    /// Seconds the transcription request took; unknown for entries from older versions.
    pub transcribe_time: Option<f64>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS secrets (
    name TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    text TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    cost REAL,
    duration REAL,
    size INTEGER,
    encode_time REAL,
    transcribe_time REAL
);
";

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &std::path::Path) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn settings(&self) -> Result<Settings, StoreError> {
        let json: Option<String> = self
            .conn()
            .query_row("SELECT json FROM settings WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()?;
        match json {
            Some(json) => Ok(serde_json::from_value(upgrade(serde_json::from_str(
                &json,
            )?))?),
            None => Ok(Settings::default()),
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), StoreError> {
        self.conn().execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)
             ON CONFLICT (id) DO UPDATE SET json = excluded.json",
            [serde_json::to_string(settings)?],
        )?;
        Ok(())
    }

    pub fn add_history(
        &self,
        text: &str,
        cost: Option<f64>,
        duration: f64,
        size: i64,
        encode_time: f64,
        transcribe_time: f64,
    ) -> Result<HistoryEntry, StoreError> {
        Ok(self.conn().query_row(
            "INSERT INTO history (text, created_at, cost, duration, size, encode_time, transcribe_time)
             VALUES (?1, CAST(unixepoch('subsec') * 1000 AS INTEGER), ?2, ?3, ?4, ?5, ?6)
             RETURNING id, text, created_at, cost, duration, size, encode_time, transcribe_time",
            rusqlite::params![text, cost, duration, size, encode_time, transcribe_time],
            history_entry,
        )?)
    }

    pub fn history(&self) -> Result<Vec<HistoryEntry>, StoreError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, text, created_at, cost, duration, size, encode_time, transcribe_time
             FROM history ORDER BY id DESC",
        )?;
        let entries = stmt
            .query_map([], history_entry)?
            .collect::<Result<_, _>>()?;
        Ok(entries)
    }

    pub fn api_key(&self) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT value FROM secrets WHERE name = 'api_key'",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn api_key_preview(&self) -> Result<Option<String>, StoreError> {
        Ok(self.api_key()?.map(|key| {
            let visible = if key.chars().count() > 12 { 4 } else { 2 };
            let prefix: String = key.chars().take(visible).collect();
            format!("{prefix}••••••••")
        }))
    }

    pub fn set_api_key(&self, key: &str) -> Result<(), StoreError> {
        if key.is_empty() {
            self.conn()
                .execute("DELETE FROM secrets WHERE name = 'api_key'", [])?;
            return Ok(());
        }
        self.conn().execute(
            "INSERT INTO secrets (name, value) VALUES ('api_key', ?1)
             ON CONFLICT (name) DO UPDATE SET value = excluded.value",
            [key],
        )?;
        Ok(())
    }

    pub fn delete_history(&self, id: i64) -> Result<(), StoreError> {
        self.conn()
            .execute("DELETE FROM history WHERE id = ?1", [id])?;
        Ok(())
    }
}

fn migrate(conn: &Connection) -> Result<(), StoreError> {
    for (column, kind) in [
        ("cost", "REAL"),
        ("duration", "REAL"),
        ("size", "INTEGER"),
        ("encode_time", "REAL"),
        ("transcribe_time", "REAL"),
    ] {
        let exists: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info('history') WHERE name = ?1)",
            [column],
            |row| row.get(0),
        )?;
        if !exists {
            conn.execute(
                &format!("ALTER TABLE history ADD COLUMN {column} {kind}"),
                [],
            )?;
        }
    }
    Ok(())
}

// Older versions stored a single optional `language` before the list of languages existed.
fn upgrade(mut json: serde_json::Value) -> serde_json::Value {
    if let Some(object) = json.as_object_mut() {
        if !object.contains_key("languages") {
            let languages = match object.remove("language") {
                Some(serde_json::Value::String(language)) => vec![language],
                _ => Vec::new(),
            };
            object.insert("languages".into(), languages.into());
        }
    }
    json
}

fn history_entry(row: &rusqlite::Row) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: row.get(0)?,
        text: row.get(1)?,
        created_at: row.get(2)?,
        cost: row.get(3)?,
        duration: row.get(4)?,
        size: row.get(5)?,
        encode_time: row.get(6)?,
        transcribe_time: row.get(7)?,
    })
}
