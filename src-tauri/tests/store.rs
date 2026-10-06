use std::path::{Path, PathBuf};

use yap_lib::store::{
    Mode, NewHistoryEntry, Settings, Stats, Store, Theme, Transformation, Transformations,
    TrayState, Usage,
};

#[test]
fn fresh_store_returns_default_settings() {
    let store = Store::open_in_memory().unwrap();

    assert_eq!(
        store.settings().unwrap(),
        Settings {
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-transcribe".into(),
            languages: vec![],
            keywords: vec![],
            input_device: None,
            shortcut: "Shift+Super+Semicolon".into(),
            mode: Mode::Hold,
            sounds: true,
            max_minutes: 5,
            theme: Theme::Auto,
        }
    );
}

#[test]
fn saved_settings_survive_reopening_the_database() {
    let path = std::env::temp_dir().join(format!("yap-test-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let settings = Settings {
        base_url: "https://api.groq.com/openai/v1".into(),
        model: "whisper-large-v3".into(),
        languages: vec!["sk".into(), "en".into()],
        keywords: vec!["Tauri".into(), "rusqlite".into()],
        input_device: Some("coreaudio:BuiltInMicrophoneDevice".into()),
        shortcut: "Ctrl+Shift+D".into(),
        mode: Mode::Toggle,
        sounds: false,
        max_minutes: 15,
        theme: Theme::Light,
    };

    Store::open(&path)
        .unwrap()
        .save_settings(&settings)
        .unwrap();
    let reopened = Store::open(&path).unwrap().settings().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(reopened, settings);
}

#[test]
fn history_lists_transcripts_newest_first() {
    let store = Store::open_in_memory().unwrap();

    store
        .add_history(&NewHistoryEntry {
            text: "first".into(),
            duration_in_seconds: 1.0,
            size_in_bytes: 1,
            ..NewHistoryEntry::default()
        })
        .unwrap();
    store
        .add_history(&NewHistoryEntry {
            text: "second".into(),
            duration_in_seconds: 1.0,
            size_in_bytes: 1,
            ..NewHistoryEntry::default()
        })
        .unwrap();

    let texts: Vec<String> = store
        .history()
        .unwrap()
        .into_iter()
        .map(|entry| entry.text)
        .collect();
    assert_eq!(texts, ["second", "first"]);
}

#[test]
fn deleted_history_entry_is_no_longer_listed() {
    let store = Store::open_in_memory().unwrap();
    let keep = store
        .add_history(&NewHistoryEntry {
            text: "keep".into(),
            duration_in_seconds: 1.0,
            size_in_bytes: 1,
            ..NewHistoryEntry::default()
        })
        .unwrap();
    let drop = store
        .add_history(&NewHistoryEntry {
            text: "drop".into(),
            duration_in_seconds: 1.0,
            size_in_bytes: 1,
            ..NewHistoryEntry::default()
        })
        .unwrap();

    store.delete_history(drop.id).unwrap();

    assert_eq!(store.history().unwrap(), [keep]);
}

#[test]
fn settings_saved_before_keywords_existed_still_load() {
    let path = std::env::temp_dir().join(format!("yap-legacy-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    drop(Store::open(&path).unwrap());
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)",
            [r#"{"baseUrl":"https://api.groq.com/openai/v1","model":"whisper-large-v3","language":null,"shortcut":"Alt+Space","mode":"hold","sounds":true}"#],
        )
        .unwrap();

    let settings = Store::open(&path).unwrap().settings().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(
        (settings.model.as_str(), settings.keywords),
        ("whisper-large-v3", vec![])
    );
}

#[test]
fn settings_saved_before_max_length_existed_default_to_five_minutes() {
    let path = std::env::temp_dir().join(format!("yap-max-length-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    drop(Store::open(&path).unwrap());
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)",
            [r#"{"baseUrl":"https://api.openai.com/v1","model":"gpt-transcribe","languages":[],"keywords":[],"inputDevice":null,"shortcut":"Alt+Space","mode":"toggle","sounds":true}"#],
        )
        .unwrap();

    let settings = Store::open(&path).unwrap().settings().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(settings.max_minutes, 5);
}

#[test]
fn settings_saved_before_theme_existed_default_to_auto() {
    let path = std::env::temp_dir().join(format!("yap-theme-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    drop(Store::open(&path).unwrap());
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)",
            [r#"{"baseUrl":"https://api.openai.com/v1","model":"gpt-transcribe","languages":[],"keywords":[],"inputDevice":null,"shortcut":"Alt+Space","mode":"toggle","sounds":true,"maxMinutes":5}"#],
        )
        .unwrap();

    let settings = Store::open(&path).unwrap().settings().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(settings.theme, Theme::Auto);
}

#[test]
fn single_language_saved_by_older_versions_loads_as_language_list() {
    let path = std::env::temp_dir().join(format!("yap-language-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    drop(Store::open(&path).unwrap());
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO settings (id, json) VALUES (1, ?1)",
            [r#"{"baseUrl":"https://api.openai.com/v1","model":"whisper-1","language":"sk","keywords":[],"shortcut":"Alt+Space","mode":"hold","sounds":true}"#],
        )
        .unwrap();

    let settings = Store::open(&path).unwrap().settings().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(settings.languages, ["sk"]);
}

#[test]
fn saved_api_key_survives_reopening_the_database() {
    let path = std::env::temp_dir().join(format!("yap-key-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);

    Store::open(&path)
        .unwrap()
        .set_api_key("sk-secret")
        .unwrap();
    let key = Store::open(&path).unwrap().api_key().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(key.as_deref(), Some("sk-secret"));
}

#[test]
fn saving_an_empty_api_key_removes_it() {
    let store = Store::open_in_memory().unwrap();
    store.set_api_key("sk-secret").unwrap();

    store.set_api_key("").unwrap();

    assert_eq!(store.api_key().unwrap(), None);
}

#[test]
fn api_key_preview_shows_only_the_first_characters() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.api_key_preview().unwrap(), None);

    store.set_api_key("sk-proj-abcdefgh1234").unwrap();
    let long = store.api_key_preview().unwrap();
    store.set_api_key("abc12345").unwrap();
    let short = store.api_key_preview().unwrap();

    assert_eq!(
        (long.as_deref(), short.as_deref()),
        (Some("sk-p••••••••"), Some("ab••••••••"))
    );
}

#[test]
fn history_entry_keeps_its_cost() {
    let store = Store::open_in_memory().unwrap();

    store
        .add_history(&NewHistoryEntry {
            text: "priced".into(),
            cost_in_usd: Some(0.0042),
            duration_in_seconds: 1.0,
            size_in_bytes: 1,
            ..NewHistoryEntry::default()
        })
        .unwrap();
    store
        .add_history(&NewHistoryEntry {
            text: "unpriced".into(),
            duration_in_seconds: 1.0,
            size_in_bytes: 1,
            ..NewHistoryEntry::default()
        })
        .unwrap();

    let costs: Vec<Option<f64>> = store
        .history()
        .unwrap()
        .into_iter()
        .map(|entry| entry.cost_in_usd)
        .collect();
    assert_eq!(costs, [None, Some(0.0042)]);
}

#[test]
fn history_entry_keeps_its_recording_duration_and_upload_size() {
    let store = Store::open_in_memory().unwrap();

    store
        .add_history(&NewHistoryEntry {
            text: "timed".into(),
            duration_in_seconds: 12.5,
            size_in_bytes: 204_800,
            ..NewHistoryEntry::default()
        })
        .unwrap();

    let entry = store.history().unwrap().remove(0);
    assert_eq!(
        (entry.duration_in_seconds, entry.size_in_bytes),
        (Some(12.5), Some(204_800))
    );
}

#[test]
fn history_entry_keeps_its_encode_and_transcribe_times() {
    let store = Store::open_in_memory().unwrap();

    store
        .add_history(&NewHistoryEntry {
            text: "timed".into(),
            duration_in_seconds: 12.5,
            size_in_bytes: 204_800,
            encode_time_in_seconds: 0.03,
            transcribe_time_in_seconds: 4.2,
            ..NewHistoryEntry::default()
        })
        .unwrap();

    let entry = store.history().unwrap().remove(0);
    assert_eq!(
        (
            entry.encode_time_in_seconds,
            entry.transcribe_time_in_seconds
        ),
        (Some(0.03), Some(4.2))
    );
}

#[test]
fn history_from_older_versions_gains_missing_columns() {
    let path = std::env::temp_dir().join(format!("yap-history-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE history (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 text TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             INSERT INTO history (text, created_at) VALUES ('old', 1);",
        )
        .unwrap();

    let store = Store::open(&path).unwrap();
    store
        .add_history(&NewHistoryEntry {
            text: "new".into(),
            cost_in_usd: Some(0.5),
            duration_in_seconds: 2.0,
            size_in_bytes: 64_000,
            encode_time_in_seconds: 0.03,
            transcribe_time_in_seconds: 4.2,
            ..NewHistoryEntry::default()
        })
        .unwrap();
    let entries: Vec<_> = store
        .history()
        .unwrap()
        .into_iter()
        .map(|entry| {
            (
                entry.text,
                entry.cost_in_usd,
                entry.duration_in_seconds,
                entry.size_in_bytes,
                entry.encode_time_in_seconds,
                entry.transcribe_time_in_seconds,
            )
        })
        .collect();
    drop(store);
    std::fs::remove_file(&path).unwrap();

    assert_eq!(
        entries,
        [
            (
                "new".into(),
                Some(0.5),
                Some(2.0),
                Some(64_000),
                Some(0.03),
                Some(4.2)
            ),
            ("old".into(), None, None, None, None, None)
        ]
    );
}

fn transformation(id: &str, name: &str) -> Transformation {
    Transformation {
        id: id.into(),
        name: name.into(),
        system_prompt: String::new(),
        user_template: String::new(),
        model: None,
        shortcut: None,
    }
}

#[test]
fn fresh_store_has_transformations_disabled() {
    let store = Store::open_in_memory().unwrap();

    assert_eq!(
        store.transformations().unwrap(),
        Transformations {
            enabled: false,
            base_url: None,
            reuse_api_key: true,
            default_model: "gpt-6-luna".into(),
            items: vec![],
        }
    );
}

#[test]
fn saved_transformations_survive_reopening_the_database() {
    let path = std::env::temp_dir().join(format!("yap-transformations-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let transformations = Transformations {
        enabled: true,
        base_url: Some("https://llm.example.com/v1".into()),
        reuse_api_key: false,
        default_model: "gpt-5".into(),
        items: vec![Transformation {
            system_prompt: "Summarize.".into(),
            user_template: "Text: {{transcript}}".into(),
            model: Some("gpt-5-nano".into()),
            shortcut: Some("Ctrl+Alt+S".into()),
            ..transformation("t1", "Summary")
        }],
    };

    Store::open(&path)
        .unwrap()
        .save_transformations(&transformations)
        .unwrap();
    let reopened = Store::open(&path).unwrap().transformations().unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(reopened, transformations);
}

#[test]
fn tray_state_defaults_to_use_once_with_nothing_selected_and_round_trips() {
    let store = Store::open_in_memory().unwrap();
    let fresh = store.tray_state().unwrap();

    store
        .save_tray_state(&TrayState {
            selected: Some("t1".into()),
            use_once: false,
        })
        .unwrap();

    assert_eq!(
        (fresh, store.tray_state().unwrap()),
        (
            TrayState {
                selected: None,
                use_once: true
            },
            TrayState {
                selected: Some("t1".into()),
                use_once: false
            }
        )
    );
}

#[test]
fn transformation_api_key_is_stored_apart_from_the_transcription_key() {
    let store = Store::open_in_memory().unwrap();
    store.set_api_key("sk-transcription").unwrap();

    store.set_transform_api_key("sk-llm-key-1234567").unwrap();

    assert_eq!(
        (
            store.api_key().unwrap().as_deref(),
            store.transform_api_key().unwrap().as_deref(),
            store.transform_api_key_preview().unwrap().as_deref()
        ),
        (
            Some("sk-transcription"),
            Some("sk-llm-key-1234567"),
            Some("sk-l••••••••")
        )
    );
}

#[test]
fn saving_an_empty_transformation_api_key_removes_it() {
    let store = Store::open_in_memory().unwrap();
    store.set_transform_api_key("sk-llm").unwrap();

    store.set_transform_api_key("").unwrap();

    assert_eq!(store.transform_api_key().unwrap(), None);
}

#[test]
fn history_entry_keeps_its_transformation_details() {
    let store = Store::open_in_memory().unwrap();

    store
        .add_history(&NewHistoryEntry {
            text: "Hello, world.".into(),
            raw_text: Some("hello world".into()),
            transformation_name: Some("Fix grammar".into()),
            transform_cost_in_usd: Some(0.0007),
            transform_time_in_seconds: Some(1.25),
            ..NewHistoryEntry::default()
        })
        .unwrap();
    store
        .add_history(&NewHistoryEntry {
            text: "hello again".into(),
            transformation_name: Some("Translate".into()),
            transform_error: Some("HTTP 500: boom".into()),
            ..NewHistoryEntry::default()
        })
        .unwrap();

    let entries: Vec<_> = store
        .history()
        .unwrap()
        .into_iter()
        .map(|e| {
            (
                e.raw_text,
                e.transformation_name,
                e.transform_error,
                e.transform_cost_in_usd,
                e.transform_time_in_seconds,
            )
        })
        .collect();
    assert_eq!(
        entries,
        [
            (
                None,
                Some("Translate".into()),
                Some("HTTP 500: boom".into()),
                None,
                None
            ),
            (
                Some("hello world".into()),
                Some("Fix grammar".into()),
                None,
                Some(0.0007),
                Some(1.25)
            ),
        ]
    );
}

#[test]
fn history_from_older_versions_gains_transformation_columns() {
    let path = std::env::temp_dir().join(format!("yap-history-tf-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE history (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 text TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             INSERT INTO history (text, created_at) VALUES ('old', 1);",
        )
        .unwrap();

    let store = Store::open(&path).unwrap();
    let old = store.history().unwrap().remove(0);
    drop(store);
    std::fs::remove_file(&path).unwrap();

    assert_eq!(
        (
            old.raw_text,
            old.transformation_name,
            old.transform_cost_in_usd
        ),
        (None, None, None)
    );
}

#[test]
fn valid_transformations_pass_validation() {
    let transformations = Transformations {
        items: vec![
            Transformation {
                user_template: "Fix: {{transcript}}".into(),
                shortcut: Some("Ctrl+Alt+G".into()),
                ..transformation("t1", "Fix grammar")
            },
            transformation("t2", "Bullet points"),
        ],
        ..Transformations::default()
    };

    assert_eq!(transformations.validate("Shift+Super+Semicolon"), Ok(()));
}

#[test]
fn validation_rejects_a_user_message_without_the_placeholder() {
    let transformations = Transformations {
        items: vec![Transformation {
            user_template: "Fix this".into(),
            ..transformation("t1", "Fix grammar")
        }],
        ..Transformations::default()
    };

    assert_eq!(
        transformations.validate("Shift+Super+Semicolon"),
        Err("\"Fix grammar\": the user message must contain {{transcript}}.".into())
    );
}

#[test]
fn validation_rejects_a_transformation_without_a_name() {
    let transformations = Transformations {
        items: vec![transformation("t1", "  ")],
        ..Transformations::default()
    };

    assert_eq!(
        transformations.validate("Shift+Super+Semicolon"),
        Err("Every transformation needs a name.".into())
    );
}

#[test]
fn validation_rejects_the_dictation_shortcut() {
    let transformations = Transformations {
        items: vec![Transformation {
            shortcut: Some("Shift+Super+Semicolon".into()),
            ..transformation("t1", "Fix grammar")
        }],
        ..Transformations::default()
    };

    assert_eq!(
        transformations.validate("Shift+Super+Semicolon"),
        Err("\"Fix grammar\": the shortcut is already the dictation shortcut.".into())
    );
}

#[test]
fn validation_rejects_two_transformations_with_the_same_shortcut() {
    let transformations = Transformations {
        items: vec![
            Transformation {
                shortcut: Some("Ctrl+Alt+G".into()),
                ..transformation("t1", "Fix grammar")
            },
            Transformation {
                shortcut: Some("Ctrl+Alt+G".into()),
                ..transformation("t2", "Translate")
            },
        ],
        ..Transformations::default()
    };

    assert_eq!(
        transformations.validate("Shift+Super+Semicolon"),
        Err("\"Fix grammar\" and \"Translate\" use the same shortcut.".into())
    );
}

#[test]
fn user_message_puts_the_transcript_into_the_template() {
    let templated = Transformation {
        user_template: "Rewrite: {{transcript}}".into(),
        ..transformation("t1", "Rewrite")
    };
    let empty = transformation("t2", "Plain");

    assert_eq!(
        (
            templated.user_message("hello world"),
            empty.user_message("hello world")
        ),
        (
            "Rewrite: hello world".to_string(),
            "hello world".to_string()
        )
    );
}

fn temp_db(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("yap-{name}-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn add_history_at(path: &Path, store: &Store, created_at: i64, entry: NewHistoryEntry) {
    let id = store.add_history(&entry).unwrap().id;
    rusqlite::Connection::open(path)
        .unwrap()
        .execute(
            "UPDATE history SET created_at = ?1 WHERE id = ?2",
            [created_at, id],
        )
        .unwrap();
}

#[test]
fn stats_sum_usage_per_bucket_and_over_the_whole_range() {
    let path = temp_db("stats");
    let store = Store::open(&path).unwrap();
    let entry = |text: &str, duration_in_seconds: f64| NewHistoryEntry {
        text: text.into(),
        duration_in_seconds,
        ..NewHistoryEntry::default()
    };
    add_history_at(&path, &store, 999, entry("before the range", 9.0));
    add_history_at(
        &path,
        &store,
        1000,
        NewHistoryEntry {
            cost_in_usd: Some(0.25),
            ..entry("one two three", 2.0)
        },
    );
    add_history_at(
        &path,
        &store,
        1999,
        NewHistoryEntry {
            transform_cost_in_usd: Some(0.125),
            ..entry("four", 1.0)
        },
    );
    add_history_at(
        &path,
        &store,
        3500,
        NewHistoryEntry {
            cost_in_usd: Some(0.5),
            ..entry("  five   six\nseven ", 3.5)
        },
    );
    add_history_at(&path, &store, 4000, entry("at the end", 9.0));

    let stats = store.stats(&[1000, 2000, 3000], 4000).unwrap();
    drop(store);
    std::fs::remove_file(&path).unwrap();

    assert_eq!(
        stats,
        Stats {
            total: Usage {
                dictations: 3,
                duration_in_seconds: 6.5,
                words: 7,
                transcription_cost_in_usd: Some(0.75),
                transformation_cost_in_usd: Some(0.125),
            },
            buckets: vec![
                Usage {
                    dictations: 2,
                    duration_in_seconds: 3.0,
                    words: 4,
                    transcription_cost_in_usd: Some(0.25),
                    transformation_cost_in_usd: Some(0.125),
                },
                Usage::default(),
                Usage {
                    dictations: 1,
                    duration_in_seconds: 3.5,
                    words: 3,
                    transcription_cost_in_usd: Some(0.5),
                    transformation_cost_in_usd: None,
                },
            ],
        }
    );
}

#[test]
fn first_history_at_is_the_oldest_entry_time() {
    let path = temp_db("first");
    let store = Store::open(&path).unwrap();
    let empty = store.first_history_at().unwrap();
    add_history_at(&path, &store, 3000, NewHistoryEntry::default());
    add_history_at(&path, &store, 1000, NewHistoryEntry::default());

    let first = store.first_history_at().unwrap();
    drop(store);
    std::fs::remove_file(&path).unwrap();

    assert_eq!((empty, first), (None, Some(1000)));
}
