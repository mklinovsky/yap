use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Hold,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Auto,
    Light,
    Dark,
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
    #[serde(default = "default_theme")]
    pub theme: Theme,
}

fn default_max_minutes() -> u32 {
    5
}

fn default_theme() -> Theme {
    Theme::Auto
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
            theme: default_theme(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transformation {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub user_template: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub shortcut: Option<String>,
}

pub const TRANSCRIPT_PLACEHOLDER: &str = "{{transcript}}";

impl Transformation {
    pub fn user_message(&self, transcript: &str) -> String {
        if self.user_template.trim().is_empty() {
            transcript.to_string()
        } else {
            self.user_template
                .replace(TRANSCRIPT_PLACEHOLDER, transcript)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Transformations {
    pub enabled: bool,
    pub base_url: Option<String>,
    pub reuse_api_key: bool,
    pub default_model: String,
    pub items: Vec<Transformation>,
}

impl Default for Transformations {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: None,
            reuse_api_key: true,
            default_model: "gpt-6-luna".into(),
            items: Vec::new(),
        }
    }
}

impl Transformations {
    pub fn validate(&self, dictation_shortcut: &str) -> Result<(), String> {
        for (index, item) in self.items.iter().enumerate() {
            if item.name.trim().is_empty() {
                return Err("Every transformation needs a name.".into());
            }
            if !item.user_template.trim().is_empty()
                && !item.user_template.contains(TRANSCRIPT_PLACEHOLDER)
            {
                return Err(format!(
                    "\"{}\": the user message must contain {TRANSCRIPT_PLACEHOLDER}.",
                    item.name
                ));
            }
            let Some(shortcut) = &item.shortcut else {
                continue;
            };
            if shortcut == dictation_shortcut {
                return Err(format!(
                    "\"{}\": the shortcut is already the dictation shortcut.",
                    item.name
                ));
            }
            if let Some(other) = self.items[index + 1..]
                .iter()
                .find(|other| other.shortcut.as_ref() == Some(shortcut))
            {
                return Err(format!(
                    "\"{}\" and \"{}\" use the same shortcut.",
                    item.name, other.name
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrayState {
    pub selected: Option<String>,
    pub use_once: bool,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            selected: None,
            use_once: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub text: String,
    pub created_at: i64,
    pub cost_in_usd: Option<f64>,
    pub duration_in_seconds: Option<f64>,
    pub size_in_bytes: Option<i64>,
    pub encode_time_in_seconds: Option<f64>,
    pub transcribe_time_in_seconds: Option<f64>,
    pub raw_text: Option<String>,
    pub transformation_name: Option<String>,
    pub transform_error: Option<String>,
    pub transform_cost_in_usd: Option<f64>,
    pub transform_time_in_seconds: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NewHistoryEntry {
    pub text: String,
    pub cost_in_usd: Option<f64>,
    pub duration_in_seconds: f64,
    pub size_in_bytes: i64,
    pub encode_time_in_seconds: f64,
    pub transcribe_time_in_seconds: f64,
    pub raw_text: Option<String>,
    pub transformation_name: Option<String>,
    pub transform_error: Option<String>,
    pub transform_cost_in_usd: Option<f64>,
    pub transform_time_in_seconds: Option<f64>,
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
CREATE TABLE IF NOT EXISTS transformations (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tray_state (
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
    transcribe_time REAL,
    raw_text TEXT,
    transformation TEXT,
    transform_error TEXT,
    transform_cost REAL,
    transform_time REAL
);
";

const HISTORY_COLUMNS: &str = "id, text, created_at, cost, duration, size, encode_time,
    transcribe_time, raw_text, transformation, transform_error, transform_cost, transform_time";

const API_KEY: &str = "api_key";
const TRANSFORM_API_KEY: &str = "transform_api_key";

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

    pub fn add_history(&self, entry: &NewHistoryEntry) -> Result<HistoryEntry, StoreError> {
        Ok(self.conn().query_row(
            &format!(
                "INSERT INTO history (text, created_at, cost, duration, size, encode_time,
                     transcribe_time, raw_text, transformation, transform_error, transform_cost,
                     transform_time)
                 VALUES (?1, CAST(unixepoch('subsec') * 1000 AS INTEGER), ?2, ?3, ?4, ?5, ?6, ?7,
                     ?8, ?9, ?10, ?11)
                 RETURNING {HISTORY_COLUMNS}"
            ),
            rusqlite::params![
                entry.text,
                entry.cost_in_usd,
                entry.duration_in_seconds,
                entry.size_in_bytes,
                entry.encode_time_in_seconds,
                entry.transcribe_time_in_seconds,
                entry.raw_text,
                entry.transformation_name,
                entry.transform_error,
                entry.transform_cost_in_usd,
                entry.transform_time_in_seconds
            ],
            history_entry,
        )?)
    }

    pub fn history(&self) -> Result<Vec<HistoryEntry>, StoreError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!(
            "SELECT {HISTORY_COLUMNS} FROM history ORDER BY id DESC"
        ))?;
        let entries = stmt
            .query_map([], history_entry)?
            .collect::<Result<_, _>>()?;
        Ok(entries)
    }

    pub fn api_key(&self) -> Result<Option<String>, StoreError> {
        self.secret(API_KEY)
    }

    pub fn api_key_preview(&self) -> Result<Option<String>, StoreError> {
        Ok(self.api_key()?.as_deref().map(preview))
    }

    pub fn set_api_key(&self, key: &str) -> Result<(), StoreError> {
        self.set_secret(API_KEY, key)
    }

    fn secret(&self, name: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn()
            .query_row("SELECT value FROM secrets WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .optional()?)
    }

    fn set_secret(&self, name: &str, value: &str) -> Result<(), StoreError> {
        if value.is_empty() {
            self.conn()
                .execute("DELETE FROM secrets WHERE name = ?1", [name])?;
            return Ok(());
        }
        self.conn().execute(
            "INSERT INTO secrets (name, value) VALUES (?1, ?2)
             ON CONFLICT (name) DO UPDATE SET value = excluded.value",
            [name, value],
        )?;
        Ok(())
    }

    fn json_row<T: serde::de::DeserializeOwned>(
        &self,
        table: &str,
    ) -> Result<Option<T>, StoreError> {
        let json: Option<String> = self
            .conn()
            .query_row(
                &format!("SELECT json FROM {table} WHERE id = 1"),
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(json.map(|json| serde_json::from_str(&json)).transpose()?)
    }

    fn save_json_row<T: Serialize>(&self, table: &str, value: &T) -> Result<(), StoreError> {
        self.conn().execute(
            &format!(
                "INSERT INTO {table} (id, json) VALUES (1, ?1)
                 ON CONFLICT (id) DO UPDATE SET json = excluded.json"
            ),
            [serde_json::to_string(value)?],
        )?;
        Ok(())
    }

    pub fn transformations(&self) -> Result<Transformations, StoreError> {
        Ok(self.json_row("transformations")?.unwrap_or_default())
    }

    pub fn save_transformations(
        &self,
        transformations: &Transformations,
    ) -> Result<(), StoreError> {
        self.save_json_row("transformations", transformations)
    }

    pub fn tray_state(&self) -> Result<TrayState, StoreError> {
        Ok(self.json_row("tray_state")?.unwrap_or_default())
    }

    pub fn save_tray_state(&self, state: &TrayState) -> Result<(), StoreError> {
        self.save_json_row("tray_state", state)
    }

    pub fn transform_api_key(&self) -> Result<Option<String>, StoreError> {
        self.secret(TRANSFORM_API_KEY)
    }

    pub fn transform_api_key_preview(&self) -> Result<Option<String>, StoreError> {
        Ok(self.transform_api_key()?.as_deref().map(preview))
    }

    pub fn set_transform_api_key(&self, key: &str) -> Result<(), StoreError> {
        self.set_secret(TRANSFORM_API_KEY, key)
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
        ("raw_text", "TEXT"),
        ("transformation", "TEXT"),
        ("transform_error", "TEXT"),
        ("transform_cost", "REAL"),
        ("transform_time", "REAL"),
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

fn preview(key: &str) -> String {
    let visible = if key.chars().count() > 12 { 4 } else { 2 };
    let prefix: String = key.chars().take(visible).collect();
    format!("{prefix}••••••••")
}

fn history_entry(row: &rusqlite::Row) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: row.get(0)?,
        text: row.get(1)?,
        created_at: row.get(2)?,
        cost_in_usd: row.get(3)?,
        duration_in_seconds: row.get(4)?,
        size_in_bytes: row.get(5)?,
        encode_time_in_seconds: row.get(6)?,
        transcribe_time_in_seconds: row.get(7)?,
        raw_text: row.get(8)?,
        transformation_name: row.get(9)?,
        transform_error: row.get(10)?,
        transform_cost_in_usd: row.get(11)?,
        transform_time_in_seconds: row.get(12)?,
    })
}
