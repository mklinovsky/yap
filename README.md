# yap

Dictation for macOS. Press a global shortcut, speak, and the transcript is pasted at the cursor.

- Works with any OpenAI-compatible `/audio/transcriptions` endpoint (OpenAI, LiteLLM, …)
- Hold or toggle mode, optional start/stop sounds
- Multiple languages and custom vocabulary (`gpt-transcribe`)
- Retry a failed transcription without recording again
- History with duration, upload size and cost (cost via LiteLLM's `x-litellm-response-cost` header)
- Lives in the menu bar, no Dock icon

No LLM cleanup: you get the raw transcript.

## Install

1. Download the `.dmg` from [Releases](../../releases/latest) (universal: Apple Silicon and Intel).
2. Drag `yap.app` to Applications.
3. Open it. yap is not notarized, so macOS blocks the first launch: open System Settings → Privacy & Security and click **Open Anyway** next to the message about yap. Alternatively run `xattr -dr com.apple.quarantine /Applications/yap.app`.
4. Grant **Microphone** (prompted on first recording) and **Accessibility** (System Settings → Privacy & Security, needed to paste).

After updating to a new version, macOS may not recognize the new build: if recording or pasting stops working, remove yap from Microphone and Accessibility and add it again.

## Setup

Settings opens on first launch:

| Setting | Default |
|---|---|
| Base URL | `https://api.openai.com/v1` |
| API key | — (required) |
| Model | `gpt-transcribe` |
| Languages | auto-detect |
| Shortcut | `⌃⌥⇧Space` |
| Mode | Hold |
| Max length | 20 min (the recording then stops and is transcribed) |

## Good to know

- The transcript is pasted through the clipboard; the previous text or image is restored right after (files and rich-text formatting are not).
- Audio is sent only to the configured endpoint. A failed recording is kept in memory for **Retry** (menu bar or Try it) until the next successful transcription or quit.
- Settings, history and the API key (in plain text) are stored locally in `~/Library/Application Support/com.mklinovsky.yap/yap.db`.

## Development

Requires Node 24, pnpm, and stable Rust.

```sh
pnpm install
pnpm tauri dev      # run with hot reload
pnpm test           # Vitest + cargo test
pnpm lint           # ESLint, clippy, rustfmt
pnpm tauri build    # release bundle in src-tauri/target/release/bundle
```

## License

[MIT](LICENSE)
