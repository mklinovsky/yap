import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import { App } from "./App";

function mockBackend() {
  mockIPC(
    (cmd) => {
      switch (cmd) {
        case "list_history":
          return [];
        case "get_settings":
          return {
            baseUrl: "https://api.openai.com/v1",
            model: "whisper-1",
            languages: [],
            keywords: [],
            inputDevice: null,
            shortcut: "Alt+Space",
            mode: "hold",
            sounds: true,
          };
        case "api_key_preview":
          return "sk-p••••••••";
        case "list_input_devices":
          return [];
        case "get_status":
          return { state: "idle" };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}

test("opens on the requested section and switches from the sidebar", async () => {
  const user = userEvent.setup();
  mockBackend();

  render(<App initialSection="history" />);
  expect(await screen.findByRole("heading", { name: "History" })).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Settings" }));

  expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "History" })).not.toBeInTheDocument();
});

