import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { HISTORY_CHANGED, type Stats as StatsData, type Usage } from "../api";
import { Stats } from "./Stats";

const empty: Usage = {
  dictations: 0,
  durationInSeconds: 0,
  words: 0,
  transcriptionCostInUsd: null,
  transformationCostInUsd: null,
};

const usage: Usage = {
  dictations: 42,
  durationInSeconds: 3725,
  words: 523,
  transcriptionCostInUsd: 0.75,
  transformationCostInUsd: 0.125,
};

function mockBackend(firstHistoryAt: number | null, stats: (bucketCount: number) => StatsData) {
  const backend = { requests: [] as { bucketStarts: number[]; end: number }[], stats };
  mockIPC(
    (cmd, payload) => {
      switch (cmd) {
        case "first_history_at":
          return firstHistoryAt;
        case "get_stats": {
          const request = payload as { bucketStarts: number[]; end: number };
          backend.requests.push(request);
          return backend.stats(request.bucketStarts.length);
        }
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
  return backend;
}

const withTotal = (total: Usage) => (bucketCount: number) => ({
  total,
  buckets: Array(bucketCount).fill(empty),
});

const lastRequest = (backend: ReturnType<typeof mockBackend>) =>
  backend.requests[backend.requests.length - 1];

const stat = (label: string) =>
  screen.getByText(label, { selector: ".stat-label" }).closest("dt")?.nextElementSibling?.textContent;

const local = (month: number, day: number, hour = 0) => new Date(2026, month, day, hour).getTime();

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(2026, 9, 6, 15, 30));
});

afterEach(() => {
  vi.useRealTimers();
});

test("without history shows the empty state", async () => {
  mockBackend(null, withTotal(empty));

  render(<Stats />);

  expect(await screen.findByText("No dictations yet.")).toBeInTheDocument();
});

test("shows totals for the last 7 days by default", async () => {
  const backend = mockBackend(local(0, 1), withTotal(usage));

  render(<Stats />);

  await screen.findByText("Dictations", { selector: ".stat-label" });
  expect(lastRequest(backend)).toEqual({
    bucketStarts: Array.from({ length: 7 }, (_, i) => local(8, 30 + i)),
    end: local(9, 7),
  });
  expect(stat("Dictations")).toBe("42");
  expect(stat("Recorded")).toBe("1 h 2 min");
  expect(stat("Words")).toBe("523");
  expect(stat("Cost")).toBe("$0.8750");
  expect(screen.getByLabelText("About Cost")).toHaveAccessibleDescription(
    /\$0\.7500 transcription\s*\$0\.1250 transformation$/,
  );
});

test("without any cost shows a dash and no cost breakdown", async () => {
  mockBackend(local(0, 1), withTotal({ ...usage, transformationCostInUsd: null, transcriptionCostInUsd: null }));

  render(<Stats />);

  await screen.findByText("Cost", { selector: ".stat-label" });
  expect(stat("Cost")).toBe("—");
  expect(screen.getByLabelText("About Cost")).not.toHaveAccessibleDescription(/\$[\d.]+ transcription/);
});

test("recorded time below a minute is shown in seconds, below an hour in minutes", async () => {
  const backend = mockBackend(local(0, 1), withTotal({ ...usage, durationInSeconds: 42.4 }));

  render(<Stats />);

  await screen.findByText("Recorded", { selector: ".stat-label" });
  expect(stat("Recorded")).toBe("42 s");
  backend.stats = withTotal({ ...usage, durationInSeconds: 754 });
  await emit(HISTORY_CHANGED);
  await vi.waitFor(() => expect(stat("Recorded")).toBe("13 min"));
});

test("today is split into hours", async () => {
  const user = userEvent.setup();
  const backend = mockBackend(local(0, 1), withTotal(usage));
  render(<Stats />);
  await screen.findByText("Dictations", { selector: ".stat-label" });

  await user.click(screen.getByRole("radio", { name: "Today" }));

  await vi.waitFor(() => expect(lastRequest(backend)?.bucketStarts).toHaveLength(24));
  expect(lastRequest(backend)).toEqual({
    bucketStarts: Array.from({ length: 24 }, (_, hour) => local(9, 6, hour)),
    end: local(9, 7),
  });
});

test("30 days are split into days", async () => {
  const user = userEvent.setup();
  const backend = mockBackend(local(0, 1), withTotal(usage));
  render(<Stats />);
  await screen.findByText("Dictations", { selector: ".stat-label" });

  await user.click(screen.getByRole("radio", { name: "30 days" }));

  await vi.waitFor(() => expect(lastRequest(backend)?.bucketStarts).toHaveLength(30));
  expect(lastRequest(backend)).toEqual({
    bucketStarts: Array.from({ length: 30 }, (_, i) => local(8, 7 + i)),
    end: local(9, 7),
  });
});

