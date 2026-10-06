import { mockIPC } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import type { Settings as SettingsData } from "./api";
import { Settings } from "./Settings";

const stored: SettingsData = {
  baseUrl: "https://api.groq.com/openai/v1",
  model: "whisper-large-v3",
  languages: ["sk"],
  keywords: ["Tauri"],
  inputDevice: null,
  shortcut: "Ctrl+Shift+D",
  mode: "toggle",
  sounds: false,
  maxMinutes: 5,
};

let preview: string | null;

function mockBackend(overrides: Record<string, (payload: unknown) => unknown> = {}) {
  preview = "sk-p••••••••";
  const calls: { cmd: string; payload: unknown }[] = [];
  const handlers: Record<string, (payload: unknown) => unknown> = {
    get_settings: () => stored,
    api_key_preview: () => preview,
    list_input_devices: () => [
      { id: "coreaudio:BuiltIn", name: "MacBook Pro Microphone" },
      { id: "coreaudio:USB", name: "USB Microphone" },
    ],
    save_settings: () => null,
    set_api_key: () => null,
    get_open_at_login: () => false,
    set_open_at_login: () => null,
    pause_shortcut: () => null,
    resume_shortcut: () => null,
    ...overrides,
  };
  mockIPC((cmd, payload) => {
    calls.push({ cmd, payload });
    return handlers[cmd]?.(payload);
  });
  return calls;
}

test("shows the stored settings", async () => {
  mockBackend();

  render(<Settings />);

  expect(await screen.findByLabelText("Base URL")).toHaveValue("https://api.groq.com/openai/v1");
  expect(screen.getByLabelText("Model")).toHaveValue("whisper-large-v3");
  expect(screen.getByRole("listitem", { name: "Slovak" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Shortcut/ })).toHaveTextContent("Ctrl+Shift+D");
  expect(screen.getByLabelText("Toggle")).toBeChecked();
  expect(screen.getByLabelText("Sounds")).not.toBeChecked();
});

test("saves edited settings", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  const model = await screen.findByLabelText("Model");
  await user.clear(model);
  await user.type(model, "gpt-4o-transcribe");
  await user.click(screen.getByLabelText("Hold"));
  await user.click(screen.getByRole("button", { name: "Save" }));

  expect(await screen.findByText("Saved")).toBeInTheDocument();
  expect(calls.find((c) => c.cmd === "save_settings")?.payload).toEqual({
    settings: { ...stored, model: "gpt-4o-transcribe", mode: "hold" },
  });
});

test("shows only a masked preview of the stored key, refreshed after saving a new one", async () => {
  const user = userEvent.setup();
  const calls = mockBackend({
    set_api_key: (payload) => {
      preview = `${(payload as { key: string }).key.slice(0, 4)}••••••••`;
      return null;
    },
  });
  render(<Settings />);

  const key = await screen.findByLabelText("API key");
  expect(key).toHaveAttribute("placeholder", "sk-p••••••••");
  expect(key).toHaveValue("");
  await user.type(key, "gsk-new");
  await user.click(screen.getByRole("button", { name: "Save" }));

  await screen.findByText("Saved");
  expect(calls.find((c) => c.cmd === "set_api_key")?.payload).toEqual({ key: "gsk-new" });
  expect(key).toHaveValue("");
  expect(key).toHaveAttribute("placeholder", "gsk-••••••••");
});

test("leaves the stored API key alone when the field is empty", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  await user.type(await screen.findByLabelText("Model"), "-2");
  await user.click(screen.getByRole("button", { name: "Save" }));

  await screen.findByText("Saved");
  expect(calls.some((c) => c.cmd === "set_api_key")).toBe(false);
});

test("shows the backend error when saving is rejected", async () => {
  const user = userEvent.setup();
  mockBackend({
    save_settings: () => {
      throw 'Invalid shortcut "Ctrl+Nope"';
    },
  });
  render(<Settings />);

  await user.type(await screen.findByLabelText("Model"), "-2");
  await user.click(screen.getByRole("button", { name: "Save" }));

  expect(await screen.findByText('Invalid shortcut "Ctrl+Nope"')).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
});

test("saves vocabulary as a list of trimmed terms", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  const vocabulary = await screen.findByLabelText("Vocabulary");
  expect(vocabulary).toHaveValue("Tauri");
  await user.clear(vocabulary);
  await user.type(vocabulary, "Tauri,  rusqlite , ,Kubernetes");
  await user.click(screen.getByRole("button", { name: "Save" }));

  await screen.findByText("Saved");
  expect(calls.find((c) => c.cmd === "save_settings")?.payload).toEqual({
    settings: { ...stored, keywords: ["Tauri", "rusqlite", "Kubernetes"] },
  });
});

