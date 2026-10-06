use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use yap_lib::dictation::{
    Apply, Apply::TrayPick, Clock, Cue, Deps, Dictation, Feedback, Paster, Recorder, Recording,
    Secrets, ShortcutEvent::*, Spawner, Status, Timer,
};
use yap_lib::http::ApiError;
use yap_lib::store::{Mode, Settings, Store, Transformation, Transformations, TrayState};
use yap_lib::transcriber::{TranscribeRequest, Transcriber, Transcription};
use yap_lib::transformer::{TransformRequest, Transformed, Transformer};

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

struct FakeClock {
    start: Instant,
    elapsed: Mutex<Duration>,
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.start + *self.elapsed.lock().unwrap()
    }
}

struct FakeTranscriber {
    reply: Mutex<Result<Transcription, ApiError>>,
    sent: Mutex<Vec<SentRequest>>,
    clock: Arc<FakeClock>,
    takes: Mutex<Duration>,
}

impl Transcriber for FakeTranscriber {
    fn transcribe(&self, request: TranscribeRequest) -> Result<Transcription, ApiError> {
        self.sent.lock().unwrap().push(SentRequest {
            base_url: request.base_url,
            api_key: request.api_key,
            model: request.model,
            languages: request.languages,
            audio_len: request.audio.len(),
        });
        *self.clock.elapsed.lock().unwrap() += *self.takes.lock().unwrap();
        self.reply.lock().unwrap().clone()
    }
}

struct SentTransform {
    base_url: String,
    api_key: String,
    model: String,
    system_prompt: String,
    user_message: String,
}

struct FakeTransformer {
    reply: Mutex<Result<Transformed, ApiError>>,
    sent: Mutex<Vec<SentTransform>>,
    clock: Arc<FakeClock>,
    takes: Mutex<Duration>,
}

impl Transformer for FakeTransformer {
    fn transform(&self, request: TransformRequest) -> Result<Transformed, ApiError> {
        self.sent.lock().unwrap().push(SentTransform {
            base_url: request.base_url,
            api_key: request.api_key,
            model: request.model,
            system_prompt: request.system_prompt,
            user_message: request.user_message,
        });
        *self.clock.elapsed.lock().unwrap() += *self.takes.lock().unwrap();
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
    statuses: Mutex<Vec<Status>>,
    trays: Mutex<Vec<TrayState>>,
}

impl Feedback for FakeFeedback {
    fn status(&self, status: &Status) {
        self.statuses.lock().unwrap().push(status.clone());
    }

    fn cue(&self, cue: Cue) {
        self.cues.lock().unwrap().push(cue);
    }

    fn history_changed(&self) {}

    fn tray(&self, state: &TrayState) {
        self.trays.lock().unwrap().push(state.clone());
    }
}

struct FakeSecrets {
    api_key: Option<String>,
    transform_api_key: Mutex<Option<String>>,
}

impl Secrets for FakeSecrets {
    fn api_key(&self) -> Option<String> {
        self.api_key.clone()
    }

    fn transform_api_key(&self) -> Option<String> {
        self.transform_api_key.lock().unwrap().clone()
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
    transformer: Arc<FakeTransformer>,
    secrets: Arc<FakeSecrets>,
    paster: Arc<FakePaster>,
    feedback: Arc<FakeFeedback>,
    spawner: Arc<QueuedSpawner>,
    timer: Arc<FakeTimer>,
}

impl Harness {
    fn new(settings: Settings, api_key: Option<&str>) -> Self {
        let store = Arc::new(Store::open_in_memory().unwrap());
        store.save_settings(&settings).unwrap();
        Self::on_store(store, api_key)
    }

    fn with_fix_grammar_and_translate() -> Self {
        let store = Arc::new(Store::open_in_memory().unwrap());
        store.save_transformations(&transformations()).unwrap();
        Self::on_store(store, Some("sk-test"))
    }

