import type { ReactNode } from "react";
import type { SaveStatus } from "./useSaveState";

export function SaveBar({
  status,
  dirty,
  children,
}: {
  status: SaveStatus;
  dirty: boolean;
  children?: ReactNode;
}) {
  return (
    <footer className="settings-bar">
      {children}
      <output className={status && !status.ok ? "failed" : undefined}>{status?.text}</output>
      <button type="submit" className="primary" disabled={!dirty}>
        Save
      </button>
    </footer>
  );
}
