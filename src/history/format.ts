import type { HistoryEntry } from "../api";

export const formatCost = (cost: number) => `$${cost.toFixed(4)}`;

export const formatDuration = (seconds: number) => {
  if (seconds < 60) {
    return `${seconds.toFixed(1)} s`;
  }
  const whole = Math.round(seconds);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
};

export const formatSeconds = (seconds: number) => `${seconds.toFixed(2)} s`;

export const formatSize = (bytes: number) =>
  bytes < 1_000_000 ? `${Math.round(bytes / 1000)} KB` : `${(bytes / 1_000_000).toFixed(1)} MB`;

export const totalCostInUsd = (entry: HistoryEntry) =>
  entry.costInUsd === null && entry.transformCostInUsd === null
    ? null
    : (entry.costInUsd ?? 0) + (entry.transformCostInUsd ?? 0);

export const joined = (parts: (string | false | null)[]) => parts.filter(Boolean).join(" · ");
