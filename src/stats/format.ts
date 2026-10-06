import type { Usage } from "../api";
import { formatCost } from "../history/format";
import type { Unit } from "./buckets";

export type Metric = "minutes" | "dictations" | "cost";

export const costInUsd = (usage: Usage) =>
  usage.transcriptionCostInUsd === null && usage.transformationCostInUsd === null
    ? null
    : (usage.transcriptionCostInUsd ?? 0) + (usage.transformationCostInUsd ?? 0);

export const metrics: Record<
  Metric,
  { label: string; value: (usage: Usage) => number; format: (value: number) => string }
> = {
  minutes: {
    label: "Minutes",
    value: (usage) => usage.durationInSeconds / 60,
    format: (minutes) => `${minutes.toFixed(1)} min`,
  },
  dictations: {
    label: "Dictations",
    value: (usage) => usage.dictations,
    format: (count) => `${count} ${count === 1 ? "dictation" : "dictations"}`,
  },
  cost: {
    label: "Cost",
    value: (usage) => costInUsd(usage) ?? 0,
    format: formatCost,
  },
};

export function formatRecorded(seconds: number) {
  if (seconds < 60) {
    return `${Math.round(seconds)} s`;
  }
  const minutes = Math.round(seconds / 60);
  return minutes < 60 ? `${minutes} min` : `${Math.floor(minutes / 60)} h ${minutes % 60} min`;
}

const labelFormats: Record<Unit, Intl.DateTimeFormatOptions> = {
  hour: { hour: "numeric" },
  day: { day: "numeric", month: "short" },
  week: { day: "numeric", month: "short" },
  month: { month: "short", year: "numeric" },
};

export const bucketLabel = (start: number, unit: Unit) =>
  new Date(start).toLocaleString(undefined, labelFormats[unit]);

export const bucketTitle = (start: number, unit: Unit) =>
  unit === "week" ? `Week of ${bucketLabel(start, unit)}` : bucketLabel(start, unit);
