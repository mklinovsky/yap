pub mod audio;
pub mod dictation;
pub mod paster;
pub mod recorder;
pub mod store;
pub mod transcriber;
pub mod tray;

use std::sync::mpsc;
use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent, Runtime, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use dictation::{Deps, Dictation, Feedback, ShortcutEvent, Spawner, Status};
use paster::ClipboardPaster;
use recorder::CpalRecorder;
use store::{HistoryEntry, Settings, Store};
use transcriber::HttpTranscriber;
use tray::{dot, TrayFeedback};

struct ThreadSpawner;

impl Spawner for ThreadSpawner {
    fn spawn(&self, job: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(job);
    }
}

enum Input {
    Shortcut(ShortcutEvent),
    Toggle,
}

impl dictation::Secrets for Store {
    fn api_key(&self) -> Option<String> {
        Store::api_key(self).ok().flatten()
    }
}

struct AppState {
    store: Arc<Store>,
    paster: Arc<ClipboardPaster<Wry>>,
    dictation: Dictation,
    inputs: mpsc::Sender<Input>,
}

const MAIN_WINDOW: &str = "main";

type State<'a> = tauri::State<'a, AppState>;

#[tauri::command]
async fn get_settings(state: State<'_>) -> Result<Settings, String> {
    state.store.settings().map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_settings(app: AppHandle, state: State<'_>, settings: Settings) -> Result<(), String> {
    let previous = state.store.settings().map_err(|e| e.to_string())?;
    if settings.shortcut != previous.shortcut {
        let shortcut = parse_shortcut(&settings.shortcut)?;
        let shortcuts = app.global_shortcut();
        shortcuts.unregister_all().map_err(|e| e.to_string())?;
        if let Err(error) = shortcuts.register(shortcut) {
            if let Ok(previous) = parse_shortcut(&previous.shortcut) {
                let _ = shortcuts.register(previous);
            }
            return Err(format!(
                "Shortcut \"{}\" is unavailable: {error}",
                settings.shortcut
            ));
        }
    }
    state
        .store
        .save_settings(&settings)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn api_key_preview(state: State<'_>) -> Result<Option<String>, String> {
    state.store.api_key_preview().map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_api_key(state: State<'_>, key: String) -> Result<(), String> {
    state
        .store
        .set_api_key(key.trim())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_history(state: State<'_>) -> Result<Vec<HistoryEntry>, String> {
    state.store.history().map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_history(state: State<'_>, id: i64) -> Result<(), String> {
    state.store.delete_history(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn copy_text(state: State<'_>, text: String) -> Result<(), String> {
    state.paster.copy(&text)
}

#[tauri::command]
async fn list_input_devices() -> Result<Vec<recorder::InputDevice>, String> {
    Ok(recorder::input_devices())
}

#[tauri::command]
async fn get_status(state: State<'_>) -> Result<Status, String> {
    Ok(state.dictation.status())
}

#[tauri::command]
async fn toggle_recording(state: State<'_>) -> Result<(), String> {
    state.inputs.send(Input::Toggle).map_err(|e| e.to_string())
}

#[tauri::command]
async fn pause_shortcut(app: AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn resume_shortcut(app: AppHandle, state: State<'_>) -> Result<(), String> {
    let shortcut = state.store.settings().map_err(|e| e.to_string())?.shortcut;
    register_shortcut(&app, &shortcut)
}

fn register_shortcut<R: Runtime>(app: &AppHandle<R>, shortcut: &str) -> Result<(), String> {
    let parsed = parse_shortcut(shortcut)?;
    let shortcuts = app.global_shortcut();
    if shortcuts.is_registered(parsed) {
        return Ok(());
    }
    shortcuts.register(parsed).map_err(|e| e.to_string())
}

fn parse_shortcut(text: &str) -> Result<Shortcut, String> {
    text.parse()
        .map_err(|_| format!("Invalid shortcut \"{text}\""))
}

fn open_window<R: Runtime>(app: &AppHandle<R>) {
    let window = match app.get_webview_window(MAIN_WINDOW) {
        Some(window) => {
            let _ = window.show();
            window
        }
        None => {
            let url = WebviewUrl::App("index.html".into());
            let builder = WebviewWindowBuilder::new(app, MAIN_WINDOW, url)
                .title("yap")
                .inner_size(780.0, 680.0)
                .min_inner_size(640.0, 420.0)
                .theme(Some(tauri::Theme::Dark));
            #[cfg(target_os = "macos")]
            let builder = builder
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true)
                .transparent(true)
                .effects(
                    tauri::window::EffectsBuilder::new()
                        .effect(tauri::window::Effect::Sidebar)
                        .state(tauri::window::EffectState::FollowsWindowActiveState)
                        .build(),
                );
            match builder.build() {
                Ok(window) => window,
                Err(error) => {
                    eprintln!("failed to open window: {error}");
                    return;
                }
            }
        }
    };
    // A tray-only (accessory) app is never activated by showing a window, so keyboard focus
    // stays with the previous app until we activate explicitly.
    let _ = window.set_focus();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    let event = match event.state {
                        ShortcutState::Pressed => ShortcutEvent::Pressed,
                        ShortcutState::Released => ShortcutEvent::Released,
                    };
                    let _ = app.state::<AppState>().inputs.send(Input::Shortcut(event));
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            api_key_preview,
            set_api_key,
            list_history,
            delete_history,
            copy_text,
            list_input_devices,
            get_status,
            toggle_recording,
            pause_shortcut,
            resume_shortcut
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = Arc::new(Store::open(&data_dir.join("yap.db"))?);

            let status_item = MenuItem::with_id(app, "status", "Idle", false, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &status_item,
                    &PredefinedMenuItem::separator(app)?,
                    &MenuItem::with_id(app, "open", "Open yap…", true, None::<&str>)?,
                    &PredefinedMenuItem::separator(app)?,
                    &MenuItem::with_id(app, "quit", "Quit yap", true, None::<&str>)?,
                ],
            )?;
            let tray = TrayIconBuilder::with_id("main")
                .icon(dot([0, 0, 0, 255]))
                .icon_as_template(true)
                .tooltip("yap — Idle")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => open_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            let feedback = Arc::new(TrayFeedback {
                app: app.handle().clone(),
                tray,
                status_item,
            });
            let paster = Arc::new(ClipboardPaster::new(app.handle().clone()));
            let dictation = Dictation::new(Deps {
                store: store.clone(),
                recorder: Arc::new(CpalRecorder::default()),
                transcriber: Arc::new(HttpTranscriber::new()),
                paster: paster.clone(),
                feedback: feedback.clone(),
                secrets: store.clone(),
                spawner: Arc::new(ThreadSpawner),
            });

            // One worker keeps press/release ordered and keeps recorder and database work off the main thread.
            let (inputs, received) = mpsc::channel();
            let worker = dictation.clone();
            std::thread::spawn(move || {
                for input in received {
                    match input {
                        Input::Shortcut(event) => worker.handle(event),
                        Input::Toggle => worker.toggle(),
                    }
                }
            });

            app.manage(AppState {
                store: store.clone(),
                paster,
                dictation,
                inputs,
            });

            let shortcut = store.settings()?.shortcut;
            if let Err(error) = register_shortcut(app.handle(), &shortcut) {
                feedback.status(&Status::Error(format!("Shortcut \"{shortcut}\": {error}")));
            }

            if store.api_key_preview()?.is_none() {
                open_window(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        if let RunEvent::ExitRequested {
            api, code: None, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}
