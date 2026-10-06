import type { Settings, Theme } from "../api";

const THEMES: { theme: Theme; label: string }[] = [
  { theme: "auto", label: "Auto" },
  { theme: "light", label: "Light" },
  { theme: "dark", label: "Dark" },
];

export function AppearanceGroup({
  settings,
  onChange,
}: {
  settings: Settings;
  onChange: (patch: Partial<Settings>) => void;
}) {
  return (
    <section className="group">
      <h2>Appearance</h2>
      <div className="group-body">
        <div className="row">
          <span className="row-label" id="theme-label">
            Theme
          </span>
          <div className="segmented" role="radiogroup" aria-labelledby="theme-label">
            {THEMES.map(({ theme, label }) => (
              <label key={theme}>
                <input
                  type="radio"
                  name="theme"
                  checked={settings.theme === theme}
                  onChange={() => onChange({ theme })}
                />
                <span>{label}</span>
              </label>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
