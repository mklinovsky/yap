import { useState } from "react";
import type { HistoryEntry } from "../api";
import { formatCost, formatSeconds, formatSize, joined } from "./format";

export function HistoryDetails({ entry }: { entry: HistoryEntry }) {
  const [open, setOpen] = useState(false);
  const timing = joined([
    entry.sizeInBytes !== null && formatSize(entry.sizeInBytes),
    entry.encodeTimeInSeconds !== null && `encode ${formatSeconds(entry.encodeTimeInSeconds)}`,
    entry.transcribeTimeInSeconds !== null && `API ${formatSeconds(entry.transcribeTimeInSeconds)}`,
    entry.transformTimeInSeconds !== null && `LLM ${formatSeconds(entry.transformTimeInSeconds)}`,
  ]);
  const costs =
    entry.transformCostInUsd !== null &&
    [
      entry.costInUsd !== null && `${formatCost(entry.costInUsd)} transcription`,
      `${formatCost(entry.transformCostInUsd)} LLM`,
    ]
      .filter(Boolean)
      .join(" + ");
  if (!timing && !costs && entry.rawText === null && entry.transformError === null) {
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
        Details
      </button>
      {open && (
        <div className="card-details-body">
          {timing && <span>{timing}</span>}
          {costs && <span>{costs}</span>}
          {entry.transformError !== null && (
            <span className="card-error">{entry.transformError}</span>
          )}
          {entry.rawText !== null && (
            <span className="card-raw">
              Raw <q>{entry.rawText}</q>
            </span>
          )}
        </div>
      )}
    </div>
  );
}
