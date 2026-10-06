use std::sync::{Arc, Mutex};
use std::time::Duration;

use yap_lib::dictation::{
    Cue, Deps, Dictation, Feedback, Paster, Recorder, Recording, Secrets, ShortcutEvent::*,
    Spawner, Status, Timer,
};
use yap_lib::store::{Mode, Settings, Store};
use yap_lib::transcriber::{TranscribeError, TranscribeRequest, Transcriber, Transcription};

struct FakeRecorder {
    next: Mutex<Recording>,
    opened: Mutex<Vec<Option<String>>>,
}

impl Recorder for FakeRecorder {
    fn start(&self, device: Option<&str>) -> Result<(), String> {
        self.opened.lock().unwrap().push(device.map(String::from));
        Ok(())
    }

    fn stop(&self) -> Recording {
        self.next.lock().unwrap().clone()
    }
}

struct SentRequest {
    base_url: String,
    api_key: String,
    model: String,
    languages: Vec<String>,
    audio_len: usize,
}

struct FakeTranscriber {
    reply: Mutex<Result<Transcription, TranscribeError>>,
    sent: Mutex<Vec<SentRequest>>,
}

impl Transcriber for FakeTranscriber {
    fn transcribe(&self, request: TranscribeRequest) -> Result<Transcription, TranscribeError> {
        self.sent.lock().unwrap().push(SentRequest {
            base_url: request.base_url,
            api_key: request.api_key,
            model: request.model,
            languages: request.languages,
            audio_len: request.audio.len(),
        });
        self.reply.lock().unwrap().clone()
    }
}

#[derive(Default)]
struct FakePaster {
    pasted: Mutex<Vec<String>>,
    failure: Mutex<Option<String>>,
}

impl Paster for FakePaster {
    fn paste(&self, text: &str) -> Result<(), String> {
        if let Some(failure) = self.failure.lock().unwrap().clone() {
            return Err(failure);
        }
        self.pasted.lock().unwrap().push(text.to_string());
        Ok(())
    }
}

#[derive(Default)]
struct FakeFeedback {
    cues: Mutex<Vec<Cue>>,
}

impl Feedback for FakeFeedback {
    fn status(&self, _status: &Status) {}

    fn cue(&self, cue: Cue) {
        self.cues.lock().unwrap().push(cue);
    }

    fn history_changed(&self) {}
}

struct FakeSecrets(Option<String>);

impl Secrets for FakeSecrets {
    fn api_key(&self) -> Option<String> {
        self.0.clone()
    }
}

#[derive(Default)]
struct QueuedSpawner {
    jobs: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

impl Spawner for QueuedSpawner {
    fn spawn(&self, job: Box<dyn FnOnce() + Send>) {
        self.jobs.lock().unwrap().push(job);
    }
}

type Job = Box<dyn FnOnce() + Send>;

#[derive(Default)]
struct FakeTimer {
    pending: Mutex<Vec<(Duration, Option<Job>)>>,
}

impl Timer for FakeTimer {
    fn after(&self, delay: Duration, job: Job) {
        self.pending.lock().unwrap().push((delay, Some(job)));
    }
}

struct Harness {
    dictation: Dictation,
    store: Arc<Store>,
    recorder: Arc<FakeRecorder>,
    transcriber: Arc<FakeTranscriber>,
    paster: Arc<FakePaster>,
    feedback: Arc<FakeFeedback>,
    spawner: Arc<QueuedSpawner>,
    timer: Arc<FakeTimer>,
}

impl Harness {
    fn new(settings: Settings, api_key: Option<&str>) -> Self {
        let store = Arc::new(Store::open_in_memory().unwrap());
        store.save_settings(&settings).unwrap();
        let recorder = Arc::new(FakeRecorder {
            next: Mutex::new(speech()),
            opened: Mutex::default(),
        });
        let transcriber = Arc::new(FakeTranscriber {
            reply: Mutex::new(Ok(transcript("hello world"))),
            sent: Mutex::default(),
        });
        let paster = Arc::new(FakePaster::default());
        let feedback = Arc::new(FakeFeedback::default());
        let spawner = Arc::new(QueuedSpawner::default());
        let timer = Arc::new(FakeTimer::default());
        let dictation = Dictation::new(Deps {
            store: store.clone(),
            recorder: recorder.clone(),
            transcriber: transcriber.clone(),
            paster: paster.clone(),
            feedback: feedback.clone(),
            secrets: Arc::new(FakeSecrets(api_key.map(String::from))),
            spawner: spawner.clone(),
            timer: timer.clone(),
        });
        Self {
            dictation,
            store,
            recorder,
            transcriber,
            paster,
            feedback,
            spawner,
            timer,
        }
    }