test("all time is split into weeks starting on Monday for a short history", async () => {
  const user = userEvent.setup();
  const backend = mockBackend(local(9, 1, 10), withTotal(usage));
  render(<Stats />);
  await screen.findByText("Dictations", { selector: ".stat-label" });

  await user.click(screen.getByRole("radio", { name: "All time" }));

  await vi.waitFor(() => expect(lastRequest(backend)?.bucketStarts).toHaveLength(2));
  expect(lastRequest(backend)).toEqual({
    bucketStarts: [local(8, 28), local(9, 5)],
    end: local(9, 12),
  });
});

test("all time is split into months for a history longer than half a year", async () => {
  const user = userEvent.setup();
  const backend = mockBackend(local(0, 15, 10), withTotal(usage));
  render(<Stats />);
  await screen.findByText("Dictations", { selector: ".stat-label" });

  await user.click(screen.getByRole("radio", { name: "All time" }));

  await vi.waitFor(() => expect(lastRequest(backend)?.bucketStarts).toHaveLength(10));
  expect(lastRequest(backend)).toEqual({
    bucketStarts: Array.from({ length: 10 }, (_, month) => local(month, 1)),
    end: local(10, 1),
  });
});

test("the chart shows the chosen metric per bucket", async () => {
  const user = userEvent.setup();
  mockBackend(local(0, 1), (bucketCount) => ({
    total: usage,
    buckets: [
      ...Array(bucketCount - 1).fill(empty),
      {
        dictations: 2,
        durationInSeconds: 150,
        words: 40,
        transcriptionCostInUsd: 0.01,
        transformationCostInUsd: 0.002,
      },
    ],
  }));
  render(<Stats />);

  expect(await screen.findByRole("img", { name: /· 2\.5 min$/ })).toBeInTheDocument();
  await user.click(screen.getByRole("radio", { name: "Dictations" }));
  expect(screen.getByRole("img", { name: /· 2 dictations$/ })).toBeInTheDocument();
  await user.click(screen.getByRole("radio", { name: "Cost" }));
  expect(screen.getByRole("img", { name: /· \$0\.0120$/ })).toBeInTheDocument();
});

test("hovering a bar shows all of its bucket's numbers", async () => {
  const user = userEvent.setup();
  mockBackend(local(0, 1), (bucketCount) => ({
    total: usage,
    buckets: [
      ...Array(bucketCount - 1).fill(empty),
      {
        dictations: 2,
        durationInSeconds: 150,
        words: 40,
        transcriptionCostInUsd: 0.01,
        transformationCostInUsd: 0.002,
      },
    ],
  }));
  render(<Stats />);
  const bar = await screen.findByRole("img", { name: /· 2\.5 min$/ });
  expect(screen.queryByRole("tooltip", { name: /2\.5 min/ })).toBeNull();

  await user.hover(bar);

  expect(screen.getByRole("tooltip", { name: /2\.5 min/ })).toHaveTextContent(
    /2\.5 min · 2 dictations · \$0\.0120$/,
  );
  await user.unhover(bar);
  expect(screen.queryByRole("tooltip", { name: /2\.5 min/ })).toBeNull();
});

test("days without dictations show nothing on hover", async () => {
  const user = userEvent.setup();
  mockBackend(local(0, 1), (bucketCount) => ({
    total: usage,
    buckets: [
      { ...empty, dictations: 1, durationInSeconds: 30 },
      ...Array(bucketCount - 1).fill(empty),
    ],
  }));
  render(<Stats />);
  const [busy, idle] = await screen.findAllByRole("img", { name: /min$/ });
  await user.hover(busy);
  expect(screen.getByRole("tooltip", { name: /0\.5 min/ })).toBeInTheDocument();

  await user.hover(idle);

  expect(screen.queryByRole("tooltip", { name: /min/ })).toBeNull();
});

test("every total has an explanation", async () => {
  mockBackend(local(0, 1), withTotal(usage));
  render(<Stats />);
  await screen.findByText("Dictations", { selector: ".stat-label" });

  for (const label of ["Dictations", "Recorded", "Words", "Cost"]) {
    expect(screen.getByLabelText(`About ${label}`)).toHaveAccessibleDescription(/\w/);
  }
});

test("reloads when the history changes", async () => {
  const backend = mockBackend(local(0, 1), withTotal(usage));
  render(<Stats />);
  await screen.findByText("Dictations", { selector: ".stat-label" });

  backend.stats = withTotal({ ...usage, dictations: 43 });
  await emit(HISTORY_CHANGED);

  await vi.waitFor(() => expect(stat("Dictations")).toBe("43"));
});
