use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::audio::encode_flac;
use crate::store::{Mode, NewHistoryEntry, Store, Transformation, Transformations, TrayState};
use crate::transcriber::{TranscribeRequest, Transcriber};
use crate::transformer::{TransformRequest, Transformer};

#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

const MIN_SECONDS: f32 = 0.3;
const SILENCE_PEAK: f32 = 0.01;

const API_KEY_MISSING: &str = "API key is not set. Open Settings to add it.";
const TRANSFORM_KEY_MISSING: &str =
    "Transformation API key is not set. Open Transformations to add it.";

impl Recording {
    fn is_silent(&self) -> bool {
        self.samples
            .iter()
            .all(|sample| sample.abs() < SILENCE_PEAK)
    }

    fn seconds(&self) -> f32 {
        self.samples.len() as f32 / self.channels.max(1) as f32 / self.sample_rate.max(1) as f32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", content = "message", rename_all = "lowercase")]
pub enum Status {
    Idle,
    Recording,
    Transcribing,
    Transforming,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Start,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Apply {
    TrayPick,
    Transformation(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutEvent {
    Pressed(Apply),
    Released,
}

pub trait Recorder: Send + Sync {
    /// `device` is a backend device id; `None` means the system default input.
    fn start(&self, device: Option<&str>) -> Result<(), String>;
    fn stop(&self) -> Recording;
}

pub trait Paster: Send + Sync {
    fn paste(&self, text: &str) -> Result<(), String>;
}

pub trait Feedback: Send + Sync {
    fn status(&self, status: &Status);
    fn cue(&self, cue: Cue);
    fn history_changed(&self);
    fn tray(&self, state: &TrayState);
}

pub trait Secrets: Send + Sync {
    fn api_key(&self) -> Option<String>;
    fn transform_api_key(&self) -> Option<String>;
}

pub trait Spawner: Send + Sync {
    fn spawn(&self, job: Box<dyn FnOnce() + Send>);
}

pub trait Timer: Send + Sync {
    /// Runs `job` after `delay` on the thread that calls `handle` and `toggle`.
    fn after(&self, delay: Duration, job: Box<dyn FnOnce() + Send>);
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

pub struct Deps {
    pub store: Arc<Store>,
    pub recorder: Arc<dyn Recorder>,
    pub transcriber: Arc<dyn Transcriber>,
    pub transformer: Arc<dyn Transformer>,
    pub paster: Arc<dyn Paster>,
    pub feedback: Arc<dyn Feedback>,
    pub secrets: Arc<dyn Secrets>,
    pub spawner: Arc<dyn Spawner>,
    pub timer: Arc<dyn Timer>,
    pub clock: Arc<dyn Clock>,
}

struct Pending {
    transformation: Transformation,
    consume_tray_pick: bool,
}

#[derive(Clone)]
pub struct Dictation {
    deps: Arc<Deps>,
    status: Arc<Mutex<Status>>,
    /// Counts recordings so a max-length timer only finishes the recording that scheduled it.
    recordings: Arc<Mutex<u64>>,
    started_with: Arc<Mutex<Apply>>,
    tray: Arc<Mutex<TrayState>>,
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Dictation {
    pub fn new(deps: Deps) -> Self {
        let tray = deps.store.tray_state().unwrap_or_default();
        Self {
            deps: Arc::new(deps),
            status: Arc::new(Mutex::new(Status::Idle)),
            recordings: Arc::default(),
            started_with: Arc::new(Mutex::new(Apply::TrayPick)),
            tray: Arc::new(Mutex::new(tray)),
        }
    }

    pub fn handle(&self, event: ShortcutEvent) {
        let mode = self.deps.store.settings().unwrap_or_default().mode;
        match (event, mode, self.status()) {
            (ShortcutEvent::Pressed(apply), _, Status::Idle | Status::Error(_)) => {
                self.start(apply)
            }
            (ShortcutEvent::Released, Mode::Hold, Status::Recording)
            | (ShortcutEvent::Pressed(_), Mode::Toggle, Status::Recording) => self.finish(),
            _ => {}
        }
    }

    pub fn toggle(&self) {
        match self.status() {
            Status::Idle | Status::Error(_) => self.start(Apply::TrayPick),
            Status::Recording => self.finish(),
            Status::Transcribing | Status::Transforming => {}
        }
    }

    pub fn status(&self) -> Status {
        locked(&self.status).clone()
    }

    pub fn tray_state(&self) -> TrayState {
        locked(&self.tray).clone()
    }

    pub fn select(&self, id: Option<String>) {
        self.update_tray(|tray| tray.selected = id);
    }

    pub fn set_use_once(&self, use_once: bool) {
        self.update_tray(|tray| tray.use_once = use_once);
    }

    pub fn clear_pick_if_deleted(&self) {
        let transformations = self.deps.store.transformations().unwrap_or_default();
        let selected = self.tray_state().selected;
        if selected.is_some_and(|id| !transformations.items.iter().any(|item| item.id == id)) {
            self.select(None);
        }
    }

    fn set_status(&self, status: Status) {
        *locked(&self.status) = status.clone();
        self.deps.feedback.status(&status);
    }

    fn cue(&self, cue: Cue) {
        if self.deps.store.settings().unwrap_or_default().sounds {
            self.deps.feedback.cue(cue);
        }
    }

    fn update_tray(&self, change: impl FnOnce(&mut TrayState)) {
        let state = {
            let mut tray = locked(&self.tray);
            change(&mut tray);
            tray.clone()
        };
        let remembered = TrayState {
            selected: state.selected.clone().filter(|_| !state.use_once),
            use_once: state.use_once,
        };
        let _ = self.deps.store.save_tray_state(&remembered);
        self.deps.feedback.tray(&state);
    }

    fn resolve(&self, apply: &Apply, transformations: &Transformations) -> Option<Pending> {
        if !transformations.enabled {
            return None;
        }
        let tray = self.tray_state();
        let (id, consume_tray_pick) = match apply {
            Apply::Transformation(id) => (id.clone(), false),
            Apply::TrayPick => (tray.selected?, tray.use_once),
        };
        let transformation = transformations
            .items
            .iter()
            .find(|item| item.id == id)?
            .clone();
        Some(Pending {
            transformation,
            consume_tray_pick,
        })
    }

    fn transform_api_key(&self, transformations: &Transformations) -> Option<String> {
        let key = if transformations.reuse_api_key {
            self.deps.secrets.api_key()
        } else {
            self.deps.secrets.transform_api_key()
        };
        key.filter(|key| !key.is_empty())
    }

    fn start(&self, apply: Apply) {
        if self.deps.secrets.api_key().is_none_or(|key| key.is_empty()) {
            self.set_status(Status::Error(API_KEY_MISSING.into()));
            return;
        }
        let transformations = self.deps.store.transformations().unwrap_or_default();
        if self.resolve(&apply, &transformations).is_some()
            && self.transform_api_key(&transformations).is_none()
        {
            self.set_status(Status::Error(TRANSFORM_KEY_MISSING.into()));
            return;
        }
        let settings = self.deps.store.settings().unwrap_or_default();
        match self.deps.recorder.start(settings.input_device.as_deref()) {
            Ok(()) => {
                *locked(&self.started_with) = apply;
                self.set_status(Status::Recording);
                self.cue(Cue::Start);
                self.finish_after(Duration::from_secs(u64::from(settings.max_minutes) * 60));
            }
            Err(error) => self.set_status(Status::Error(error)),
        }
    }

    fn finish_after(&self, delay: Duration) {
        let recording = {
            let mut recordings = locked(&self.recordings);
            *recordings += 1;
            *recordings
        };
        let this = self.clone();
        self.deps.timer.after(
            delay,
            Box::new(move || {
                let current = *locked(&this.recordings);
                if current == recording && this.status() == Status::Recording {
                    this.finish();
                }
            }),
        );
    }

    fn finish(&self) {
        let recording = self.deps.recorder.stop();
        self.cue(Cue::Stop);
        if recording.seconds() < MIN_SECONDS || recording.is_silent() {
            self.set_status(Status::Idle);
            return;
        }
        let apply = locked(&self.started_with).clone();
        let transformations = self.deps.store.transformations().unwrap_or_default();
        let pending = self.resolve(&apply, &transformations);
        self.set_status(Status::Transcribing);
        let this = self.clone();
        self.deps.spawner.spawn(Box::new(move || {
            this.transcribe_and_paste(recording, pending)
        }));
    }

    fn transcribe_and_paste(&self, recording: Recording, pending: Option<Pending>) {
        let settings = self.deps.store.settings().unwrap_or_default();
        let started = self.deps.clock.now();
        let audio = encode_flac(
            &recording.samples,
            recording.sample_rate,
            recording.channels,
        );
        let encoded = self.deps.clock.now();
        let size_in_bytes = audio.len() as i64;
        let request = TranscribeRequest {
            base_url: settings.base_url.clone(),
            api_key: self.deps.secrets.api_key().unwrap_or_default(),
            model: settings.model,
            languages: settings.languages,
            keywords: settings.keywords,
            audio,
        };
        let result = self.deps.transcriber.transcribe(request);
        let transcribed = self.deps.clock.now();
        let transcription = match result {
            Ok(transcription) if transcription.text.trim().is_empty() => {
                return self.set_status(Status::Idle);
            }
            Ok(transcription) => transcription,
            Err(error) => return self.set_status(Status::Error(error.to_string())),
        };
        let text = transcription.text.trim();
        let entry = NewHistoryEntry {
            text: text.to_string(),
            cost_in_usd: transcription.cost_in_usd,
            duration_in_seconds: recording.seconds() as f64,
            size_in_bytes,
            encode_time_in_seconds: encoded.duration_since(started).as_secs_f64(),
            transcribe_time_in_seconds: transcribed.duration_since(encoded).as_secs_f64(),
            ..NewHistoryEntry::default()
        };
        match pending {
            None => {
                self.save(&entry);
                self.paste(text);
            }
            Some(pending) => self.transform_and_paste(entry, pending, &settings.base_url),
        }
    }

    fn transform_and_paste(&self, entry: NewHistoryEntry, pending: Pending, base_url: &str) {
        let transformation = pending.transformation;
        if pending.consume_tray_pick
            && self.tray_state().selected.as_ref() == Some(&transformation.id)
        {
            self.select(None);
        }
        let raw = entry.text.clone();
        let failed = |error: String, cost: Option<f64>, time: Option<f64>| {
            self.save(&NewHistoryEntry {
                transformation_name: Some(transformation.name.clone()),
                transform_error: Some(error.clone()),
                transform_cost_in_usd: cost,
                transform_time_in_seconds: time,
                ..entry.clone()
            });
            self.set_status(Status::Error(error));
        };
        let transformations = self.deps.store.transformations().unwrap_or_default();
        let Some(api_key) = self.transform_api_key(&transformations) else {
            return failed(TRANSFORM_KEY_MISSING.into(), None, None);
        };
        self.set_status(Status::Transforming);
        let request = TransformRequest {
            base_url: transformations
                .base_url
                .unwrap_or_else(|| base_url.to_string()),
            api_key,
            model: transformation
                .model
                .clone()
                .filter(|model| !model.trim().is_empty())
                .unwrap_or(transformations.default_model),
            system_prompt: transformation.system_prompt.clone(),
            user_message: transformation.user_message(&raw),
        };
        let started = self.deps.clock.now();
        let result = self.deps.transformer.transform(request);
        let time = Some(self.deps.clock.now().duration_since(started).as_secs_f64());
        match result {
            Ok(transformed) if transformed.text.trim().is_empty() => failed(
                "Transformation returned no text".into(),
                transformed.cost_in_usd,
                time,
            ),
            Ok(transformed) => {
                let text = transformed.text.trim();
                self.save(&NewHistoryEntry {
                    text: text.to_string(),
                    raw_text: Some(raw),
                    transformation_name: Some(transformation.name),
                    transform_cost_in_usd: transformed.cost_in_usd,
                    transform_time_in_seconds: time,
                    ..entry
                });
                self.paste(text);
            }
            Err(error) => failed(format!("Transformation failed: {error}"), None, time),
        }
    }

    fn save(&self, entry: &NewHistoryEntry) {
        if self.deps.store.add_history(entry).is_ok() {
            self.deps.feedback.history_changed();
        }
    }

    fn paste(&self, text: &str) {
        match self.deps.paster.paste(text) {
            Ok(()) => self.set_status(Status::Idle),
            Err(error) => self.set_status(Status::Error(error)),
        }
    }
}