test("records a new shortcut from pressed keys while the global shortcut is paused", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  await user.click(await screen.findByRole("button", { name: /Shortcut/ }));
  expect(calls.some((c) => c.cmd === "pause_shortcut")).toBe(true);
  await user.keyboard("{Control>}{Alt>}[KeyK]{/Alt}{/Control}");
  expect(calls.some((c) => c.cmd === "resume_shortcut")).toBe(true);
  await user.click(screen.getByRole("button", { name: "Save" }));

  await screen.findByText("Saved");
  expect(calls.find((c) => c.cmd === "save_settings")?.payload).toEqual({
    settings: { ...stored, shortcut: "Ctrl+Alt+K" },
  });
});

test("picks several languages, and no languages means auto-detect", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  await user.selectOptions(await screen.findByLabelText("Add language"), "English");
  await user.click(screen.getByRole("button", { name: "Remove Slovak" }));
  await user.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText("Saved");
  await user.click(screen.getByRole("button", { name: "Remove English" }));
  expect(screen.getByText("Auto-detect")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Save" }));

  const saved = calls
    .filter((c) => c.cmd === "save_settings")
    .map((c) => (c.payload as { settings: SettingsData }).settings.languages);
  expect(saved).toEqual([["en"], []]);
});

test("the eye button reveals only the key being typed", async () => {
  const user = userEvent.setup();
  mockBackend();
  render(<Settings />);

  const key = await screen.findByLabelText("API key");
  expect(screen.queryByRole("button", { name: "Show API key" })).not.toBeInTheDocument();
  await user.type(key, "sk-new");
  await user.click(screen.getByRole("button", { name: "Show API key" }));
  expect(key).toHaveAttribute("type", "text");
  await user.click(screen.getByRole("button", { name: "Hide API key" }));

  expect(key).toHaveAttribute("type", "password");
});

test("records the shortcut even when clicking does not focus the recorder, as in WebKit", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  fireEvent.click(await screen.findByRole("button", { name: /Shortcut/ }));
  expect(document.activeElement).toBe(document.body);
  await user.keyboard("{Control>}{Alt>}[Space]{/Alt}{/Control}");
  await user.click(screen.getByRole("button", { name: "Save" }));

  await screen.findByText("Saved");
  expect(calls.find((c) => c.cmd === "save_settings")?.payload).toEqual({
    settings: { ...stored, shortcut: "Ctrl+Alt+Space" },
  });
});

test("picks the input device, with the system default listed first", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  const input = await screen.findByLabelText("Input");
  await screen.findByRole("option", { name: "USB Microphone" });
  expect(within(input).getAllByRole("option").map((o) => o.textContent)).toEqual([
    "System default",
    "MacBook Pro Microphone",
    "USB Microphone",
  ]);
  await user.selectOptions(input, "USB Microphone");
  await user.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText("Saved");
  await user.selectOptions(input, "System default");
  await user.click(screen.getByRole("button", { name: "Save" }));

  const saved = calls
    .filter((c) => c.cmd === "save_settings")
    .map((c) => (c.payload as { settings: SettingsData }).settings.inputDevice);
  expect(saved).toEqual(["coreaudio:USB", null]);
});

test("picks the max recording length from minute presets", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  const maxLength = await screen.findByLabelText("Max length");
  expect(maxLength).toHaveDisplayValue("5 min");
  expect(within(maxLength).getAllByRole("option").map((o) => o.textContent)).toEqual([
    "1 min",
    "2 min",
    "5 min",
    "10 min",
    "15 min",
    "20 min",
  ]);
  await user.selectOptions(maxLength, "10 min");
  await user.click(screen.getByRole("button", { name: "Save" }));

  expect(await screen.findByText("Saved")).toBeInTheDocument();
  expect(calls.find((c) => c.cmd === "save_settings")?.payload).toEqual({
    settings: { ...stored, maxMinutes: 10 },
  });
});

test("shows whether yap opens at login", async () => {
  mockBackend({ get_open_at_login: () => true });

  render(<Settings />);

  expect(await screen.findByLabelText("Open at login")).toBeChecked();
});

test("turns on opening at login when saved", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  await user.click(await screen.findByLabelText("Open at login"));
  await user.click(screen.getByRole("button", { name: "Save" }));

  expect(await screen.findByText("Saved")).toBeInTheDocument();
  expect(calls.find((c) => c.cmd === "set_open_at_login")?.payload).toEqual({ enabled: true });
});

test("leaves the login item alone when it was not changed", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Settings />);

  await screen.findByLabelText("Open at login");
  await user.type(screen.getByLabelText("Model"), "-2");
  await user.click(screen.getByRole("button", { name: "Save" }));

  await screen.findByText("Saved");
  expect(calls.some((c) => c.cmd === "set_open_at_login")).toBe(false);
});

test("Save is enabled only while there are unsaved changes", async () => {
  const user = userEvent.setup();
  mockBackend();
  render(<Settings />);

  const save = await screen.findByRole("button", { name: "Save" });
  expect(save).toBeDisabled();
  await user.click(screen.getByLabelText("Sounds"));
  expect(save).toBeEnabled();
  expect(screen.getByText("Unsaved changes")).toBeInTheDocument();
  await user.click(save);

  await screen.findByText("Saved");
  expect(save).toBeDisabled();
});
