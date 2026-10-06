import { useState } from "react";
import type { Usage } from "../api";
import type { Buckets, Unit } from "./buckets";
import { bucketLabel, bucketTitle, metrics, type Metric } from "./format";

const BAR_WIDTH = 0.7;

export function ActivityChart({
  buckets,
  usage,
  metric,
}: {
  buckets: Buckets;
  usage: Usage[];
  metric: Metric;
}) {
  const [hovered, setHovered] = useState<number | null>(null);
  const { value, format } = metrics[metric];
  const values = usage.map(value);
  const max = Math.max(0, ...values);
  const heightOf = (amount: number) => (max > 0 ? (amount / max) * 100 : 0);
  const last = values.length - 1;
  const labelled = [...new Set([0, Math.floor(last / 2), last])];

  return (
    <div className="chart">
      <span className="chart-max">{max > 0 ? format(max) : " "}</span>
      <div className="chart-plot">
        <svg
          className="chart-bars"
          viewBox={`0 0 ${values.length} 100`}
          preserveAspectRatio="none"
          role="group"
          aria-label="Activity"
          onMouseLeave={() => setHovered(null)}
        >
          {values.map((amount, i) => (
            <g key={buckets.starts[i]} className="chart-slot" data-hovered={hovered === i || undefined}>
              <rect
                className="chart-hover"
                x={i}
                y={0}
                width={1}
                height={100}
                role="img"
                aria-label={`${bucketTitle(buckets.starts[i], buckets.unit)} · ${format(amount)}`}
                onMouseEnter={() => setHovered(usage[i].dictations > 0 ? i : null)}
              />
              <rect
                className="chart-bar"
                x={i + (1 - BAR_WIDTH) / 2}
                y={100 - heightOf(amount)}
                width={BAR_WIDTH}
                height={heightOf(amount)}
              />
            </g>
          ))}
        </svg>
        {hovered !== null && (
          <BucketTooltip
            start={buckets.starts[hovered]}
            unit={buckets.unit}
            usage={usage[hovered]}
            centerInPercent={((hovered + 0.5) / values.length) * 100}
            bottomInPercent={heightOf(values[hovered])}
          />
        )}
      </div>
      <div className="chart-labels">
        {labelled.map((i) => (
          <span key={i}>{bucketLabel(buckets.starts[i], buckets.unit)}</span>
        ))}
      </div>
    </div>
  );
}

function BucketTooltip({
  start,
  unit,
  usage,
  centerInPercent,
  bottomInPercent,
}: {
  start: number;
  unit: Unit;
  usage: Usage;
  centerInPercent: number;
  bottomInPercent: number;
}) {
  return (
    <div
      className="tooltip chart-tooltip"
      role="tooltip"
      style={{
        left: `${centerInPercent}%`,
        bottom: `calc(${bottomInPercent}% + 6px)`,
        transform: `translateX(-${centerInPercent}%)`,
      }}
    >
      <strong>{bucketTitle(start, unit)}</strong>
      <span>
        {Object.values(metrics)
          .map(({ value, format }) => format(value(usage)))
          .join(" · ")}
      </span>
    </div>
  );
}