    fn on_store(store: Arc<Store>, api_key: Option<&str>) -> Self {
        let recorder = Arc::new(FakeRecorder {
            next: Mutex::new(speech()),
            opened: Mutex::default(),
        });
        let clock = Arc::new(FakeClock {
            start: Instant::now(),
            elapsed: Mutex::default(),
        });
        let transcriber = Arc::new(FakeTranscriber {
            reply: Mutex::new(Ok(transcript("hello world"))),
            sent: Mutex::default(),
            clock: clock.clone(),
            takes: Mutex::default(),
        });
        let transformer = Arc::new(FakeTransformer {
            reply: Mutex::new(Ok(Transformed {
                text: "Hello, world.".into(),
                cost_in_usd: None,
            })),
            sent: Mutex::default(),
            clock: clock.clone(),
            takes: Mutex::default(),
        });
        let secrets = Arc::new(FakeSecrets {
            api_key: api_key.map(String::from),
            transform_api_key: Mutex::default(),
        });
        let paster = Arc::new(FakePaster::default());
        let feedback = Arc::new(FakeFeedback::default());
        let spawner = Arc::new(QueuedSpawner::default());
        let timer = Arc::new(FakeTimer::default());
        let dictation = Dictation::new(Deps {
            store: store.clone(),
            recorder: recorder.clone(),
            transcriber: transcriber.clone(),
            transformer: transformer.clone(),
            paster: paster.clone(),
            feedback: feedback.clone(),
            secrets: secrets.clone(),
            spawner: spawner.clone(),
            timer: timer.clone(),
            clock,
        });
        Self {
            dictation,
            store,
            recorder,
            transcriber,
            transformer,
            secrets,
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

    fn dictate(&self, apply: Apply) {
        self.dictation.handle(Pressed(apply));
        self.dictation.handle(Released);
        self.run_background_jobs();
    }

    fn transformed(&self) -> usize {
        self.transformer.sent.lock().unwrap().len()
    }
}

fn transformations() -> Transformations {
    Transformations {
        enabled: true,
        items: vec![
            Transformation {
                id: "t1".into(),
                name: "Fix grammar".into(),
                system_prompt: "Fix grammar.".into(),
                user_template: "Text: {{transcript}}".into(),
                model: None,
                shortcut: None,
            },
            Transformation {
                id: "t2".into(),
                name: "Translate".into(),
                system_prompt: "Translate to English.".into(),
                user_template: String::new(),
                model: Some("gpt-5".into()),
                shortcut: Some("Ctrl+Alt+E".into()),
            },
        ],
        ..Transformations::default()
    }
}

fn transcript(text: &str) -> Transcription {
    Transcription {
        text: text.into(),
        cost_in_usd: None,
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

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.run_background_jobs();

    assert_eq!(h.pasted(), ["hello world"]);
}

#[test]
fn transcript_is_saved_to_history() {
    let h = Harness::hold();

    h.dictation.handle(Pressed(TrayPick));
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
        cost_in_usd: Some(0.0021),
    });

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.run_background_jobs();

    let costs: Vec<Option<f64>> = h
        .store
        .history()
        .unwrap()
        .into_iter()
        .map(|e| e.cost_in_usd)
        .collect();
    assert_eq!(costs, [Some(0.0021)]);
}

#[test]
fn history_entry_records_how_long_encoding_and_transcription_took() {
    let h = Harness::hold();
    *h.transcriber.takes.lock().unwrap() = Duration::from_millis(4500);

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.run_background_jobs();

    let entry = h.store.history().unwrap().remove(0);
    assert_eq!(
        (
            entry.encode_time_in_seconds,
            entry.transcribe_time_in_seconds
        ),
        (Some(0.0), Some(4.5))
    );
}

#[test]
fn history_entry_records_duration_and_uploaded_size() {
    let h = Harness::hold();
    *h.recorder.next.lock().unwrap() = Recording {
        samples: vec![0.3; 48_000 * 2 * 3 / 2],
        sample_rate: 48_000,
        channels: 2,
    };

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.run_background_jobs();

    let uploaded = h.transcriber.sent.lock().unwrap()[0].audio_len as i64;
    let entry = h.store.history().unwrap().remove(0);
    assert_eq!(
        (entry.duration_in_seconds, entry.size_in_bytes),
        (Some(1.5), Some(uploaded))
    );
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

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    assert_eq!(h.dictation.status(), Status::Recording);

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.run_background_jobs();
    assert_eq!(h.pasted(), ["hello world"]);
}

#[test]
fn failed_transcription_reports_error_and_next_press_records_again() {
    let h = Harness::hold();
    *h.transcriber.reply.lock().unwrap() = Err(ApiError::Http {
        status: 401,
        message: "Incorrect API key provided".into(),
    });

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.run_background_jobs();
    assert_eq!(
        (h.dictation.status(), h.pasted()),
        (
            Status::Error("HTTP 401: Incorrect API key provided".into()),
            vec![]
        )
    );

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));

