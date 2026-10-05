use yap_lib::store::{Mode, Settings, Store};

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
            shortcut: "Alt+Space".into(),
            mode: Mode::Hold,
            sounds: true,
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

    store.add_history("first", None).unwrap();
    store.add_history("second", None).unwrap();

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
    let keep = store.add_history("keep", None).unwrap();
    let drop = store.add_history("drop", None).unwrap();

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

    store.add_history("priced", Some(0.0042)).unwrap();
    store.add_history("unpriced", None).unwrap();

    let costs: Vec<Option<f64>> = store
        .history()
        .unwrap()
        .into_iter()
        .map(|entry| entry.cost)
        .collect();
    assert_eq!(costs, [None, Some(0.0042)]);
}

#[test]
fn history_created_before_costs_existed_gains_a_cost_column() {
    let path = std::env::temp_dir().join(format!("yap-cost-{}.db", std::process::id()));
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
    store.add_history("new", Some(0.5)).unwrap();
    let entries: Vec<(String, Option<f64>)> = store
        .history()
        .unwrap()
        .into_iter()
        .map(|entry| (entry.text, entry.cost))
        .collect();
    drop(store);
    std::fs::remove_file(&path).unwrap();

    assert_eq!(entries, [("new".into(), Some(0.5)), ("old".into(), None)]);
}
