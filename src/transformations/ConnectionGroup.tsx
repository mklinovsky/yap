import type { Transformations } from "../api";
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
        <label className="row">
          <span className="row-label">Default model</span>
          <input
            value={transformations.defaultModel}
            spellCheck={false}
            onChange={(e) => onChange({ defaultModel: e.target.value })}
          />
        </label>
      </div>
      <p className="group-note">
        Transcripts are sent to this endpoint when a transformation runs. Pick one for the next
        dictation from the yap menu bar icon, or press its shortcut.
      </p>
    </section>
  );
}
