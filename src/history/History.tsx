import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, HISTORY_CHANGED, type HistoryEntry } from "../api";
import { formatCost, totalCostInUsd } from "./format";
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

  const costs = entries.flatMap((entry) => totalCostInUsd(entry) ?? []);

  return (
    <>
      {costs.length > 0 && (
        <p className="history-total">Total {formatCost(costs.reduce((sum, cost) => sum + cost, 0))}</p>
      )}
      <ul className="history">
        {entries.map((entry) => (
          <HistoryCard key={entry.id} entry={entry} onDelete={() => remove(entry.id)} />
        ))}
      </ul>
    </>
  );
}
