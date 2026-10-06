import { useState } from "react";

export function SecretInput({
  value,
  placeholder,
  label,
  onChange,
}: {
  value: string;
  placeholder: string;
  label?: string;
  onChange: (value: string) => void;
}) {
  const [revealed, setRevealed] = useState(false);
  if (revealed && value === "") {
    setRevealed(false);
  }

  return (
    <div className="secret">
      <input
        type={revealed ? "text" : "password"}
        aria-label={label}
        value={value}
        spellCheck={false}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
      {value && (
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
  );
}
