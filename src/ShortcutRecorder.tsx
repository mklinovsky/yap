import { useEffect, useRef, useState } from "react";
import { api } from "./api";

const MODIFIER_KEYS = new Set(["Control", "Alt", "Shift", "Meta"]);
const MAC = typeof navigator !== "undefined" && /Mac/.test(navigator.userAgent);
const MAC_SYMBOLS: Record<string, string> = { Ctrl: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘" };

function keyName(code: string) {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  return code;
}

function shortcutFrom(event: KeyboardEvent) {
  if (MODIFIER_KEYS.has(event.key)) return null;
  const modifiers = [
    event.ctrlKey && "Ctrl",
    event.altKey && "Alt",
    event.shiftKey && "Shift",
    event.metaKey && "Super",
  ].filter((m): m is string => Boolean(m));
  const key = keyName(event.code);
  // A bare letter or digit would hijack normal typing everywhere; only function keys may stand alone.
  if (modifiers.length === 0 && !/^F\d+$/.test(key)) return null;
  return [...modifiers, key].join("+");
}

export function formatShortcut(shortcut: string) {
  if (!MAC) return shortcut;
  const parts = shortcut.split("+");
  const key = parts.pop() ?? "";
  return parts.map((part) => MAC_SYMBOLS[part] ?? part).join("") + key;
}

export function ShortcutRecorder({
  value,
  onChange,
}: {
  value: string;
  onChange: (shortcut: string) => void;
}) {
  const [recording, setRecording] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const latestOnChange = useRef(onChange);

  useEffect(() => {
    latestOnChange.current = onChange;
  });

  useEffect(() => {
    if (!recording) return;
    api.pauseShortcut();
    // WebKit does not focus a button on click, so keys must be captured at the window.
    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") {
        setRecording(false);
        return;
      }
      const shortcut = shortcutFrom(event);
      if (shortcut) {
        latestOnChange.current(shortcut);
        setRecording(false);
      }
    };
    const onPointerDown = (event: PointerEvent) => {
      if (!button.current?.contains(event.target as Node)) setRecording(false);
    };
    const cancel = () => setRecording(false);
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("blur", cancel);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("blur", cancel);
      api.resumeShortcut();
    };
  }, [recording]);

  return (
    <button
      ref={button}
      type="button"
      className={recording ? "recorder recording" : "recorder"}
      onClick={() => setRecording(!recording)}
    >
      {recording ? "Press a shortcut…" : formatShortcut(value)}
    </button>
  );
}
