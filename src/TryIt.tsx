import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { api, STATUS_CHANGED, type Status } from "./api";

const LABELS: Record<Status["state"], string> = {
  idle: "Start recording",
  error: "Start recording",
  recording: "Stop recording",
  transcribing: "Transcribing…",
};

function StateIcon({ state }: { state: Status["state"] }) {
  if (state === "recording") return <span className="record-dot" aria-hidden="true" />;
  if (state === "transcribing") return <span className="spinner" aria-hidden="true" />;
  return (
    <svg className="mic" viewBox="0 0 20 20" aria-hidden="true">
      <rect x="7.25" y="2.75" width="5.5" height="9.5" rx="2.75" />
      <path d="M4.75 9.5a5.25 5.25 0 0 0 10.5 0M10 14.75v2.5" />
    </svg>
  );
}

export function TryIt() {
  const pad = useRef<HTMLTextAreaElement>(null);
  const [status, setStatus] = useState<Status>({ state: "idle", canRetry: false });

  useEffect(() => {
    api.getStatus().then(setStatus);
    const unlisten = listen<Status>(STATUS_CHANGED, (event) => setStatus(event.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  // The transcript is pasted with a real Cmd/Ctrl+V, so the scratch pad must hold focus.
  const focusPadAnd = (action: () => Promise<void>) => async () => {
    pad.current?.focus();
    await action();
  };
  const canRetry = status.canRetry && (status.state === "idle" || status.state === "error");

  return (
    <div className="try-it">
      <label className="scratch">
        <span>Scratch pad</span>
        <textarea ref={pad} placeholder="Your transcript will be pasted here." />
      </label>
      <div className="try-controls">
        <button
          type="button"
          className={`record ${status.state}`}
          disabled={status.state === "transcribing"}
          onClick={focusPadAnd(api.toggleRecording)}
        >
          <StateIcon state={status.state} />
          {LABELS[status.state]}
        </button>
        {canRetry && (
          <button type="button" className="record retry" onClick={focusPadAnd(api.retryRecording)}>
            Retry
          </button>
        )}
      </div>
      {status.state === "error" && (
        <p className="try-error" role="alert">
          {status.message}
        </p>
      )}
    </div>
  );
}
