# yap

Push-to-talk dictation for macOS (Windows/Linux possible later), in the spirit of superwhisper / Whispering but deliberately small: press a global shortcut, speak, the audio goes to an OpenAI-compatible transcription endpoint, the text is pasted at the cursor and saved to history. No LLM cleanup pass. Personal tool; the user may share it.

## Stack

- Tauri 2, Rust backend (`src-tauri/`), React 19 + TypeScript + Vite frontend (`src/`), pnpm.
- Rust crates: `cpal` (mic + sound cues), `rubato` (FFT resampler), `flacenc` (pure-Rust FLAC, default features off), `reqwest` blocking + multipart + rustls (HTTP), `rusqlite` bundled (storage), `enigo` (Cmd/Ctrl+V), `arboard` (clipboard), `tauri-plugin-global-shortcut`, `tauri-plugin-autostart` (open at login), `thiserror`. Dev: `mockito`, `claxon` (FLAC decoding in tests).
- Frontend tests: Vitest + jsdom + Testing Library + user-event, Tauri IPC faked with `mockIPC` from `@tauri-apps/api/mocks`.

## Commands

| What | Command |
|---|---|
| Dev (Vite + cargo, hot reload) | `pnpm tauri dev` |
| All tests (Vitest, then cargo test) | `pnpm test` |
| Lint (ESLint, clippy `-D warnings`, rustfmt check) | `pnpm lint` |
| Typecheck | `pnpm exec tsc` (also part of `pnpm build`) |
| Full release build + bundle | `pnpm tauri build` (`--no-bundle`, `--bundles app`, `--debug` variants) |

- Typecheck IS allowed and expected here: the user's global "never typecheck" rules apply only to their Angular apps. `pnpm build` is `tsc && vite build` on purpose. A `verify` subagent may refuse `tsc` citing global CLAUDE.md; pass this clarification in its prompt.
- Do not run `pnpm tauri build` at the end of a change unless asked; lint + tests (+ tsc) is the finish line.
- Run `cargo fmt` (in `src-tauri/`) after Rust edits, otherwise `pnpm lint` fails on the fmt check.
- The first DMG bundling attempt once failed with a generic `bundle_dmg.sh` error and passed on rerun; cause unknown. A leftover `rw.*.dmg` in `target/release/bundle/macos/` can be deleted.
- CI (`.github/workflows/ci.yml`): `pnpm build` (must precede cargo: `generate_context!` needs `dist/`), lint, test on macOS.
- Release: bump the version in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, then push a `v*` tag; `.github/workflows/release.yml` builds a universal DMG into a draft GitHub release.

## Working agreements

- TDD, vertical slices: one test, watch it fail for the right reason (assertion or `todo!()` panic, not a compile/import error), minimal code, green. Tests only at agreed seams (below). If a test passes on first run because earlier code already covers it, say so.
- Ask one question at a time when aligning; give a recommended option. Don't silently pick on decisions with rework cost.
- Nothing is committed yet (single `init` commit on `main`). Commit only when asked; branch names never use a `claude/` prefix.
- The app has never been run by Claude: everything visual or platform-level (vibrancy, tray, focus, mic, paste, sounds) is unverified unless the user reports on it.

## Architecture

```
global shortcut ──► mpsc worker thread ──► Dictation (state machine)
Try it button ──► toggle_recording ─┘          │
                                               ├─ Recorder (cpal)         start/stop → Recording
                                               ├─ encode_flac             16 kHz mono 16-bit FLAC
                                               ├─ Transcriber (reqwest)   POST {base}/audio/transcriptions
                                               ├─ Store (SQLite)          settings, history, API key
                                               ├─ Paster (arboard+enigo)  clipboard + Cmd/Ctrl+V
                                               └─ Feedback (tray)         icon, status text, sounds, events
```

