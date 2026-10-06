import { useState } from "react";
import type { HistoryEntry } from "../api";
import { formatCost, formatDuration, formatSeconds, formatSize, joined } from "./format";

export function HistoryDetails({ entry }: { entry: HistoryEntry }) {
  const [open, setOpen] = useState(false);
  const audio =
    entry.sizeInBytes !== null &&
    joined([
      entry.durationInSeconds !== null && formatDuration(entry.durationInSeconds),
      formatSize(entry.sizeInBytes),
    ]);
  const timing = joined([
    entry.encodeTimeInSeconds !== null && `encode ${formatSeconds(entry.encodeTimeInSeconds)}`,
    entry.transcribeTimeInSeconds !== null &&
      `transcription ${formatSeconds(entry.transcribeTimeInSeconds)}`,
    entry.transformTimeInSeconds !== null && `LLM ${formatSeconds(entry.transformTimeInSeconds)}`,
  ]);
  const cost =
    entry.transformCostInUsd !== null &&
    joined([
      entry.costInUsd !== null && `${formatCost(entry.costInUsd)} transcription`,
      `${formatCost(entry.transformCostInUsd)} LLM`,
    ]);
  const rows = [
    { label: "Audio", value: audio },
    { label: "Timing", value: timing },
    { label: "Cost", value: cost },
    { label: "Error", value: entry.transformError },
  ].filter((row) => row.value);
  if (rows.length === 0) {
    return null;
  }
  return (
    <div className="card-details">
      <button
        type="button"
        className="disclosure"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M3.5 2 6.5 5l-3 3" />
        </svg>
        Details
      </button>
      {open && (
        <dl className="card-details-body">
          {rows.map(({ label, value }) => (
            <div key={label} className={label === "Error" ? "card-error" : undefined}>
              <dt>{label}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  );
}
