import type { Transformation } from "../api";
import { ShortcutRecorder } from "../components/ShortcutRecorder";

export function TransformationEditor({
  item,
  defaultModel,
  onChange,
  onBack,
}: {
  item: Transformation;
  defaultModel: string;
  onChange: (patch: Partial<Transformation>) => void;
  onBack: () => void;
}) {
  return (
    <>
      <button
        type="button"
        className="text-button back"
        aria-label="Back to transformations"
        onClick={onBack}
      >
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="m12 5-5 5 5 5" />
        </svg>
        Transformations
      </button>
      <section className="group">
        <h2>{item.name}</h2>
        <div className="group-body">
          <label className="row">
            <span className="row-label">Name</span>
            <input value={item.name} onChange={(e) => onChange({ name: e.target.value })} />
          </label>
          <label className="row">
            <span className="row-label">Model</span>
            <input
              value={item.model ?? ""}
              spellCheck={false}
              placeholder={`${defaultModel} (default)`}
              onChange={(e) => onChange({ model: e.target.value === "" ? null : e.target.value })}
            />
          </label>
          <div className="row">
            <label className="row-label" htmlFor="transformation-shortcut">
              Shortcut
            </label>
            <div className="shortcut-field">
              <ShortcutRecorder
                id="transformation-shortcut"
                value={item.shortcut ?? ""}
                onChange={(shortcut) => onChange({ shortcut })}
              />
              {item.shortcut && (
                <button
                  type="button"
                  className="icon-button"
                  aria-label="Remove shortcut"
                  onClick={() => onChange({ shortcut: null })}
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true">
                    <path d="m6 6 8 8M14 6l-8 8" />
                  </svg>
                </button>
              )}
            </div>
          </div>
          <label className="row top">
            <span className="row-label">System prompt</span>
            <textarea
              rows={4}
              value={item.systemPrompt}
              onChange={(e) => onChange({ systemPrompt: e.target.value })}
            />
          </label>
          <label className="row top">
            <span className="row-label">User message</span>
            <textarea
              rows={3}
              value={item.userTemplate}
              spellCheck={false}
              placeholder="{{transcript}}"
              onChange={(e) => onChange({ userTemplate: e.target.value })}
            />
          </label>
        </div>
        <p className="group-note">
          {"{{transcript}}"} is replaced by the dictated text. Leave the user message empty to send
          the transcript alone. Shortcuts follow the Hold/Toggle mode from Settings.
        </p>
      </section>
    </>
  );
}
