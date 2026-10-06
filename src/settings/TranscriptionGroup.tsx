import type { Settings } from "../api";
import { SecretInput } from "../components/SecretInput";
import { LanguagePicker } from "./LanguagePicker";

export function TranscriptionGroup({
  settings,
  apiKey,
  keyPreview,
  vocabulary,
  onChange,
  onApiKeyChange,
  onVocabularyChange,
}: {
  settings: Settings;
  apiKey: string;
  keyPreview: string | null;
  vocabulary: string;
  onChange: (patch: Partial<Settings>) => void;
  onApiKeyChange: (key: string) => void;
  onVocabularyChange: (vocabulary: string) => void;
}) {
  return (
    <section className="group">
      <h2>Transcription</h2>
      <div className="group-body">
        <label className="row">
          <span className="row-label">Base URL</span>
          <input
            value={settings.baseUrl}
            spellCheck={false}
            onChange={(e) => onChange({ baseUrl: e.target.value })}
          />
        </label>
        <label className="row">
          <span className="row-label">API key</span>
          <SecretInput
            value={apiKey}
            placeholder={keyPreview ?? "Not set"}
            onChange={onApiKeyChange}
          />
        </label>
        <label className="row">
          <span className="row-label">Model</span>
          <input
            value={settings.model}
            spellCheck={false}
            onChange={(e) => onChange({ model: e.target.value })}
          />
        </label>
        <div className="row">
          <span className="row-label">Languages</span>
          <LanguagePicker
            value={settings.languages}
            onChange={(languages) => onChange({ languages })}
          />
        </div>
        <label className="row">
          <span className="row-label">Vocabulary</span>
          <input
            value={vocabulary}
            placeholder="Names and terms, comma separated"
            onChange={(e) => onVocabularyChange(e.target.value)}
          />
        </label>
      </div>
      <p className="group-note">
        gpt-transcribe uses every language and the vocabulary; other models use only the first
        language and ignore the vocabulary.
      </p>
    </section>
  );
}
