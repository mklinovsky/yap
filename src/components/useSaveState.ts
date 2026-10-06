import { useState } from "react";

export type SaveStatus = { ok: boolean; text: string } | null;

export function useSaveState(dirty: boolean) {
  const [result, setResult] = useState<SaveStatus>(null);
  const status: SaveStatus =
    result && !result.ok ? result : dirty ? { ok: true, text: "Unsaved changes" } : result;

  const onSubmit = (save: () => Promise<void>) => async (event: React.FormEvent) => {
    event.preventDefault();
    try {
      await save();
      setResult({ ok: true, text: "Saved" });
    } catch (error) {
      setResult({ ok: false, text: String(error) });
    }
  };

  return { status, onSubmit };
}
