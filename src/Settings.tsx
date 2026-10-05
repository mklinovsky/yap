import { useEffect, useState } from "react";
import { api, type InputDevice, type Mode, type Settings as SettingsData } from "./api";
import { LanguagePicker } from "./LanguagePicker";
import { ShortcutRecorder } from "./ShortcutRecorder";

const MAX_MINUTES = [5, 10, 20, 30, 60];

export function Settings() {
  const [settings, setSettings] = useState<SettingsData | null>(null);
  const [keyPreview, setKeyPreview] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [revealed, setRevealed] = useState(false);
  const [vocabulary, setVocabulary] = useState("");
  const [saved, setSaved] = useState<{ settings: SettingsData; vocabulary: string } | null>(null);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const [devices, setDevices] = useState<InputDevice[]>([]);

  useEffect(() => {
    api.getSettings().then((loaded) => {
      const loadedVocabulary = loaded.keywords.join(", ");
      setSettings(loaded);
      setVocabulary(loadedVocabulary);
      setSaved({ settings: loaded, vocabulary: loadedVocabulary });
    });
    api.apiKeyPreview().then(setKeyPreview);
    api.listInputDevices().then(setDevices);
  }, []);

  if (!settings) {
    return null;
  }

  const update = (patch: Partial<SettingsData>) => setSettings({ ...settings, ...patch });

  const dirty =
    apiKey !== "" ||
    vocabulary !== saved?.vocabulary ||
    JSON.stringify(settings) !== JSON.stringify(saved?.settings);
  const status =
    result && !result.ok ? result : dirty ? { ok: true, text: "Unsaved changes" } : result;

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    try {
      const keywords = vocabulary
        .split(",")
        .map((term) => term.trim())
        .filter(Boolean);
      const next = { ...settings, keywords };
      await api.saveSettings(next);
      if (apiKey) {
        await api.setApiKey(apiKey);
        setApiKey("");
        setRevealed(false);
        setKeyPreview(await api.apiKeyPreview());
      }
      const nextVocabulary = keywords.join(", ");
      setSettings(next);
      setVocabulary(nextVocabulary);
      setSaved({ settings: next, vocabulary: nextVocabulary });
      setResult({ ok: true, text: "Saved" });
    } catch (error) {
      setResult({ ok: false, text: String(error) });
    }
  };

  return (
    <form className="settings-page" onSubmit={save}>
      <div className="settings-scroll">
        <div className="settings">
          <section className="group">
            <h2>Transcription</h2>
            <div className="group-body">
              <label className="row">
                <span className="row-label">Base URL</span>
                <input
                  value={settings.baseUrl}
                  spellCheck={false}
                  onChange={(e) => update({ baseUrl: e.target.value })}
                />
              </label>
              <label className="row">
                <span className="row-label">API key</span>
                <div className="secret">
                  <input
                    type={revealed ? "text" : "password"}
                    value={apiKey}
                    spellCheck={false}
                    placeholder={keyPreview ?? "Not set"}
                    onChange={(e) => setApiKey(e.target.value)}
                  />
                  {apiKey && (
                    <button
                      type="button"
                      className="icon-button"
                      aria-label={revealed ? "Hide API key" : "Show API key"}
                      onClick={() => setRevealed(!revealed)}
                    >
                      <svg viewBox="0 0 20 20" aria-hidden="true">
                        <path d="M2.5 10s2.75-5.25 7.5-5.25S17.5 10 17.5 10s-2.75 5.25-7.5 5.25S2.5 10 2.5 10Z" />
                        <circle cx="10" cy="10" r="2.25" />
                        {revealed && <path d="M4 16 16 4" />}
                      </svg>
                    </button>
                  )}
                </div>
              </label>
              <label className="row">
                <span className="row-label">Model</span>
                <input
                  value={settings.model}
                  spellCheck={false}
                  onChange={(e) => update({ model: e.target.value })}
                />
              </label>
              <div className="row">
                <span className="row-label">Languages</span>
                <LanguagePicker
                  value={settings.languages}
                  onChange={(languages) => update({ languages })}
                />
              </div>
              <label className="row">
                <span className="row-label">Vocabulary</span>
                <input
                  value={vocabulary}
                  placeholder="Names and terms, comma separated"
                  onChange={(e) => setVocabulary(e.target.value)}
                />
              </label>
            </div>
            <p className="group-note">
              gpt-transcribe uses every language and the vocabulary; other models use only the first
              language and ignore the vocabulary.
            </p>
          </section>

          <section className="group">
            <h2>Dictation</h2>
            <div className="group-body">
              <label className="row">
                <span className="row-label">Input</span>
                <select
                  value={settings.inputDevice ?? ""}
                  onChange={(e) => update({ inputDevice: e.target.value || null })}
                >
                  <option value="">System default</option>
                  {settings.inputDevice && !devices.some((d) => d.id === settings.inputDevice) && (
                    <option value={settings.inputDevice}>Disconnected device</option>
                  )}
                  {devices.map((device) => (
                    <option key={device.id} value={device.id}>
                      {device.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="row">
                <span className="row-label">Shortcut</span>
                <ShortcutRecorder
                  value={settings.shortcut}
                  onChange={(shortcut) => update({ shortcut })}
                />
              </label>
              <div className="row">
                <span className="row-label" id="mode-label">
                  Mode
                </span>
                <div className="segmented" role="radiogroup" aria-labelledby="mode-label">
                  {(["hold", "toggle"] as Mode[]).map((mode) => (
                    <label key={mode}>
                      <input
                        type="radio"
                        name="mode"
                        checked={settings.mode === mode}
                        onChange={() => update({ mode })}
                      />
                      <span>{mode === "hold" ? "Hold" : "Toggle"}</span>
                    </label>
                  ))}
                </div>
              </div>
              <label className="row">
                <span className="row-label">Max length</span>
                <select
                  value={settings.maxMinutes}
                  onChange={(e) => update({ maxMinutes: Number(e.target.value) })}
                >
                  {[...new Set([...MAX_MINUTES, settings.maxMinutes])]
                    .sort((a, b) => a - b)
                    .map((minutes) => (
                      <option key={minutes} value={minutes}>
                        {minutes} min
                      </option>
                    ))}
                </select>
              </label>
              <label className="row">
                <span className="row-label">Sounds</span>
                <input
                  type="checkbox"
                  className="switch"
                  checked={settings.sounds}
                  onChange={(e) => update({ sounds: e.target.checked })}
                />
              </label>
            </div>
            <p className="group-note">
              A disconnected input falls back to the system default. Hold: record while the shortcut
              is held. Toggle: press once to start, again to stop. At the max length the recording
              stops and is transcribed; OpenAI rejects uploads over 25 MB (about 20 minutes).
            </p>
          </section>

        </div>
      </div>
      <footer className="settings-bar">
        <output className={status && !status.ok ? "failed" : undefined}>{status?.text}</output>
        <button type="submit" className="primary" disabled={!dirty}>
          Save
        </button>
      </footer>
    </form>
  );
}
