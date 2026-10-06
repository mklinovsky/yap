pub mod audio;
pub mod dictation;
pub mod http;
pub mod paster;
pub mod recorder;
pub mod store;
pub mod transcriber;
pub mod transformer;
pub mod tray;

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, RunEvent, Runtime, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use dictation::{Apply, Clock, Deps, Dictation, Feedback, ShortcutEvent, Spawner, Status, Timer};
use paster::ClipboardPaster;
use recorder::CpalRecorder;
use store::{HistoryEntry, Settings, Store, Transformations};
use transcriber::HttpTranscriber;
use transformer::HttpTransformer;
use tray::{dot, TrayFeedback, MENU_OPEN, MENU_PICK_PREFIX, MENU_QUIT, MENU_USE_ONCE};

struct ThreadSpawner;

impl Spawner for ThreadSpawner {
    fn spawn(&self, job: Box<dyn FnOnce() + Send>) {
        std::thread::spawn(job);
    }
}

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

enum Input {
    Shortcut(ShortcutEvent),
    Toggle,
    Select(Option<String>),
    SetUseOnce(bool),
    Run(Box<dyn FnOnce() + Send>),
}

struct WorkerTimer {
    inputs: mpsc::Sender<Input>,
}

impl Timer for WorkerTimer {
    fn after(&self, delay: Duration, job: Box<dyn FnOnce() + Send>) {
        let inputs = self.inputs.clone();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            let _ = inputs.send(Input::Run(job));
        });
    }
}

impl dictation::Secrets for Store {
    fn api_key(&self) -> Option<String> {
        Store::api_key(self).ok().flatten()
    }

    fn transform_api_key(&self) -> Option<String> {
        Store::transform_api_key(self).ok().flatten()
    }
}

