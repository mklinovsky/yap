import { useId, type ReactNode } from "react";

export function Info({ label, children }: { label: string; children: ReactNode }) {
  const id = useId();
  return (
    <span className="info">
      <span className="info-icon" role="img" tabIndex={0} aria-label={`About ${label}`} aria-describedby={id}>
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="8" cy="8" r="6.25" />
          <path d="M8 7.25v3.75M8 5.1v.01" />
        </svg>
      </span>
      <span className="tooltip" role="tooltip" id={id}>
        {children}
      </span>
    </span>
  );
}
