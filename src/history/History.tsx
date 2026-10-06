import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { api, HISTORY_CHANGED, type HistoryEntry } from "../api";
import { HistoryCard } from "./HistoryCard";

const PAGE_SIZE = 50;

interface Loaded {
  entries: HistoryEntry[];
  hasMore: boolean;
}

function withNewest(loaded: Loaded | null, newestPage: HistoryEntry[]): Loaded {
  const newestLoadedId = loaded?.entries[0]?.id ?? 0;
  const fresh = newestPage.filter((entry) => entry.id > newestLoadedId);
  if (!loaded || fresh.length === newestPage.length) {
    return { entries: newestPage, hasMore: newestPage.length === PAGE_SIZE };
  }
  return { ...loaded, entries: [...fresh, ...loaded.entries] };
}

export function History() {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const loadingMore = useRef(false);
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const loadNewest = () =>
      api
        .listHistory(null, PAGE_SIZE)
        .then((page) => setLoaded((current) => withNewest(current, page)));
    loadNewest();
    const unlisten = listen(HISTORY_CHANGED, loadNewest);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const lastId = loaded?.entries[loaded.entries.length - 1]?.id ?? null;
  const hasMore = loaded?.hasMore ?? false;

  useEffect(() => {
    if (!hasMore || !end.current) {
      return;
    }
    const loadOlder = async () => {
      if (loadingMore.current) {
        return;
      }
      loadingMore.current = true;
      try {
        const page = await api.listHistory(lastId, PAGE_SIZE);
        setLoaded((current) =>
          current && (current.entries[current.entries.length - 1]?.id ?? null) === lastId
            ? { entries: [...current.entries, ...page], hasMore: page.length === PAGE_SIZE }
            : current,
        );
      } finally {
        loadingMore.current = false;
      }
    };
    const observer = new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting) {
        loadOlder();
      }
    });
    observer.observe(end.current);
    return () => observer.disconnect();
  }, [lastId, hasMore]);

  const remove = async (id: number) => {
    await api.deleteHistory(id);
    setLoaded(
      (current) =>
        current && { ...current, entries: current.entries.filter((entry) => entry.id !== id) },
    );
  };

  if (!loaded) {
    return null;
  }

  if (loaded.entries.length === 0 && !hasMore) {
    return (
      <div className="empty">
        <p>No transcripts yet.</p>
        <p className="empty-hint">Dictate with your shortcut or in Try it, and they will show up here.</p>
      </div>
    );
  }

  return (
    <>
      <ul className="history">
        {loaded.entries.map((entry) => (
          <HistoryCard key={entry.id} entry={entry} onDelete={() => remove(entry.id)} />
        ))}
      </ul>
      {hasMore && <div ref={end} aria-hidden="true" />}
    </>
  );
}