    fn hold() -> Self {
        Self::new(Settings::default(), Some("sk-test"))
    }

    fn run_background_jobs(&self) {
        let jobs: Vec<_> = self.spawner.jobs.lock().unwrap().drain(..).collect();
        for job in jobs {
            job();
        }
    }

    fn timer_delays(&self) -> Vec<Duration> {
        self.timer
            .pending
            .lock()
            .unwrap()
            .iter()
            .map(|(delay, _)| *delay)
            .collect()
    }

    fn fire_timer(&self, index: usize) {
        let job = self.timer.pending.lock().unwrap()[index].1.take().unwrap();
        job();
    }

    fn pasted(&self) -> Vec<String> {
        self.paster.pasted.lock().unwrap().clone()
    }
}

fn transcript(text: &str) -> Transcription {
    Transcription {
        text: text.into(),
        cost: None,
    }
}

fn speech() -> Recording {
    Recording {
        samples: vec![0.3; 16_000],
        sample_rate: 16_000,
        channels: 1,
    }
}

#[test]
fn hold_mode_pastes_transcript_after_release() {
    let h = Harness::hold();

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    assert_eq!(h.pasted(), ["hello world"]);
}

#[test]
fn transcript_is_saved_to_history() {
    let h = Harness::hold();

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    let texts: Vec<String> = h
        .store
        .history()
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect();
    assert_eq!(texts, ["hello world"]);
}

#[test]
fn transcription_cost_is_saved_with_the_history_entry() {
    let h = Harness::hold();
    *h.transcriber.reply.lock().unwrap() = Ok(Transcription {
        text: "hello world".into(),
        cost: Some(0.0021),
    });

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    let costs: Vec<Option<f64>> = h
        .store
        .history()
        .unwrap()
        .into_iter()
        .map(|e| e.cost)
        .collect();
    assert_eq!(costs, [Some(0.0021)]);
}

#[test]
fn history_entry_records_duration_and_uploaded_size() {
    let h = Harness::hold();
    *h.recorder.next.lock().unwrap() = Recording {
        samples: vec![0.3; 48_000 * 2 * 3 / 2],
        sample_rate: 48_000,
        channels: 2,
    };

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    let uploaded = h.transcriber.sent.lock().unwrap()[0].audio_len as i64;
    let entry = h.store.history().unwrap().remove(0);
    assert_eq!((entry.duration, entry.size), (Some(1.5), Some(uploaded)));
}

#[test]
fn toggle_mode_keeps_recording_after_release_and_finishes_on_second_press() {
    let h = Harness::new(
        Settings {
            mode: Mode::Toggle,
            ..Settings::default()
        },
        Some("sk-test"),
    );

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    assert_eq!(h.dictation.status(), Status::Recording);

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();
    assert_eq!(h.pasted(), ["hello world"]);
}

#[test]
fn failed_transcription_reports_error_and_next_press_records_again() {
    let h = Harness::hold();
    *h.transcriber.reply.lock().unwrap() = Err(TranscribeError::Http {
        status: 401,
        message: "Incorrect API key provided".into(),
    });

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();
    assert_eq!(
        (h.dictation.status(), h.pasted()),
        (
            Status::Error("HTTP 401: Incorrect API key provided".into()),
            vec![]
        )
    );

    h.dictation.handle(Pressed);
    assert_eq!(h.dictation.status(), Status::Recording);
}

#[test]
fn accidental_tap_is_discarded_without_calling_the_api() {
    let h = Harness::hold();
    *h.recorder.next.lock().unwrap() = Recording {
        samples: vec![0.3; 1_600],
        sample_rate: 16_000,
        channels: 1,
    };

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    assert_eq!(
        (
            h.dictation.status(),
            h.transcriber.sent.lock().unwrap().len()
        ),
        (Status::Idle, 0)
    );
}

#[test]
fn silent_recording_is_discarded_without_calling_the_api() {
    let h = Harness::hold();
    *h.recorder.next.lock().unwrap() = Recording {
        samples: vec![0.001; 16_000],
        sample_rate: 16_000,
        channels: 1,
    };

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    assert_eq!(
        (
            h.dictation.status(),
            h.transcriber.sent.lock().unwrap().len()
        ),
        (Status::Idle, 0)
    );
}

#[test]
fn pressing_without_api_key_reports_error_instead_of_recording() {
    let h = Harness::new(Settings::default(), None);

    h.dictation.handle(Pressed);

    assert_eq!(
        h.dictation.status(),
        Status::Error("API key is not set. Open Settings to add it.".into())
    );
}

#[test]
fn surrounding_whitespace_is_trimmed_before_pasting() {
    let h = Harness::hold();
    *h.transcriber.reply.lock().unwrap() = Ok(transcript(" hello world\n"));

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    assert_eq!(h.pasted(), ["hello world"]);
}

#[test]
fn blank_transcript_pastes_nothing_and_skips_history() {
    let h = Harness::hold();
    *h.transcriber.reply.lock().unwrap() = Ok(transcript(" \n"));

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    assert_eq!(
        (
            h.pasted().len(),
            h.store.history().unwrap().len(),
            h.dictation.status()
        ),
        (0, 0, Status::Idle)
    );
}

#[test]
fn failed_paste_reports_error_but_keeps_transcript_in_history() {
    let h = Harness::hold();
    *h.paster.failure.lock().unwrap() = Some("Accessibility permission missing".into());

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    let texts: Vec<String> = h
        .store
        .history()
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect();
    assert_eq!(
        (h.dictation.status(), texts),
        (
            Status::Error("Accessibility permission missing".into()),
            vec!["hello world".to_string()]
        )
    );
}

#[test]
fn start_and_stop_cues_frame_the_recording() {
    let h = Harness::hold();

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);

