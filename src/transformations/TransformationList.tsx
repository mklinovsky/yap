import type { Transformation } from "../api";
import { formatShortcut } from "../components/ShortcutRecorder";

export function TransformationList({
  items,
  onEdit,
  onAdd,
}: {
  items: Transformation[];
  onEdit: (id: string) => void;
  onAdd: () => void;
}) {
  return (
    <section className="group">
      <h2>Transformations</h2>
      <div className="group-body">
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            className="row list-row"
            aria-label={`Edit ${item.name}`}
            onClick={() => onEdit(item.id)}
          >
            <span className="list-name">{item.name}</span>
            <span className="list-model">{item.model ?? "default model"}</span>
            <span className={item.shortcut ? "keycap" : "keycap none"}>
              {item.shortcut ? formatShortcut(item.shortcut) : "No shortcut"}
            </span>
            <svg className="chevron" viewBox="0 0 20 20" aria-hidden="true">
              <path d="m8 5 5 5-5 5" />
            </svg>
          </button>
        ))}
        <button type="button" className="row list-row add" onClick={onAdd}>
          Add transformation
        </button>
      </div>
    </section>
  );
}
