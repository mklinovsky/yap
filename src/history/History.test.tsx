import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { HISTORY_CHANGED, type HistoryEntry } from "../api";
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

const plain = {
  rawText: null,
  transformationName: null,
  transformError: null,
  transformCostInUsd: null,
  transformTimeInSeconds: null,
};

const entries: HistoryEntry[] = [
  {
    id: 2,
    text: "Second thought",
    createdAt: Date.UTC(2026, 9, 5, 12, 30),
    costInUsd: 0.0012,
    durationInSeconds: 12.4,
    sizeInBytes: 204_800,
    encodeTimeInSeconds: 0.031,
    transcribeTimeInSeconds: 4.214,
    ...plain,
  },
  {
    id: 1,
    text: "First thought",
    createdAt: Date.UTC(2026, 9, 5, 12, 0),
    costInUsd: null,
    durationInSeconds: null,
    sizeInBytes: null,
    encodeTimeInSeconds: null,
    transcribeTimeInSeconds: null,
    ...plain,
  },
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

  backend.rows = [
    {
      id: 3,
      text: "Fresh dictation",
      createdAt: Date.UTC(2026, 9, 5, 13, 0),
      costInUsd: null,
      durationInSeconds: null,
      sizeInBytes: null,
      encodeTimeInSeconds: null,
      transcribeTimeInSeconds: null,
      ...plain,
    },
    ...entries,
  ];
  await emit(HISTORY_CHANGED);

  expect(await screen.findByText("Fresh dictation")).toBeInTheDocument();
});

test("explains that nothing has been dictated yet when history is empty", async () => {
  mockBackend([]);

  render(<History />);

  expect(await screen.findByText("No transcripts yet.")).toBeInTheDocument();
});

test("shows how long each recording was and what it cost", async () => {
  mockBackend(entries);

  render(<History />);

  const [priced, older] = await screen.findAllByRole("listitem");
  expect(within(priced).getByText("12.4 s · $0.0012")).toBeInTheDocument();
  expect(within(older).queryByText(/\$| s/)).toBeNull();
});

test("totals the cost of the listed transcripts", async () => {
  mockBackend([
    ...entries,
    {
      id: 0,
      text: "Earlier thought",
      createdAt: Date.UTC(2026, 9, 5, 11, 0),
      costInUsd: 0.003,
      durationInSeconds: null,
      sizeInBytes: null,
      encodeTimeInSeconds: null,
      transcribeTimeInSeconds: null,
      ...plain,
    },
  ]);

  render(<History />);

  expect(await screen.findByText("Total $0.0042")).toBeInTheDocument();
});

test("shows no total when no transcript has a cost", async () => {
  mockBackend(entries.map((entry) => ({ ...entry, costInUsd: null })));

  render(<History />);

  await screen.findByText("Second thought");
  expect(screen.queryByText(/Total/)).toBeNull();
});

test("details show the upload size and how long encoding and the request took", async () => {
  const user = userEvent.setup();
  mockBackend(entries);
  render(<History />);

  const [recorded] = await screen.findAllByRole("listitem");
  expect(within(recorded).queryByText(/API/)).toBeNull();
  await user.click(within(recorded).getByRole("button", { name: "Details" }));

  expect(within(recorded).getByText("205 KB · encode 0.03 s · API 4.21 s")).toBeInTheDocument();
});

test("entries from older versions have no details", async () => {
  mockBackend(entries);

  render(<History />);

  const [, older] = await screen.findAllByRole("listitem");
  expect(within(older).queryByRole("button", { name: "Details" })).toBeNull();
});

test("shows minutes and megabytes for long recordings", async () => {
  const user = userEvent.setup();
  mockBackend([{ ...entries[0], durationInSeconds: 75.2, sizeInBytes: 2_400_000 }]);
  render(<History />);

  expect(await screen.findByText("1:15 · $0.0012")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Details" }));

  expect(screen.getByText(/^2\.4 MB/)).toBeInTheDocument();
});

const transformed: HistoryEntry = {
  ...entries[0],
  id: 5,
  text: "The meeting moved to Thursday.",
  rawText: "the meeting uh moved to thursday",
  transformationName: "Fix grammar",
  transformCostInUsd: 0.0007,
  transformTimeInSeconds: 1.12,
};

test("a transformed entry names its transformation and adds both costs", async () => {
  mockBackend([transformed]);

  render(<History />);

  const [card] = await screen.findAllByRole("listitem");
  expect(within(card).getByText("Fix grammar")).toBeInTheDocument();
  expect(within(card).getByText("12.4 s · $0.0019")).toBeInTheDocument();
});

test("details of a transformed entry show the raw transcript and the cost split", async () => {
  const user = userEvent.setup();
  mockBackend([transformed]);
  render(<History />);

  const [card] = await screen.findAllByRole("listitem");
  await user.click(within(card).getByRole("button", { name: "Details" }));

  expect(
    within(card).getByText("205 KB · encode 0.03 s · API 4.21 s · LLM 1.12 s"),
  ).toBeInTheDocument();
  expect(within(card).getByText("$0.0012 transcription + $0.0007 LLM")).toBeInTheDocument();
  expect(within(card).getByText("the meeting uh moved to thursday")).toBeInTheDocument();
});

test("a failed transformation is marked and its details carry the error", async () => {
  const user = userEvent.setup();
  mockBackend([
    {
      ...entries[0],
      transformationName: "Translate",
      transformError: "Transformation failed: HTTP 500: boom",
    },
  ]);
  render(<History />);

  const [card] = await screen.findAllByRole("listitem");
  expect(within(card).getByText("Translate")).toHaveAttribute("title", "Transformation failed");
  await user.click(within(card).getByRole("button", { name: "Details" }));

  expect(within(card).getByText("Transformation failed: HTTP 500: boom")).toBeInTheDocument();
});

test("the total includes transformation costs", async () => {
  mockBackend([transformed, entries[1]]);

  render(<History />);

  expect(await screen.findByText("Total $0.0019")).toBeInTheDocument();
});