    assert_eq!(
        h.dictation.status(),
        Status::Error("API key is not set. Open Settings to add it.".into())
    );
}

#[test]
fn surrounding_whitespace_is_trimmed_before_pasting() {
    let h = Harness::hold();
    *h.transcriber.reply.lock().unwrap() = Ok(transcript(" hello world\n"));

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.run_background_jobs();

    assert_eq!(h.pasted(), ["hello world"]);
}

#[test]
fn blank_transcript_pastes_nothing_and_skips_history() {
    let h = Harness::hold();
    *h.transcriber.reply.lock().unwrap() = Ok(transcript(" \n"));

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));

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

    h.dictation.handle(Pressed(TrayPick));
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

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.handle(Released);
    h.run_background_jobs();
    h.dictation.handle(Pressed(TrayPick));
    h.fire_timer(0);

    assert_eq!(h.dictation.status(), Status::Recording);
}

#[test]
fn picked_transformation_is_applied_before_pasting() {
    let h = Harness::with_fix_grammar_and_translate();
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    let sent = h.transformer.sent.lock().unwrap();
    assert_eq!(
        (
            h.pasted(),
            sent[0].system_prompt.as_str(),
            sent[0].user_message.as_str(),
            sent[0].model.as_str()
        ),
        (
            vec!["Hello, world.".to_string()],
            "Fix grammar.",
            "Text: hello world",
            "gpt-6-luna"
        )
    );
}

#[test]
fn nothing_picked_pastes_the_transcript_untouched() {
    let h = Harness::with_fix_grammar_and_translate();

    h.dictate(TrayPick);

    assert_eq!(
        (h.pasted(), h.transformed()),
        (vec!["hello world".to_string()], 0)
    );
}

#[test]
fn transformation_shortcut_applies_its_transformation_without_a_pick() {
    let h = Harness::with_fix_grammar_and_translate();

    h.dictate(Apply::Transformation("t2".into()));

    let sent = h.transformer.sent.lock().unwrap();
    assert_eq!(
        (sent[0].model.as_str(), sent[0].user_message.as_str()),
        ("gpt-5", "hello world")
    );
}

#[test]
fn disabled_transformations_ignore_the_pick() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    store
        .save_transformations(&Transformations {
            enabled: false,
            ..transformations()
        })
        .unwrap();
    let h = Harness::on_store(store, Some("sk-test"));
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    assert_eq!(
        (h.pasted(), h.transformed()),
        (vec!["hello world".to_string()], 0)
    );
}

#[test]
fn transformation_reuses_transcription_endpoint_and_key_by_default() {
    let h = Harness::with_fix_grammar_and_translate();
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    let sent = h.transformer.sent.lock().unwrap();
    assert_eq!(
        (sent[0].base_url.as_str(), sent[0].api_key.as_str()),
        ("https://api.openai.com/v1", "sk-test")
    );
}

#[test]
fn transformation_uses_its_own_endpoint_and_key_when_configured() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    store
        .save_transformations(&Transformations {
            base_url: Some("https://llm.example.com/v1".into()),
            reuse_api_key: false,
            ..transformations()
        })
        .unwrap();
    let h = Harness::on_store(store, Some("sk-test"));
    *h.secrets.transform_api_key.lock().unwrap() = Some("sk-llm".into());
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    let sent = h.transformer.sent.lock().unwrap();
    assert_eq!(
        (sent[0].base_url.as_str(), sent[0].api_key.as_str()),
        ("https://llm.example.com/v1", "sk-llm")
    );
}

