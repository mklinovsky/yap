import { LANGUAGES } from "./languages";

const nameOf = (code: string) => LANGUAGES.find((l) => l.code === code)?.name ?? code;

export function LanguagePicker({
  value,
  onChange,
}: {
  value: string[];
  onChange: (languages: string[]) => void;
}) {
  const available = LANGUAGES.filter((l) => !value.includes(l.code));

  return (
    <div className="languages">
      <ul className="chips">
        {value.length === 0 && <li className="chip auto">Auto-detect</li>}
        {value.map((code) => (
          <li key={code} className="chip" aria-label={nameOf(code)}>
            {nameOf(code)}
            <button
              type="button"
              aria-label={`Remove ${nameOf(code)}`}
              onClick={() => onChange(value.filter((c) => c !== code))}
            >
              <svg viewBox="0 0 12 12" aria-hidden="true">
                <path d="M3.5 3.5l5 5M8.5 3.5l-5 5" />
              </svg>
            </button>
          </li>
        ))}
      </ul>
      <select
        aria-label="Add language"
        value=""
        onChange={(e) => e.target.value && onChange([...value, e.target.value])}
      >
        <option value="">Add language…</option>
        {available.map((l) => (
          <option key={l.code} value={l.code}>
            {l.name}
          </option>
        ))}
      </select>
    </div>
  );
}
