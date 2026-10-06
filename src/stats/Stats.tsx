import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { api, HISTORY_CHANGED, type Stats as StatsData } from "../api";
import { formatCost } from "../history/format";
import { ActivityChart } from "./ActivityChart";
import { bucketsFor, type Buckets, type Period } from "./buckets";
import { costInUsd, formatRecorded, metrics, type Metric } from "./format";
import { Info } from "./Info";
import { Segmented } from "./Segmented";

const PERIODS: { value: Period; label: string }[] = [
  { value: "today", label: "Today" },
  { value: "week", label: "7 days" },
  { value: "month", label: "30 days" },
  { value: "all", label: "All time" },
];

const METRICS = (Object.keys(metrics) as Metric[]).map((metric) => ({
  value: metric,
  label: metrics[metric].label,
}));

export function Stats() {
  const [period, setPeriod] = useState<Period>("week");
  const [metric, setMetric] = useState<Metric>("minutes");
  const [loaded, setLoaded] = useState<{ buckets: Buckets; stats: StatsData } | "empty" | null>(
    null,
  );

  useEffect(() => {
    let stale = false;
    const load = async () => {
      const firstHistoryAt = await api.firstHistoryAt();
      if (firstHistoryAt === null) {
        if (!stale) setLoaded("empty");
        return;
      }
      const buckets = bucketsFor(period, new Date(), firstHistoryAt);
      const stats = await api.getStats(buckets.starts, buckets.end);
      if (!stale) setLoaded({ buckets, stats });
    };
    load();
    const unlisten = listen(HISTORY_CHANGED, load);
    return () => {
      stale = true;
      unlisten.then((stop) => stop());
    };
  }, [period]);

  if (!loaded) {
    return null;
  }

  if (loaded === "empty") {
    return (
      <div className="empty">
        <p>No dictations yet.</p>
        <p className="empty-hint">Dictate with your shortcut or in Try it, and stats will show up here.</p>
      </div>
    );
  }

  const { total, buckets } = loaded.stats;
  const cost = costInUsd(total);

  return (
    <div className="stats">
      <Segmented name="period" label="Period" options={PERIODS} value={period} onChange={setPeriod} />
      <dl className="stat-tiles">
        <div className="stat-tile">
          <dt>
            <span className="stat-label">Dictations</span>
            <Info label="Dictations">
              Transcripts saved to History. Discarded recordings, blank transcripts and deleted
              entries don't count.
            </Info>
          </dt>
          <dd>{total.dictations}</dd>
        </div>
        <div className="stat-tile">
          <dt>
            <span className="stat-label">Recorded</span>
            <Info label="Recorded">Total length of the recorded audio that was transcribed.</Info>
          </dt>
          <dd>{formatRecorded(total.durationInSeconds)}</dd>
        </div>
        <div className="stat-tile">
          <dt>
            <span className="stat-label">Words</span>
            <Info label="Words">Words in the pasted text, after the transformation when one was applied.</Info>
          </dt>
          <dd>{total.words}</dd>
        </div>
        <div className="stat-tile">
          <dt>
            <span className="stat-label">Cost</span>
            <Info label="Cost">
              Transcription plus transformation (LLM) cost, as reported by LiteLLM. Requests without a
              reported cost are not included.
              {total.transformationCostInUsd !== null && (
                <span className="tooltip-detail">
                  <span>{formatCost(total.transcriptionCostInUsd ?? 0)} transcription</span>
                  <span>{formatCost(total.transformationCostInUsd)} transformation</span>
                </span>
              )}
            </Info>
          </dt>
          <dd>{cost === null ? "—" : formatCost(cost)}</dd>
        </div>
      </dl>
      <section className="group">
        <div className="chart-header">
          <h2>Activity</h2>
          <Segmented name="metric" label="Chart metric" options={METRICS} value={metric} onChange={setMetric} />
        </div>
        <div className="group-body">
          <ActivityChart buckets={loaded.buckets} usage={buckets} metric={metric} />
        </div>
      </section>
    </div>
  );
}
