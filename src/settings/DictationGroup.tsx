import { useEffect, useState } from "react";
import { api, type InputDevice, type Mode, type Settings } from "../api";
import { ShortcutRecorder } from "../components/ShortcutRecorder";

const MAX_MINUTES = [1, 2, 5, 10, 15, 20];

export function DictationGroup({
  settings,
  openAtLogin,
  onChange,
  onOpenAtLoginChange,
}: {
  settings: Settings;
  openAtLogin: boolean;
  onChange: (patch: Partial<Settings>) => void;
  onOpenAtLoginChange: (openAtLogin: boolean) => void;
}) {
  const [devices, setDevices] = useState<InputDevice[]>([]);

  useEffect(() => {
    const loadDevices = () => api.listInputDevices().then(setDevices);
    loadDevices();
    // The backend has no device-change event, so a mic plugged in while Settings is open is found by polling.
    const timer = setInterval(loadDevices, 2000);
    return () => clearInterval(timer);
  }, []);

  return (
    <section className="group">
      <h2>Dictation</h2>
      <div className="group-body">
        <label className="row">
          <span className="row-label">Input</span>
          <select
            value={settings.inputDevice ?? ""}
            onChange={(e) => onChange({ inputDevice: e.target.value || null })}
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
            onChange={(shortcut) => onChange({ shortcut })}
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
                  onChange={() => onChange({ mode })}
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
            onChange={(e) => onChange({ maxMinutes: Number(e.target.value) })}
          >
            {MAX_MINUTES.map((minutes) => (
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
            onChange={(e) => onChange({ sounds: e.target.checked })}
          />
        </label>
        <label className="row">
          <span className="row-label">Open at login</span>
          <input
            type="checkbox"
            className="switch"
            checked={openAtLogin}
            onChange={(e) => onOpenAtLoginChange(e.target.checked)}
          />
        </label>
      </div>
      <p className="group-note">
        A disconnected input falls back to the system default. Hold: record while the shortcut
        is held. Toggle: press once to start, again to stop. At the max length the recording
        stops and is transcribed.
      </p>
    </section>
  );
}
