import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { HISTORY_CHANGED, type HistoryEntry } from "./api";
import { History } from "./History";

function mockBackend(entries: HistoryEntry[]) {
  const calls: { cmd: string; payload: unknown }[] = [];
  const backend = { calls, rows: [...entries] };
  mockIPC(
    (cmd, payload) => {
      calls.push({ cmd, payload });
      switch (cmd) {
        case "list_history":
          return backend.rows;
        case "delete_history":
          backend.rows = backend.rows.filter((row) => row.id !== (payload as { id: number }).id);
          return null;
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  return backend;
}

const entries: HistoryEntry[] = [
  { id: 2, text: "Second thought", createdAt: Date.UTC(2026, 9, 5, 12, 30) },
  { id: 1, text: "First thought", createdAt: Date.UTC(2026, 9, 5, 12, 0) },
];

test("lists transcripts in the order the backend returns them", async () => {
  mockBackend(entries);

  render(<History />);

  const items = await screen.findAllByRole("listitem");
  expect(items.map((item) => item.querySelector("p")?.textContent)).toEqual([
    "Second thought",
    "First thought",
  ]);
});

test("copies a transcript to the clipboard", async () => {
  const user = userEvent.setup();
  const { calls } = mockBackend(entries);
  render(<History />);

  const [, first] = await screen.findAllByRole("listitem");
  await user.click(within(first).getByRole("button", { name: "Copy" }));

  expect(calls.find((c) => c.cmd === "copy_text")?.payload).toEqual({ text: "First thought" });
});

test("deleting a transcript removes it from the list", async () => {
  const user = userEvent.setup();
  mockBackend(entries);
  render(<History />);

  const [newest] = await screen.findAllByRole("listitem");
  await user.click(within(newest).getByRole("button", { name: "Delete" }));

  await expect.poll(() => screen.queryByText("Second thought")).toBeNull();
  expect(screen.getByText("First thought")).toBeInTheDocument();
});

test("shows a new transcript when the backend announces a history change", async () => {
  const backend = mockBackend(entries);
  render(<History />);
  await screen.findByText("Second thought");

  backend.rows = [{ id: 3, text: "Fresh dictation", createdAt: Date.UTC(2026, 9, 5, 13, 0) }, ...entries];
  await emit(HISTORY_CHANGED);

  expect(await screen.findByText("Fresh dictation")).toBeInTheDocument();
});

test("explains that nothing has been dictated yet when history is empty", async () => {
  mockBackend([]);

  render(<History />);

  expect(await screen.findByText("No transcripts yet.")).toBeInTheDocument();
});
