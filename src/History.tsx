import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, HISTORY_CHANGED, type HistoryEntry } from "./api";

const formatCost = (cost: number) => `$${cost.toFixed(4)}`;

export function History() {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);

  useEffect(() => {
    const load = () => api.listHistory().then(setEntries);
    load();
    const unlisten = listen(HISTORY_CHANGED, load);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const remove = async (id: number) => {
    await api.deleteHistory(id);
    setEntries(await api.listHistory());
  };

  if (!entries) {
    return null;
  }

  if (entries.length === 0) {
    return (
      <div className="empty">
        <p>No transcripts yet.</p>
        <p className="empty-hint">Dictate with your shortcut or in Try it, and they will show up here.</p>
      </div>
    );
  }

  const costs = entries.flatMap((entry) => (entry.cost === null ? [] : [entry.cost]));

  return (
    <>
      {costs.length > 0 && (
        <p className="history-total">Total {formatCost(costs.reduce((sum, cost) => sum + cost, 0))}</p>
      )}
      <ul className="history">
        {entries.map((entry) => (
          <li key={entry.id} className="card">
            <header>
              <div className="card-meta">
                <time dateTime={new Date(entry.createdAt).toISOString()}>
                  {new Date(entry.createdAt).toLocaleString(undefined, {
                    dateStyle: "medium",
                    timeStyle: "short",
                  })}
                </time>
                {entry.cost !== null && <span className="card-cost">{formatCost(entry.cost)}</span>}
              </div>
              <div className="card-actions">
                <button
                  type="button"
                  className="icon-button"
                  aria-label="Copy"
                  title="Copy"
                  onClick={() => api.copyText(entry.text)}
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true">
                    <rect x="6.75" y="6.75" width="9.5" height="9.5" rx="2" />
                    <path d="M13.25 6.75V5.5a1.75 1.75 0 0 0-1.75-1.75h-6A1.75 1.75 0 0 0 3.75 5.5v6a1.75 1.75 0 0 0 1.75 1.75h1.25" />
                  </svg>
                </button>
                <button
                  type="button"
                  className="icon-button danger"
                  aria-label="Delete"
                  title="Delete"
                  onClick={() => remove(entry.id)}
                >
                  <svg viewBox="0 0 20 20" aria-hidden="true">
                    <path d="M4 5.75h12M8.25 5.75V4.5c0-.41.34-.75.75-.75h2c.41 0 .75.34.75.75v1.25M5.5 5.75l.7 9.6c.06.8.72 1.4 1.52 1.4h4.56c.8 0 1.46-.6 1.52-1.4l.7-9.6" />
                  </svg>
                </button>
              </div>
            </header>
            <p>{entry.text}</p>
          </li>
        ))}
      </ul>
    </>
  );
}