Backend (`src-tauri/src/`):
- `dictation.rs` — the core. `Dictation` holds `Deps` (all boundaries as `Arc<dyn Trait>`: `Recorder`, `Transcriber`, `Paster`, `Feedback`, `Secrets`, `Spawner`, `Timer`, plus `Arc<Store>`). `handle(ShortcutEvent)` for hold/toggle modes, `toggle()` for the button (mode-independent), `status()`. Status: `Idle | Recording | Transcribing | Error(String)`, serialized as `{"state": "...", "message"?: "..."}`.
  - No API key → `Error("API key is not set. Open Settings to add it.")` on press, nothing recorded.
  - Recording < 0.3 s or peak < 0.01 → discarded, back to Idle, no API call.
  - Transcript is trimmed; blank → Idle, nothing pasted, no history.
  - History is saved before pasting, so a paste failure (`Error`) still keeps the text. The transcription cost, recording duration (s) and uploaded FLAC size (bytes) are stored with the entry; blank transcripts are billed but not recorded.
  - Press while Transcribing is ignored; Error behaves like Idle on the next press.
  - Each start schedules `Timer::after(maxMinutes)`; when it fires (on the worker thread) and that same recording is still running, it finishes like a manual stop. A per-recording counter makes timers of earlier recordings no-ops. A Hold release arriving after the auto-stop is ignored.
  - Cues (`Cue::Start/Stop`) only when `settings.sounds`.
  - Transcription runs via `Spawner` (threads in the app, a queue in tests).
