import { api, type HistoryEntry } from "../api";
import { formatCost, formatDuration, joined, totalCostInUsd } from "./format";
import { HistoryDetails } from "./HistoryDetails";

export function HistoryCard({ entry, onDelete }: { entry: HistoryEntry; onDelete: () => void }) {
  const cost = totalCostInUsd(entry);
  const summary = joined([
    entry.durationInSeconds !== null && formatDuration(entry.durationInSeconds),
    cost !== null && formatCost(cost),
  ]);
  const failed = entry.transformError !== null;

  return (
    <li className="card">
      <header>
        <div className="card-meta">
          <time dateTime={new Date(entry.createdAt).toISOString()}>
            {new Date(entry.createdAt).toLocaleString(undefined, {
              dateStyle: "medium",
              timeStyle: "short",
            })}
          </time>
          {entry.transformationName !== null && (
            <span
              className={failed ? "transform-tag failed" : "transform-tag"}
              title={failed ? "Transformation failed" : undefined}
            >
              {entry.transformationName}
            </span>
          )}
          {summary && <span className="card-summary">{summary}</span>}
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
            onClick={onDelete}
          >
            <svg viewBox="0 0 20 20" aria-hidden="true">
              <path d="M4 5.75h12M8.25 5.75V4.5c0-.41.34-.75.75-.75h2c.41 0 .75.34.75.75v1.25M5.5 5.75l.7 9.6c.06.8.72 1.4 1.52 1.4h4.56c.8 0 1.46-.6 1.52-1.4l.7-9.6" />
            </svg>
          </button>
        </div>
      </header>
      <p>{entry.text}</p>
      <HistoryDetails entry={entry} />
    </li>
  );
}
