import { useState } from "react";
import { api, type Transformations } from "../api";
import { ShortcutRecorder } from "../components/ShortcutRecorder";
import { SecretInput } from "../components/SecretInput";

export function ConnectionGroup({
  transformations,
  transcriptionUrl,
  apiKey,
  keyPreview,
  onChange,
  onApiKeyChange,
}: {
  transformations: Transformations;
  transcriptionUrl: string;
  apiKey: string;
  keyPreview: string | null;
  onChange: (patch: Partial<Transformations>) => void;
  onApiKeyChange: (key: string) => void;
}) {
  const claude = transformations.provider === "claude";
  const [checking, setChecking] = useState(false);
  const [connectionStatus, setConnectionStatus] = useState("");
  const testConnection = async () => {
    setChecking(true);
    setConnectionStatus("");
    try {
      setConnectionStatus(
        await api.testClaudeConnection(
          transformations.claudeExecutable, transformations.claudeDefaultModel,
        ),
      );
    } catch (error) {
      setConnectionStatus(String(error));
    } finally {
      setChecking(false);
    }
  };
  return (
    <section className="group">
      <h2>Connection</h2>
      <div className="group-body">
        <label className="row">
          <span className="row-label">Enabled</span>
          <input
            type="checkbox"
            className="switch"
            checked={transformations.enabled}
            onChange={(e) => onChange({ enabled: e.target.checked })}
          />
        </label>
        <label className="row">
          <span className="row-label">Provider</span>
          <select
            value={transformations.provider}
            disabled={checking}
            onChange={(e) => {
              onChange({ provider: e.target.value as Transformations["provider"] });
              setConnectionStatus("");
            }}
          >
            <option value="api">OpenAI-compatible API</option>
            <option value="claude">Claude Code (subscription)</option>
          </select>
        </label>
        {claude ? (
          <>
            <label className="row">
              <span className="row-label">Claude executable</span>
              <input
                value={transformations.claudeExecutable ?? ""}
                placeholder="Auto-detect"
                disabled={checking}
                spellCheck={false}
                onChange={(e) => {
                  onChange({ claudeExecutable: e.target.value || null });
                  setConnectionStatus("");
                }}
              />
            </label>
            <div className="row">
              <span className="row-label">Connection</span>
              <button type="button" disabled={checking} onClick={testConnection}>
                {checking ? "Testing…" : "Test connection"}
              </button>
            </div>
            {connectionStatus && <p role="status" aria-label="Claude connection status" className="group-note">{connectionStatus}</p>}
          </>
        ) : (
          <>
            <label className="row">
              <span className="row-label">Base URL</span>
              <input
                value={transformations.baseUrl ?? transcriptionUrl}
                spellCheck={false}
                onChange={(e) => onChange({ baseUrl: e.target.value })}
              />
            </label>
            <div className="row">
              <span className="row-label">API key</span>
              <label className="inline-switch">
                <span>Use transcription API key</span>
                <input
                  type="checkbox"
                  className="switch"
                  checked={transformations.reuseApiKey}
                  onChange={(e) => onChange({ reuseApiKey: e.target.checked })}
                />
              </label>
            </div>
            {!transformations.reuseApiKey && (
              <div className="row">
                <span />
                <SecretInput
                  label="API key"
                  value={apiKey}
                  placeholder={keyPreview ?? "Not set"}
                  onChange={onApiKeyChange}
                />
              </div>
            )}
          </>
        )}
        <div className="row">
          <label className="row-label" htmlFor="instruction-shortcut">Dictated instruction shortcut</label>
          <div className="shortcut-field">
            <ShortcutRecorder
              id="instruction-shortcut"
              value={transformations.instructionShortcut ?? ""}
              onChange={(instructionShortcut) => onChange({ instructionShortcut })}
            />
            {transformations.instructionShortcut && (
              <button type="button" className="text-button" aria-label="Remove instruction shortcut"
                onClick={() => onChange({ instructionShortcut: null })}>Remove</button>
            )}
          </div>
        </div>
        <label className="row">
          <span className="row-label">Default model</span>
          <input
            value={claude ? transformations.claudeDefaultModel : transformations.defaultModel}
            spellCheck={false}
            disabled={checking}
            onChange={(e) => {
              onChange(claude
                ? { claudeDefaultModel: e.target.value }
                : { defaultModel: e.target.value });
              setConnectionStatus("");
            }}
          />
        </label>
      </div>
      <p className="group-note">
        {claude
          ? "Install Claude Code and run claude auth login in Terminal with your subscription. Transcripts are sent to Claude; each connection test uses a small amount of your plan allowance. Usage limits and any enabled extra usage apply."
          : "Transcripts are sent to this endpoint when a transformation runs. Pick one from the yap menu bar icon, or press its shortcut."}
      </p>
      <p className="group-note">
        The dictated instruction shortcut executes what you say, optionally using selected text
        as context. It uses the selected provider and its default model when Enabled is on.
        Keep focus and any selection in the target input until the result is pasted.
      </p>
    </section>
  );
}