- `store.rs` — `Store` (Mutex<Connection>), tables `settings` (one JSON row), `history` (`id, text, created_at, cost REAL, duration REAL, size INTEGER`, the last three nullable; `migrate()` adds missing columns to older DBs). `add_history(text, cost, duration, size)`, `secrets`. `Settings { baseUrl, model, languages, keywords, inputDevice, shortcut, mode, sounds, maxMinutes }` camelCase JSON. Defaults: `https://api.openai.com/v1`, `gpt-transcribe`, no languages (auto-detect), no keywords, system default input (`None`), `Alt+Space`, Hold, sounds on, max length 5 min (`#[serde(default)]`, so older rows load as 5). `upgrade()` migrates the legacy single `language` field to `languages`. `keywords` has `#[serde(default)]`. API key: `api_key()`, `set_api_key("")` deletes, `api_key_preview()` masks (`sk-p••••••••`, 2 visible chars if key ≤ 12). `Store` implements `dictation::Secrets` (in `lib.rs`).
- `transcriber.rs` — `HttpTranscriber`. Multipart `model`, `response_format=json` (otherwise LiteLLM routes non-`gpt-4o` OpenAI models through its Whisper config, which fills in `verbose_json`; `gpt-transcribe` rejects that), `file` (`audio.flac`, `audio/flac`). Model-specific: if model starts with `gpt-transcribe` → every language as `languages[]` and every keyword as `keywords[]`; otherwise only the first language as `language`, no keywords. Returns `Transcription { text, cost: Option<f64> }`; cost (USD) comes from LiteLLM's `x-litellm-response-cost` header, `None` when absent/unparsable (e.g. OpenAI direct). Errors: `Http { status, message }` (message from `{"error":{"message"}}` or raw body), `InvalidResponse`, `Network`. Trailing slash on base URL is trimmed. Client timeout is 15 min (reqwest's 30 s default would kill long recordings).
- `audio.rs` — `encode_flac(samples, rate, channels)`: downmix by averaging, band-limited resample to 16 kHz (`rubato::Fft`, `process_all`; the old linear interpolation aliased everything above 8 kHz into the speech band), clamp, ×32767 rounded, FLAC 16-bit. Roughly half the size of the equivalent WAV; 60 s of 48 kHz input encodes in ~30 ms (release). FLAC can't encode streams shorter than 16 samples (irrelevant past the 0.3 s gate). Chosen over native-rate upload (bigger, no proven accuracy gain) and Opus (lossy, C dependency).
- `recorder.rs` — `CpalRecorder`. `Recorder::start(device)` gets the saved cpal `DeviceId` string (stable across runs; display names can repeat); unknown/unplugged id or `None` → system default input. `input_devices()` lists `{id, name}` for the Settings picker. The cpal stream lives on its own thread (not `Send` everywhere); start waits for a ready signal so device errors surface synchronously. Handles F32/I16/I32/U16.
- `paster.rs` — `ClipboardPaster`: sets clipboard (keeps one `arboard::Clipboard` alive — X11/Wayland serve clipboard from the owner), then Cmd+V (macOS) / Ctrl+V via enigo **on the main thread** (`run_on_main_thread`; enigo's keyboard-layout lookups crash off-main on macOS). Afterwards the previous clipboard is restored (user asked for it, after first choosing not to): saved as text or image before pasting, put back 300 ms after Cmd+V (the target app reads the clipboard asynchronously), skipped if something else was copied meanwhile; other formats (e.g. copied files) can't be read, so the transcript stays. `copy_text` uses the same clipboard without restoring.
- `tray.rs` — `TrayFeedback`: icon is generated in code: template dot idle, red dot recording, amber dot transcribing, template warning triangle error. Red means recording only (user found a red error ring confusing), tooltip + disabled status menu item, emits `status-changed` and `history-changed`. Cues are synthesized sine beeps through cpal output (880 Hz start, 660 Hz stop) — `rodio` was dropped to avoid a second audio stack.
- `lib.rs` — wiring: commands `get_settings, save_settings, api_key_preview, set_api_key, list_history, delete_history, copy_text, list_input_devices, get_status, toggle_recording, pause_shortcut, resume_shortcut, get_open_at_login, set_open_at_login`; open at login = autostart plugin with the macOS LaunchAgent launcher (`~/Library/LaunchAgents`, no System Events permission prompt), state lives only in the OS, not in SQLite; one worker thread consumes `Input::{Shortcut, Toggle, Run}` so press/release stay ordered off the main thread; `WorkerTimer` sleeps on its own thread and posts `Input::Run(job)` back to the worker; tray menu = status line, "Open yap…", "Quit yap"; single window `main` (780×800, tall enough for Settings without scrolling, min 640×420, dark theme, macOS overlay title bar + hidden title + transparent + Sidebar vibrancy); `open_window` always calls `set_focus()`; app is `ActivationPolicy::Accessory` (tray only, no Dock); `ExitRequested { code: None }` is prevented so closing the window doesn't quit; Settings window opens on launch when no API key. `save_settings` re-registers the shortcut only when it changed and restores the old one if the new one is refused.

Frontend (`src/`):
- `api.ts` — typed `invoke` wrappers, `Settings`/`HistoryEntry`/`Status` types, event names `history-changed`, `status-changed`.
- `App.tsx` — sidebar shell, order **Settings, History, Try it**, opens on Settings (`main.tsx`). `main.tsx` adds `html.mac` for mac-only styling (transparent body + sidebar padding for traffic lights).
- `Settings.tsx` — grouped rows (Transcription: Base URL, API key, Model, Languages, Vocabulary; Dictation: Input device select — "System default" first, then devices, "Disconnected device" if the saved one is missing — Shortcut, Mode segmented, Max length select (1, 2, 5, 10, 15, 20 min; OpenAI's 25 MB upload limit is roughly 20 min of FLAC), Sounds switch, Open at login switch — loaded from `get_open_at_login`, sent with `set_open_at_login` on Save only when changed), explicit Save button (user rejected autosave) in a bar pinned below the scrolling groups (Settings scrolls its own `.settings-scroll`; `.content-body[data-section="settings"]` doesn't scroll). Save is disabled until the form differs from the last saved snapshot (incl. a typed API key); bar shows "Unsaved changes" / "Saved" / the backend error, and a failed save stays dirty so it can be retried. API key field is always empty; placeholder = masked preview; eye button only appears while typing a new key. Vocabulary is comma-separated text → trimmed `keywords` array on save.
- `LanguagePicker.tsx` — chips with remove buttons + "Add language…" select; empty = "Auto-detect". `languages.ts` pins `en` first.
- `ShortcutRecorder.tsx` — click → pauses global shortcut, captures the next combo **on `window` (capture phase)**, Esc / outside click / window blur cancels, resume on stop. Stores `Ctrl+Alt+Shift+Super` + key from `event.code` (`KeyK`→`K`, `Digit1`→`1`, else the code, e.g. `Space`). Requires a modifier unless F-key. Displays ⌃⌥⇧⌘ on mac.
- `History.tsx` — cards, hover copy/delete icon buttons, empty state, reloads on `history-changed`. Per card: `12.4 s · 205 KB` (≥ 60 s as `1:15`, ≥ 1 MB as `2.4 MB`, decimal units) when known (older entries have none), cost (`$0.0012`, 4 decimals) when known; "Total $…" above the list = sum of listed entries with a cost (deleted entries drop out), hidden when none has one.
- `TryIt.tsx` — scratch pad textarea + Start/Stop button driven by `status-changed` (mic icon idle, pulsing red dot recording, spinner transcribing; button never wraps; errors in a full-width callout below); focuses the textarea before toggling so the real Cmd+V paste lands there (tests the full pipeline).
- `.app` grid needs `grid-template-rows: minmax(0, 1fr)` (and `.content { min-height: 0 }`), otherwise the row grows with content and inner scroll areas/sticky elements stop working.
- `styles.css` — dark only (user: "dark is enough"), system font, `-apple-system-control-accent` with fallback, all text controls and selects fixed at 28px height.

## Test seams (agreed — test only here)

1. `Store` against real in-memory/temp-file SQLite (`tests/store.rs`).
2. `Transcriber` against a mockito server (`tests/transcriber.rs`; `capture_body` helper for multipart assertions).
3. `encode_flac` decoded via claxon (`tests/audio.rs`): hand-computed literals at 16 kHz (no resampling), sample count for 44.1 kHz, sine level kept at 1 kHz and a 12 kHz tone filtered out (anti-aliasing).
4. `Dictation` with fakes only at boundaries, real in-memory Store, `QueuedSpawner` + `run_background_jobs()` (`tests/dictation.rs`).
5. React components through `mockIPC` (+ `shouldMockEvents: true` and `emit` for events): `App`, `Settings`, `History`, `TryIt`; `ShortcutRecorder` and `LanguagePicker` via Settings.

Not tested (thin adapters, verify by running): cpal recorder, paster, tray/sounds, window/tray wiring in `lib.rs`.

## Learnings / gotchas

- **WebKit doesn't focus a `<button>` on click** (Safari/WKWebView). Key handlers on the button never fire in the app while jsdom tests pass. Capture keys on `window`; reproduce in tests with `fireEvent.click` (no focus) instead of `user.click`.
- **WebKit ignores padding/height on native `<select>`** unless `appearance: none`; give inputs and selects the same explicit height and draw the chevron as a background SVG.
- **user-event key syntax:** `[KeyK]` presses a physical code; `{KeyK}` is a key *name* and won't set `code`.
- **Tauri mocks teardown:** unmount cleanup calls `unlisten` asynchronously; `clearMocks()` must wait a tick (`src/test-setup.ts`) or `unregisterListener is not a function` errors appear after passing tests.
- `listitem` has no accessible name from content; use `aria-label` when querying chips by role+name.
- **Accessory (tray-only) apps aren't activated when a window is created**; the window gets made key but keyboard focus stays in the previous app (no caret). `set_focus()` after creating/showing is the fix — believed, not yet confirmed by the user.
- **Ad-hoc signing changes the code signature every rebuild**, so macOS re-prompts Keychain access and may drop Microphone/Accessibility grants. This is why the keychain was removed (key now plaintext in SQLite, user's explicit choice). Stable alternative if ever needed: self-signed code-signing cert. Sharing with other Macs needs notarization (paid account) or users clearing quarantine; Windows/Linux sharing was the reason for Tauri over Swift.
- **OpenAI transcription (checked Oct 2026):** `gpt-transcribe` is the recommended model; it takes `languages[]` (never send `language` too), `keywords[]`, `prompt`; response `{text, languages}`. `gpt-4o-transcribe`/`gpt-4o-mini-transcribe` and `whisper-1` (legacy) use singular `language`; `whisper-1` prompt ≤ 224 tokens. Max file 25 MB. Docs: https://developers.openai.com/api/docs/guides/speech-to-text
- **macOS shortcut conflicts:** ⌘Space Spotlight, ⌥⌘Space Finder, ⌃Space / ⌃⌥Space input sources (swallowed before the app sees them), ⌃⌘Space emoji. ⌥Space is Alfred/Raycast's default. On non-US layouts avoid ⌥+letter/digit (types characters). Recommended: ⌃⌥⇧Space.
- `tauri::WebviewUrl::App("index.html?view=…")` joins correctly, but the app now opens plain `index.html`.
- Spotlight indexes `src-tauri/target/release/bundle/macos/yap.app` as a second "yap"; `touch src-tauri/target/.metadata_never_index` hides it. Two running copies fight over the global shortcut.
- App data: `~/Library/Application Support/com.mklinovsky.yap/yap.db`. A stale Keychain item (service `com.mklinovsky.yap`, account `api-key`) may remain from the keychain era.

## Open / unverified

- Never launched by Claude: vibrancy look, cursor/focus fix, tray icon rendering, sound cues, paste into other apps and into Try it.
- macOS needs Microphone (Info.plist `NSMicrophoneUsageDescription`) and Accessibility (for paste) permissions; both may reset after rebuilds.
- Mode Hold vs Toggle with key repeat hasn't been observed.
- Known, parked by the user: clicking Start in Try it makes the window fall behind other apps (yap gets deactivated when recording starts; trigger unknown). Options discussed: re-focus the window after a Try it start, or log activation changes to find the cause.