struct AppState {
    store: Arc<Store>,
    paster: Arc<ClipboardPaster<Wry>>,
    feedback: Arc<TrayFeedback<Wry>>,
    dictation: Dictation,
    inputs: mpsc::Sender<Input>,
    apply_by_shortcut_id: Mutex<HashMap<u32, Apply>>,
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
        let transformations = state.store.transformations().map_err(|e| e.to_string())?;
        transformations.validate(&settings.shortcut)?;
        rebind_or_restore(
            &app,
            &state,
            (&settings, &transformations),
            (&previous, &transformations),
        )?;
    }
    state
        .store
        .save_settings(&settings)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_transformations(state: State<'_>) -> Result<Transformations, String> {
    state.store.transformations().map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_transformations(
    app: AppHandle,
    state: State<'_>,
    transformations: Transformations,
) -> Result<(), String> {
    let settings = state.store.settings().map_err(|e| e.to_string())?;
    transformations.validate(&settings.shortcut)?;
    let previous = state.store.transformations().map_err(|e| e.to_string())?;
    if bindings(&settings, &transformations)? != bindings(&settings, &previous)? {
        rebind_or_restore(
            &app,
            &state,
            (&settings, &transformations),
            (&settings, &previous),
        )?;
    }
    state
        .store
        .save_transformations(&transformations)
        .map_err(|e| e.to_string())?;
    state.dictation.clear_pick_if_deleted();
    state.feedback.rebuild().map_err(|e| e.to_string())
}

#[tauri::command]
async fn transform_api_key_preview(state: State<'_>) -> Result<Option<String>, String> {
    state
        .store
        .transform_api_key_preview()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_transform_api_key(state: State<'_>, key: String) -> Result<(), String> {
    state
        .store
        .set_transform_api_key(key.trim())
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
async fn get_open_at_login(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_open_at_login(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    }
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
async fn pause_shortcut(app: AppHandle, state: State<'_>) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;
    // An unregistered shortcut never reports its release, which would leave a Hold recording running.
    let _ = state.inputs.send(Input::Shortcut(ShortcutEvent::Released));
    Ok(())
}

#[tauri::command]
async fn resume_shortcut(app: AppHandle, state: State<'_>) -> Result<(), String> {
    let settings = state.store.settings().map_err(|e| e.to_string())?;
    let transformations = state.store.transformations().map_err(|e| e.to_string())?;
    register_all_or_none(&app, &state, &bindings(&settings, &transformations)?)
}

type Binding = (String, Shortcut, Apply);

fn bindings(
    settings: &Settings,
    transformations: &Transformations,
) -> Result<Vec<Binding>, String> {
    let mut bindings = vec![(
        settings.shortcut.clone(),
        parse_shortcut(&settings.shortcut)?,
        Apply::TrayPick,
    )];
    if transformations.enabled {
        for item in &transformations.items {
            if let Some(shortcut) = &item.shortcut {
                bindings.push((
                    shortcut.clone(),
                    parse_shortcut(shortcut)?,
                    Apply::Transformation(item.id.clone()),
                ));
            }
        }
    }
    Ok(bindings)
}

fn register_all_or_none<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    bindings: &[Binding],
) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    shortcuts.unregister_all().map_err(|e| e.to_string())?;
    let mut registered = HashMap::new();
    for (text, shortcut, apply) in bindings {
        if let Err(error) = shortcuts.register(*shortcut) {
            let _ = shortcuts.unregister_all();
            registered.clear();
            *state
                .apply_by_shortcut_id
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = registered;
            return Err(format!("Shortcut \"{text}\" is unavailable: {error}"));
        }
        registered.insert(shortcut.id(), apply.clone());
    }
    *state
        .apply_by_shortcut_id
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = registered;
    Ok(())
}

fn rebind_or_restore<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    next: (&Settings, &Transformations),
    previous: (&Settings, &Transformations),
) -> Result<(), String> {
    let next = bindings(next.0, next.1)?;
    register_all_or_none(app, state, &next).inspect_err(|_| {
        if let Ok(previous) = bindings(previous.0, previous.1) {
            let _ = register_all_or_none(app, state, &previous);
        }
    })
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
                .inner_size(780.0, 800.0)
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
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(MacosLauncher::LaunchAgent)
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    let state = app.state::<AppState>();
                    let event = match event.state {
                        ShortcutState::Pressed => {
                            let bindings = state
                                .apply_by_shortcut_id
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            let apply = bindings.get(&shortcut.id()).cloned();
                            ShortcutEvent::Pressed(apply.unwrap_or(Apply::TrayPick))
                        }
                        ShortcutState::Released => ShortcutEvent::Released,
                    };
                    let _ = state.inputs.send(Input::Shortcut(event));
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_transformations,
            save_transformations,
            transform_api_key_preview,
            set_transform_api_key,
            api_key_preview,
            set_api_key,
            get_open_at_login,
            set_open_at_login,
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

            let tray = TrayIconBuilder::with_id("main")
                .icon(dot([0, 0, 0, 255]))
                .icon_as_template(true)
                .tooltip("yap — Idle")
                .on_menu_event(|app, event| {
                    let id = event.id().as_ref();
                    let state = app.state::<AppState>();
                    let inputs = &state.inputs;
                    if id == MENU_OPEN {
                        open_window(app);
                    } else if id == MENU_QUIT {
                        app.exit(0);
                    } else if id == MENU_USE_ONCE {
                        let use_once = state.dictation.tray_state().use_once;
                        let _ = inputs.send(Input::SetUseOnce(!use_once));
                    } else if let Some(pick) = id.strip_prefix(MENU_PICK_PREFIX) {
                        let pick = (!pick.is_empty()).then(|| pick.to_string());
                        let _ = inputs.send(Input::Select(pick));
                    }
                })
                .build(app)?;
            let feedback = Arc::new(TrayFeedback::new(
                app.handle().clone(),
                tray,
                store.clone(),
            )?);
            let paster = Arc::new(ClipboardPaster::new(app.handle().clone()));
            // One worker keeps press/release ordered and keeps recorder and database work off the main thread.
            let (inputs, received) = mpsc::channel();
            let dictation = Dictation::new(Deps {
                store: store.clone(),
                recorder: Arc::new(CpalRecorder::default()),
                transcriber: Arc::new(HttpTranscriber::new()),
                transformer: Arc::new(HttpTransformer::new()),
                paster: paster.clone(),
                feedback: feedback.clone(),
                secrets: store.clone(),
                spawner: Arc::new(ThreadSpawner),
                timer: Arc::new(WorkerTimer {
                    inputs: inputs.clone(),
                }),
                clock: Arc::new(SystemClock),
            });

            let worker = dictation.clone();
            std::thread::spawn(move || {
                for input in received {
                    match input {
                        Input::Shortcut(event) => worker.handle(event),
                        Input::Toggle => worker.toggle(),
                        Input::Select(id) => worker.select(id),
                        Input::SetUseOnce(use_once) => worker.set_use_once(use_once),
                        Input::Run(job) => job(),
                    }
                }
            });

            app.manage(AppState {
                store: store.clone(),
                paster,
                feedback: feedback.clone(),
                dictation,
                inputs,
                apply_by_shortcut_id: Mutex::default(),
            });

            let state = app.state::<AppState>();
            let registered = bindings(&store.settings()?, &store.transformations()?)
                .and_then(|bindings| register_all_or_none(app.handle(), &state, &bindings));
            if let Err(error) = registered {
                feedback.status(&Status::Error(error));
            }

            if store.api_key_preview()?.is_none() {
                open_window(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| match event {
        RunEvent::ExitRequested {
            api, code: None, ..
        } => api.prevent_exit(),
        // Launching yap again while it runs; the way in when the menu bar icon is hidden.
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => open_window(app),
        _ => {}
    });
}
