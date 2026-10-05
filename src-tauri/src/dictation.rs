use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::audio::encode_flac;
use crate::store::{Mode, Store};
use crate::transcriber::{TranscribeRequest, Transcriber};

#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

const MIN_SECONDS: f32 = 0.3;
const SILENCE_PEAK: f32 = 0.01;

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
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(flatten)]
    pub status: Status,
    /// The audio of the last failed transcription is kept until a transcription succeeds.
    pub can_retry: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Start,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutEvent {
    Pressed,
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
    fn status(&self, snapshot: &Snapshot);
    fn cue(&self, cue: Cue);
    fn history_changed(&self);
}

pub trait Secrets: Send + Sync {
    fn api_key(&self) -> Option<String>;
}

pub trait Spawner: Send + Sync {
    fn spawn(&self, job: Box<dyn FnOnce() + Send>);
}

pub trait Timer: Send + Sync {
    /// Runs `job` after `delay` on the thread that calls `handle`, `toggle` and `retry`.
    fn after(&self, delay: Duration, job: Box<dyn FnOnce() + Send>);
}

pub struct Deps {
    pub store: Arc<Store>,
    pub recorder: Arc<dyn Recorder>,
    pub transcriber: Arc<dyn Transcriber>,
    pub paster: Arc<dyn Paster>,
    pub feedback: Arc<dyn Feedback>,
    pub secrets: Arc<dyn Secrets>,
    pub spawner: Arc<dyn Spawner>,
    pub timer: Arc<dyn Timer>,
}

#[derive(Clone)]
pub struct Dictation {
    deps: Arc<Deps>,
    status: Arc<Mutex<Status>>,
    failed: Arc<Mutex<Option<Recording>>>,
    session: Arc<Mutex<u64>>,
}

impl Dictation {
    pub fn new(deps: Deps) -> Self {
        Self {
            deps: Arc::new(deps),
            status: Arc::new(Mutex::new(Status::Idle)),
            failed: Arc::default(),
            session: Arc::default(),
        }
    }

    pub fn handle(&self, event: ShortcutEvent) {
        let mode = self.deps.store.settings().unwrap_or_default().mode;
        match (event, mode, self.status()) {
            (ShortcutEvent::Pressed, _, Status::Idle | Status::Error(_)) => self.start(),
            (ShortcutEvent::Released, Mode::Hold, Status::Recording)
            | (ShortcutEvent::Pressed, Mode::Toggle, Status::Recording) => self.finish(),
            _ => {}
        }
    }

    pub fn toggle(&self) {
        match self.status() {
            Status::Idle | Status::Error(_) => self.start(),
            Status::Recording => self.finish(),
            Status::Transcribing => {}
        }
    }

    pub fn retry(&self) {
        if matches!(self.status(), Status::Recording | Status::Transcribing) {
            return;
        }
        let failed = self.failed().take();
        if let Some(recording) = failed {
            self.transcribe(recording);
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            status: self.status(),
            can_retry: self.failed().is_some(),
        }
    }

    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set_status(&self, status: Status) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = status;
        self.deps.feedback.status(&self.snapshot());
    }

    fn failed(&self) -> std::sync::MutexGuard<'_, Option<Recording>> {
        self.failed.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn cue(&self, cue: Cue) {
        if self.deps.store.settings().unwrap_or_default().sounds {
            self.deps.feedback.cue(cue);
        }
    }

    fn start(&self) {
        if self.deps.secrets.api_key().is_none_or(|key| key.is_empty()) {
            self.set_status(Status::Error(
                "API key is not set. Open Settings to add it.".into(),
            ));
            return;
        }
        let settings = self.deps.store.settings().unwrap_or_default();
        match self.deps.recorder.start(settings.input_device.as_deref()) {
            Ok(()) => {
                self.set_status(Status::Recording);
                self.cue(Cue::Start);
                self.stop_at(Duration::from_secs(u64::from(settings.max_minutes) * 60));
            }
            Err(error) => self.set_status(Status::Error(error)),
        }
    }

    fn stop_at(&self, delay: Duration) {
        let session = {
            let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
            *session += 1;
            *session
        };
        let this = self.clone();
        self.deps.timer.after(
            delay,
            Box::new(move || {
                let current = *this.session.lock().unwrap_or_else(|e| e.into_inner());
                if current == session && this.status() == Status::Recording {
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
        self.transcribe(recording);
    }

    fn transcribe(&self, recording: Recording) {
        self.set_status(Status::Transcribing);
        let this = self.clone();
        self.deps
            .spawner
            .spawn(Box::new(move || this.transcribe_and_paste(recording)));
    }

    fn transcribe_and_paste(&self, recording: Recording) {
        let settings = self.deps.store.settings().unwrap_or_default();
        let audio = encode_flac(
            &recording.samples,
            recording.sample_rate,
            recording.channels,
        );
        let size = audio.len() as i64;
        let request = TranscribeRequest {
            base_url: settings.base_url,
            api_key: self.deps.secrets.api_key().unwrap_or_default(),
            model: settings.model,
            languages: settings.languages,
            keywords: settings.keywords,
            audio,
        };
        let transcription = match self.deps.transcriber.transcribe(request) {
            Ok(transcription) => transcription,
            Err(error) => {
                *self.failed() = Some(recording);
                self.set_status(Status::Error(error.to_string()));
                return;
            }
        };
        *self.failed() = None;
        let text = transcription.text.trim();
        if text.is_empty() {
            self.set_status(Status::Idle);
            return;
        }
        if self
            .deps
            .store
            .add_history(text, transcription.cost, recording.seconds() as f64, size)
            .is_ok()
        {
            self.deps.feedback.history_changed();
        }
        match self.deps.paster.paste(text) {
            Ok(()) => self.set_status(Status::Idle),
            Err(error) => self.set_status(Status::Error(error)),
        }
    }
}
