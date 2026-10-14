# <img src="src-tauri/icons/128x128@2x.png" width="56" height="56" align="absmiddle" alt=""> yap

Push-to-talk dictation for macOS. Press a global shortcut, speak, and the transcript is pasted at the cursor.

- Works with any OpenAI-compatible `/audio/transcriptions` endpoint (OpenAI, LiteLLM, …)
- Hold or toggle mode, optional start/stop sounds
- Recordings stop and are transcribed at a max length
- Restores your previous clipboard after pasting
- Optional open at login
- Multiple languages and custom vocabulary (`gpt-transcribe`)
- Optional transformations: an LLM rewrites the transcript before pasting (an OpenAI-compatible `/chat/completions` endpoint or Claude Code with your subscription)
- Dictated instructions: a separate shortcut runs a spoken prompt, optionally with selected text
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

- Turn on **Enabled** and choose a provider. For **OpenAI-compatible API**, Base URL and API key default to the transcription ones; a separate endpoint, key and default model can be set.
- Each transformation has a system prompt, a user message (`{{transcript}}` marks where the transcript goes; empty sends the transcript alone), an optional model for each provider and an optional shortcut.
- A transformation's shortcut dictates and applies it. The regular shortcut applies the transformation checked in the menu bar menu; with **Use once** checked, the menu goes back to None after one dictation.
- If the transformation fails, nothing is pasted; the raw transcript is in History.

For **Claude Code (subscription)**:

1. Install a current [Claude Code](https://code.claude.com/docs/en/setup) version supporting `--safe-mode`, then run `claude auth login` in Terminal and sign in with your Claude subscription.
2. Choose **Claude Code (subscription)** in Transformations. The default model is `sonnet`; you can change it or override it on individual transformations. API and Claude models are saved separately.
3. Leave **Claude executable** empty to auto-detect a standard installation, or enter the absolute path to the executable (for example, the output of `command -v claude`).
4. Click **Test connection**, then **Save**. The test uses the current form values without saving and sends a small prompt that counts toward your plan usage.

Each transformation runs a fresh `claude -p` process with the user message on stdin, tools and customizations disabled, an isolated temporary working directory, and session persistence disabled. Calls time out after 60 seconds, including the subscription authentication check. Yap reuses Claude Code's login; it does not store Claude credentials. API authentication and provider environment overrides are excluded from the child process, and non-subscription logins are rejected. Managed Claude Code policies still apply.

Subscription usage limits and any extra usage enabled on your Claude account apply. Yap never automatically falls back to API billing. History records the provider and time; subscription transformations have unknown USD cost, rather than counting CLI cost estimates as billed charges. Transcription still requires its existing API key.

### Dictated instructions

Assign a **Dictated instruction shortcut** in Transformations and turn **Enabled** on. This shortcut uses the selected transformation provider and that provider's default model, plus the existing dictation hold/toggle mode, microphone and recording limit.

Select text in an editable input, activate the shortcut, and say an instruction such as “translate to Korean.” yap captures the selection when recording starts, transcribes your instruction, sends both to the selected transformation provider, and pastes the result over the selection. Without selected text, the spoken instruction runs alone—for example, “write a short birthday greeting.”

Keep focus and any intended selection in the target input until processing finishes: the result is pasted into whichever input is active at that time. Selection capture first uses macOS Accessibility. If an input does not expose selected text (as can happen in Slack), yap sends Copy, waits for fresh clipboard content, and restores the previous clipboard formats. Unchanged clipboard contents are never used as context. An Accessibility or capture error stops the action rather than sending an instruction with missing context.

The instruction shortcut leaves the menu bar's transformation pick and **Use once** unchanged. History identifies these entries as **Dictated instruction**; Details shows the spoken instruction and any selected text. If the model fails or returns no text, nothing is pasted and the instruction and context remain in History.

With transformations on, transcripts are also sent to the selected transformation provider. Dictated instructions also send any captured selected text.

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
