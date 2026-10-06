import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import { HISTORY_CHANGED, type HistoryEntry } from "../api";
import { History } from "./History";

const detail = (card: HTMLElement, label: string) =>
  within(card).getByText(label, { selector: "dt" }).nextElementSibling?.textContent;

function mockBackend(entries: HistoryEntry[]) {
  const calls: { cmd: string; payload: unknown }[] = [];
  const backend = { calls, rows: [...entries] };
  mockIPC(
    (cmd, payload) => {
      calls.push({ cmd, payload });
      switch (cmd) {
        case "list_history": {
          const { beforeId, limit } = payload as { beforeId: number | null; limit: number };
          return backend.rows.filter((row) => beforeId === null || row.id < beforeId).slice(0, limit);
        }
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

const visibilityObservers = new Set<FakeIntersectionObserver>();

class FakeIntersectionObserver {
  constructor(private readonly callback: IntersectionObserverCallback) {}
  observe() {
    visibilityObservers.add(this);
  }
  disconnect() {
    visibilityObservers.delete(this);
  }
  reportVisible() {
    this.callback(
      [{ isIntersecting: true } as IntersectionObserverEntry],
      this as unknown as IntersectionObserver,
    );
  }
}

vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);

const scrollToEnd = () => visibilityObservers.forEach((observer) => observer.reportVisible());

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

const numbered = (count: number): HistoryEntry[] =>
  Array.from({ length: count }, (_, index) => ({
    ...entries[1],
    id: count - index,
    text: `Thought ${count - index}`,
  }));

test("loads older transcripts when the end of the list scrolls into view", async () => {
  mockBackend(numbered(120));
  render(<History />);
  expect(await screen.findAllByRole("listitem")).toHaveLength(50);

  scrollToEnd();
  await expect.poll(() => screen.getAllByRole("listitem")).toHaveLength(100);
  scrollToEnd();
  await expect.poll(() => screen.getAllByRole("listitem")).toHaveLength(120);

  expect(screen.getAllByRole("listitem")[119]).toHaveTextContent("Thought 1");
});

test("a new transcript keeps the older transcripts already loaded", async () => {
  const backend = mockBackend(numbered(60));
  render(<History />);
  await screen.findAllByRole("listitem");
  scrollToEnd();
  await expect.poll(() => screen.getAllByRole("listitem")).toHaveLength(60);

  backend.rows = [{ ...entries[1], id: 61, text: "Fresh dictation" }, ...backend.rows];
  await emit(HISTORY_CHANGED);

  await expect.poll(() => screen.getAllByRole("listitem")).toHaveLength(61);
  expect(screen.getAllByRole("listitem")[0]).toHaveTextContent("Fresh dictation");
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

test("details show the upload size and how long encoding and the request took", async () => {
  const user = userEvent.setup();
  mockBackend(entries);
  render(<History />);

  const [recorded] = await screen.findAllByRole("listitem");
  expect(within(recorded).queryByText("Timing")).toBeNull();
  await user.click(within(recorded).getByRole("button", { name: "Details" }));

  expect(detail(recorded, "Audio")).toBe("12.4 s · 205 KB");
  expect(detail(recorded, "Timing")).toBe("encode 0.03 s · transcription 4.21 s");
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

  expect(detail(screen.getByRole("listitem"), "Audio")).toBe("1:15 · 2.4 MB");
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

test("a transformed entry shows the raw transcript under the text", async () => {
  mockBackend([transformed]);

  render(<History />);

  const [card] = await screen.findAllByRole("listitem");
  expect(within(card).getByText("the meeting uh moved to thursday")).toBeVisible();
  expect(within(card).getByRole("button", { name: "Details" })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
});

test("details of a transformed entry show the cost split", async () => {
  const user = userEvent.setup();
  mockBackend([transformed]);
  render(<History />);

  const [card] = await screen.findAllByRole("listitem");
  await user.click(within(card).getByRole("button", { name: "Details" }));

  expect(detail(card, "Timing")).toBe("encode 0.03 s · transcription 4.21 s · LLM 1.12 s");
  expect(detail(card, "Cost")).toBe("$0.0012 transcription · $0.0007 LLM");
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

  expect(detail(card, "Error")).toBe("Transformation failed: HTTP 500: boom");
});

test("a cost too small to show at four decimals is shown as below a hundredth of a cent", async () => {
  const user = userEvent.setup();
  mockBackend([{ ...transformed, transformCostInUsd: 0.00002 }]);
  render(<History />);

  const [card] = await screen.findAllByRole("listitem");
  await user.click(within(card).getByRole("button", { name: "Details" }));

  expect(detail(card, "Cost")).toBe("$0.0012 transcription · <$0.0001 LLM");
});

afterEach(() => {
  vi.restoreAllMocks();
});

test("a transcript taller than its clamp can be expanded and collapsed", async () => {
  const user = userEvent.setup();
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(300);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(120);
  mockBackend([transformed]);
  render(<History />);

  const [card] = await screen.findAllByRole("listitem");
  await user.click(within(card).getByRole("button", { name: "Show more" }));
  expect(within(card).getByRole("button", { name: "Show less" })).toHaveAttribute(
    "aria-expanded",
    "true",
  );

  await user.click(within(card).getByRole("button", { name: "Show less" }));
  expect(within(card).getByRole("button", { name: "Show more" })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
});

test("a transcript that fits has nothing to expand", async () => {
  mockBackend([transformed]);

  render(<History />);

  const [card] = await screen.findAllByRole("listitem");
  expect(within(card).queryByRole("button", { name: "Show more" })).toBeNull();
});