    assert_eq!(*h.feedback.cues.lock().unwrap(), [Cue::Start, Cue::Stop]);
}

#[test]
fn disabled_sounds_play_no_cues() {
    let h = Harness::new(
        Settings {
            sounds: false,
            ..Settings::default()
        },
        Some("sk-test"),
    );

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);

    assert_eq!(*h.feedback.cues.lock().unwrap(), []);
}

#[test]
fn request_uses_configured_endpoint_model_languages_and_stored_key() {
    let h = Harness::new(
        Settings {
            base_url: "https://api.groq.com/openai/v1".into(),
            model: "whisper-large-v3".into(),
            languages: vec!["sk".into()],
            ..Settings::default()
        },
        Some("gsk-secret"),
    );

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();

    let sent = h.transcriber.sent.lock().unwrap();
    assert_eq!(
        (
            sent[0].base_url.as_str(),
            sent[0].model.as_str(),
            sent[0].languages.clone(),
            sent[0].api_key.as_str()
        ),
        (
            "https://api.groq.com/openai/v1",
            "whisper-large-v3",
            vec!["sk".to_string()],
            "gsk-secret"
        )
    );
}

#[test]
fn press_during_transcription_is_ignored() {
    let h = Harness::hold();

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.dictation.handle(Pressed);
    assert_eq!(h.dictation.status(), Status::Transcribing);

    h.run_background_jobs();
    assert_eq!(h.dictation.status(), Status::Idle);
}

#[test]
fn toggle_starts_and_finishes_recording_even_in_hold_mode() {
    let h = Harness::hold();

    h.dictation.toggle();
    assert_eq!(h.dictation.status(), Status::Recording);

    h.dictation.toggle();
    h.run_background_jobs();
    assert_eq!(h.pasted(), ["hello world"]);
}

#[test]
fn recording_opens_the_configured_input_device() {
    let h = Harness::new(
        Settings {
            input_device: Some("coreaudio:USBMic".into()),
            ..Settings::default()
        },
        Some("sk-test"),
    );

    h.dictation.handle(Pressed);

    assert_eq!(
        *h.recorder.opened.lock().unwrap(),
        [Some("coreaudio:USBMic".to_string())]
    );
}

#[test]
fn recording_stops_and_is_transcribed_at_the_max_length() {
    let h = Harness::new(
        Settings {
            max_minutes: 2,
            ..Settings::default()
        },
        Some("sk-test"),
    );

    h.dictation.handle(Pressed);
    assert_eq!(h.timer_delays(), [Duration::from_secs(120)]);
    h.fire_timer(0);
    h.run_background_jobs();
    h.dictation.handle(Released);

    assert_eq!(h.pasted(), ["hello world"]);
    assert_eq!(h.dictation.status(), Status::Idle);
}

#[test]
fn max_length_of_an_earlier_recording_does_not_stop_the_current_one() {
    let h = Harness::hold();

    h.dictation.handle(Pressed);
    h.dictation.handle(Released);
    h.run_background_jobs();
    h.dictation.handle(Pressed);
    h.fire_timer(0);

    assert_eq!(h.dictation.status(), Status::Recording);
}