#[test]
fn missing_transformation_key_is_reported_on_press_without_recording() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    store
        .save_transformations(&Transformations {
            reuse_api_key: false,
            ..transformations()
        })
        .unwrap();
    let h = Harness::on_store(store, Some("sk-test"));

    h.dictation
        .handle(Pressed(Apply::Transformation("t1".into())));

    assert_eq!(
        (
            h.dictation.status(),
            h.recorder.opened.lock().unwrap().len()
        ),
        (
            Status::Error(
                "Transformation API key is not set. Open Transformations to add it.".into()
            ),
            0
        )
    );
}

#[test]
fn transformed_entry_keeps_raw_text_name_cost_and_time() {
    let h = Harness::with_fix_grammar_and_translate();
    *h.transformer.reply.lock().unwrap() = Ok(Transformed {
        text: " Hello, world.\n".into(),
        cost_in_usd: Some(0.0007),
    });
    *h.transformer.takes.lock().unwrap() = Duration::from_millis(1250);
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    let entry = h.store.history().unwrap().remove(0);
    assert_eq!(
        (
            entry.text.as_str(),
            entry.raw_text.as_deref(),
            entry.transformation_name.as_deref(),
            entry.transform_error,
            entry.transform_cost_in_usd,
            entry.transform_time_in_seconds
        ),
        (
            "Hello, world.",
            Some("hello world"),
            Some("Fix grammar"),
            None,
            Some(0.0007),
            Some(1.25)
        )
    );
}

#[test]
fn status_is_transforming_while_the_transformation_runs() {
    let h = Harness::with_fix_grammar_and_translate();
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    assert_eq!(
        *h.feedback.statuses.lock().unwrap(),
        [
            Status::Recording,
            Status::Transcribing,
            Status::Transforming,
            Status::Idle
        ]
    );
}

#[test]
fn failed_transformation_pastes_nothing_and_keeps_raw_text_in_history() {
    let h = Harness::with_fix_grammar_and_translate();
    *h.transformer.reply.lock().unwrap() = Err(ApiError::Http {
        status: 500,
        message: "boom".into(),
    });
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    let entry = h.store.history().unwrap().remove(0);
    assert_eq!(
        (
            h.dictation.status(),
            h.pasted(),
            entry.text.as_str(),
            entry.raw_text,
            entry.transformation_name.as_deref(),
            entry.transform_error.as_deref()
        ),
        (
            Status::Error("Transformation failed: HTTP 500: boom".into()),
            vec![],
            "hello world",
            None,
            Some("Fix grammar"),
            Some("Transformation failed: HTTP 500: boom")
        )
    );
}

#[test]
fn blank_transformation_answer_is_an_error() {
    let h = Harness::with_fix_grammar_and_translate();
    *h.transformer.reply.lock().unwrap() = Ok(Transformed {
        text: " \n".into(),
        cost_in_usd: None,
    });
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    assert_eq!(
        (h.dictation.status(), h.pasted()),
        (
            Status::Error("Transformation returned no text".into()),
            vec![]
        )
    );
}

#[test]
fn use_once_clears_the_pick_after_a_transformed_dictation() {
    let h = Harness::with_fix_grammar_and_translate();
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);
    h.dictate(TrayPick);

    assert_eq!(
        (
            h.dictation.tray_state().selected,
            h.feedback.trays.lock().unwrap().last().cloned(),
            h.transformed(),
            h.pasted()
        ),
        (
            None,
            Some(TrayState {
                selected: None,
                use_once: true
            }),
            1,
            vec!["Hello, world.".to_string(), "hello world".to_string()]
        )
    );
}

