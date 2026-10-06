# <img src="src-tauri/icons/128x128@2x.png" width="56" height="56" align="absmiddle" alt=""> yap

Push-to-talk dictation for macOS. Press a global shortcut, speak, and the transcript is pasted at the cursor.

- Works with any OpenAI-compatible `/audio/transcriptions` endpoint (OpenAI, LiteLLM, …)
- Hold or toggle mode, optional start/stop sounds
- Recordings stop and are transcribed at a max length
- Restores your previous clipboard after pasting
- Optional open at login
- Multiple languages and custom vocabulary (`gpt-transcribe`)
- Optional transformations: an LLM rewrites the transcript before pasting (any OpenAI-compatible `/chat/completions` endpoint)
- History with duration, cost and details (cost via LiteLLM's `x-litellm-response-cost` header)
- Lives in the menu bar, no Dock icon

Without a transformation you get the raw transcript.

## Install

1. Download the `.dmg` from [Releases](../../releases/latest) (universal: Apple Silicon and Intel).
2. Drag `yap.app` to Applications.
3. yap is not notarized, so macOS blocks the first launch ("Apple could not verify…"). Clear the download flag (or use System Settings → Privacy & Security → Open Anyway):
   ```sh
   xattr -dr com.apple.quarantine /Applications/yap.app
   ```
4. Grant **Microphone** (prompted on first recording) and **Accessibility** (System Settings → Privacy & Security, needed to paste).

## Setup

Settings opens on first launch:

| Setting | Default |
|---|---|
| Base URL | `https://api.openai.com/v1` |
| API key | — (required) |
| Model | `gpt-transcribe` |
| Languages | auto-detect |
| Input | system default |
| Shortcut | `⌘⇧;` |
| Mode | Hold |
| Max length | 5 min |
| Sounds | on |
| Open at login | off |

## Transformations

A transformation is a named prompt applied to the transcript before it is pasted: fix grammar, translate, turn notes into bullet points. Set them up in the Transformations section:

- Turn on **Enabled**. Base URL and API key default to the transcription ones; a separate endpoint, key and default model can be set.
- Each transformation has a system prompt, a user message (`{{transcript}}` marks where the transcript goes; empty sends the transcript alone), an optional model and an optional shortcut.
- A transformation's shortcut dictates and applies it. The regular shortcut applies the transformation checked in the menu bar menu; with **Use once** checked, the menu goes back to None after one dictation.
- If the transformation fails, nothing is pasted; the raw transcript is in History.

With transformations on, transcripts are also sent to the transformation endpoint.

Data, including the API keys in plain text, is stored in `~/Library/Application Support/com.mklinovsky.yap/yap.db`.

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
