import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { STATUS_CHANGED } from "./api";
import { TryIt } from "./TryIt";

function mockBackend() {
  const calls: string[] = [];
  mockIPC(
    (cmd) => {
      calls.push(cmd);
      return cmd === "get_status" ? { state: "idle", canRetry: false } : null;
    },
    { shouldMockEvents: true },
  );
  return calls;
}

test("starting a recording toggles it and returns focus to the scratch pad", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<TryIt />);

  await user.click(await screen.findByRole("button", { name: "Start recording" }));

  expect(calls).toContain("toggle_recording");
  expect(screen.getByLabelText("Scratch pad")).toHaveFocus();
});

test("follows the live dictation status", async () => {
  mockBackend();
  render(<TryIt />);
  await screen.findByRole("button", { name: "Start recording" });

  await emit(STATUS_CHANGED, { state: "recording" });
  expect(await screen.findByRole("button", { name: "Stop recording" })).toBeEnabled();

  await emit(STATUS_CHANGED, { state: "transcribing" });
  expect(await screen.findByRole("button", { name: "Transcribing…" })).toBeDisabled();

  await emit(STATUS_CHANGED, { state: "error", message: "HTTP 401: Incorrect API key provided" });
  expect(await screen.findByText("HTTP 401: Incorrect API key provided")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Start recording" })).toBeEnabled();
});

test("offers to retry a failed transcription and pastes the result into the scratch pad", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<TryIt />);
  await screen.findByRole("button", { name: "Start recording" });
  expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();

  await emit(STATUS_CHANGED, { state: "error", message: "network error: timed out", canRetry: true });
  await user.click(await screen.findByRole("button", { name: "Retry" }));

  expect(calls).toContain("retry_recording");
  expect(screen.getByLabelText("Scratch pad")).toHaveFocus();
});