#[test]
fn without_use_once_the_pick_stays_and_is_remembered() {
    let h = Harness::with_fix_grammar_and_translate();
    h.dictation.set_use_once(false);
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);
    h.dictate(TrayPick);

    assert_eq!(
        (h.transformed(), h.store.tray_state().unwrap()),
        (
            2,
            TrayState {
                selected: Some("t1".into()),
                use_once: false
            }
        )
    );
}

#[test]
fn with_use_once_the_pick_is_not_remembered() {
    let h = Harness::with_fix_grammar_and_translate();

    h.dictation.select(Some("t1".into()));

    assert_eq!(h.store.tray_state().unwrap(), TrayState::default());
}

#[test]
fn remembered_pick_is_restored_on_start() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    store.save_transformations(&transformations()).unwrap();
    store
        .save_tray_state(&TrayState {
            selected: Some("t2".into()),
            use_once: false,
        })
        .unwrap();
    let h = Harness::on_store(store, Some("sk-test"));

    h.dictate(TrayPick);

    assert_eq!(h.transformer.sent.lock().unwrap()[0].model, "gpt-5");
}

#[test]
fn blank_transcript_keeps_the_pick() {
    let h = Harness::with_fix_grammar_and_translate();
    *h.transcriber.reply.lock().unwrap() = Ok(transcript(" "));
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    assert_eq!(
        (h.dictation.tray_state().selected, h.transformed()),
        (Some("t1".into()), 0)
    );
}

#[test]
fn failed_transformation_still_uses_up_the_pick() {
    let h = Harness::with_fix_grammar_and_translate();
    *h.transformer.reply.lock().unwrap() = Err(ApiError::Network("offline".into()));
    h.dictation.select(Some("t1".into()));

    h.dictate(TrayPick);

    assert_eq!(h.dictation.tray_state().selected, None);
}

#[test]
fn transformation_shortcut_leaves_the_pick_alone() {
    let h = Harness::with_fix_grammar_and_translate();
    h.dictation.select(Some("t1".into()));

    h.dictate(Apply::Transformation("t2".into()));

    assert_eq!(h.dictation.tray_state().selected, Some("t1".into()));
}

#[test]
fn pick_changed_during_recording_applies_to_that_recording() {
    let h = Harness::with_fix_grammar_and_translate();

    h.dictation.handle(Pressed(TrayPick));
    h.dictation.select(Some("t2".into()));
    h.dictation.handle(Released);
    h.run_background_jobs();

    assert_eq!(h.transformer.sent.lock().unwrap()[0].model, "gpt-5");
}

#[test]
fn recording_started_by_a_transformation_shortcut_keeps_it_when_stopped_by_another() {
    let store = Arc::new(Store::open_in_memory().unwrap());
    store
        .save_settings(&Settings {
            mode: Mode::Toggle,
            ..Settings::default()
        })
        .unwrap();
    store.save_transformations(&transformations()).unwrap();
    let h = Harness::on_store(store, Some("sk-test"));

    h.dictation
        .handle(Pressed(Apply::Transformation("t2".into())));
    h.dictation.handle(Pressed(TrayPick));
    h.run_background_jobs();

    assert_eq!(h.transformer.sent.lock().unwrap()[0].model, "gpt-5");
}

#[test]
fn deleting_the_picked_transformation_clears_the_pick() {
    let h = Harness::with_fix_grammar_and_translate();
    h.dictation.select(Some("t2".into()));
    h.store
        .save_transformations(&Transformations {
            items: transformations().items[..1].to_vec(),
            ..transformations()
        })
        .unwrap();

    h.dictation.clear_pick_if_deleted();

    assert_eq!(h.dictation.tray_state().selected, None);
}

#[test]
fn tray_hears_about_every_pick() {
    let h = Harness::with_fix_grammar_and_translate();

    h.dictation.select(Some("t1".into()));
    h.dictation.set_use_once(false);

    assert_eq!(
        *h.feedback.trays.lock().unwrap(),
        [
            TrayState {
                selected: Some("t1".into()),
                use_once: true
            },
            TrayState {
                selected: Some("t1".into()),
                use_once: false
            }
        ]
    );
}
