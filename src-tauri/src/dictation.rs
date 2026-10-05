use std::sync::{Arc, Mutex};

use crate::store::{Mode, Store};
use crate::transcriber::{TranscribeRequest, Transcriber};
use crate::wav::encode_wav;

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
    fn status(&self, status: &Status);
    fn cue(&self, cue: Cue);
    fn history_changed(&self);
}

pub trait Secrets: Send + Sync {
    fn api_key(&self) -> Option<String>;
}

pub trait Spawner: Send + Sync {
    fn spawn(&self, job: Box<dyn FnOnce() + Send>);
}

pub struct Deps {
    pub store: Arc<Store>,
    pub recorder: Arc<dyn Recorder>,
    pub transcriber: Arc<dyn Transcriber>,
    pub paster: Arc<dyn Paster>,
    pub feedback: Arc<dyn Feedback>,
    pub secrets: Arc<dyn Secrets>,
    pub spawner: Arc<dyn Spawner>,
}

#[derive(Clone)]
pub struct Dictation {
    deps: Arc<Deps>,
    status: Arc<Mutex<Status>>,
}

impl Dictation {
    pub fn new(deps: Deps) -> Self {
        Self {
            deps: Arc::new(deps),
            status: Arc::new(Mutex::new(Status::Idle)),
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

    pub fn status(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn set_status(&self, status: Status) {
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = status.clone();
        self.deps.feedback.status(&status);
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
        let device = self.deps.store.settings().unwrap_or_default().input_device;
        match self.deps.recorder.start(device.as_deref()) {
            Ok(()) => {
                self.set_status(Status::Recording);
                self.cue(Cue::Start);
            }
            Err(error) => self.set_status(Status::Error(error)),
        }
    }

    fn finish(&self) {
        let recording = self.deps.recorder.stop();
        self.cue(Cue::Stop);
        if recording.seconds() < MIN_SECONDS || recording.is_silent() {
            self.set_status(Status::Idle);
            return;
        }
        self.set_status(Status::Transcribing);
        let this = self.clone();
        self.deps
            .spawner
            .spawn(Box::new(move || this.transcribe_and_paste(recording)));
    }

    fn transcribe_and_paste(&self, recording: Recording) {
        let settings = self.deps.store.settings().unwrap_or_default();
        let request = TranscribeRequest {
            base_url: settings.base_url,
            api_key: self.deps.secrets.api_key().unwrap_or_default(),
            model: settings.model,
            languages: settings.languages,
            keywords: settings.keywords,
            wav: encode_wav(
                &recording.samples,
                recording.sample_rate,
                recording.channels,
            ),
        };
        match self.deps.transcriber.transcribe(request) {
            Ok(text) if text.trim().is_empty() => self.set_status(Status::Idle),
            Ok(text) => {
                let text = text.trim();
                if self.deps.store.add_history(text).is_ok() {
                    self.deps.feedback.history_changed();
                }
                match self.deps.paster.paste(text) {
                    Ok(()) => self.set_status(Status::Idle),
                    Err(error) => self.set_status(Status::Error(error)),
                }
            }
            Err(error) => self.set_status(Status::Error(error.to_string())),
        }
    }
}
