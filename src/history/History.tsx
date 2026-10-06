import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, HISTORY_CHANGED, type HistoryEntry } from "../api";
import { HistoryCard } from "./HistoryCard";

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

  return (
    <ul className="history">
      {entries.map((entry) => (
        <HistoryCard key={entry.id} entry={entry} onDelete={() => remove(entry.id)} />
      ))}
    </ul>
  );
}
