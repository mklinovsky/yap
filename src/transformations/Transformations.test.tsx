import { mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test } from "vitest";
import type { Transformations as TransformationsData } from "../api";
import { Transformations } from "./Transformations";

const stored: TransformationsData = {
  enabled: true,
  baseUrl: "https://llm.example.com/v1",
  reuseApiKey: true,
  defaultModel: "gpt-6-luna",
  items: [
    {
      id: "t1",
      name: "Fix grammar",
      systemPrompt: "Fix grammar.",
      userTemplate: "",
      model: null,
      shortcut: "Ctrl+Alt+G",
    },
    {
      id: "t2",
      name: "Translate",
      systemPrompt: "Translate to English.",
      userTemplate: "Text: {{transcript}}",
      model: "gpt-5",
      shortcut: null,
    },
  ],
};

let preview: string | null;

function mockBackend(overrides: Record<string, (payload: unknown) => unknown> = {}) {
  preview = null;
  const calls: { cmd: string; payload: unknown }[] = [];
  const handlers: Record<string, (payload: unknown) => unknown> = {
    get_transformations: () => stored,
    get_settings: () => ({ baseUrl: "https://api.openai.com/v1", shortcut: "Shift+Super+Semicolon" }),
    transform_api_key_preview: () => preview,
    save_transformations: () => null,
    set_transform_api_key: () => null,
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

const savedPayload = (calls: { cmd: string; payload: unknown }[]) => {
  const saves = calls.filter((c) => c.cmd === "save_transformations");
  return (saves[saves.length - 1].payload as { transformations: TransformationsData })
    .transformations;
};

test("shows the connection and the list of transformations", async () => {
  mockBackend();

  render(<Transformations />);

  expect(await screen.findByLabelText("Enabled")).toBeChecked();
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://llm.example.com/v1");
  expect(screen.getByLabelText("Use transcription API key")).toBeChecked();
  expect(screen.queryByLabelText("API key")).toBeNull();
  expect(screen.getByLabelText("Default model")).toHaveValue("gpt-6-luna");
  expect(screen.getByRole("button", { name: "Edit Fix grammar" })).toHaveTextContent(
    "Fix grammardefault modelCtrl+Alt+G",
  );
  expect(screen.getByRole("button", { name: "Edit Translate" })).toHaveTextContent(
    "Translategpt-5No shortcut",
  );
});

test("base URL starts as the transcription URL and is stored on the first save", async () => {
  const user = userEvent.setup();
  const calls = mockBackend({ get_transformations: () => ({ ...stored, baseUrl: null }) });
  render(<Transformations />);

  expect(await screen.findByLabelText("Base URL")).toHaveValue("https://api.openai.com/v1");
  await user.click(screen.getByLabelText("Enabled"));
  await user.click(screen.getByRole("button", { name: "Save" }));

  expect(await screen.findByText("Saved")).toBeInTheDocument();
  expect(savedPayload(calls)).toEqual({
    ...stored,
    enabled: false,
    baseUrl: "https://api.openai.com/v1",
  });
});

test("a separate API key can be entered once the transcription key is not reused", async () => {
  const user = userEvent.setup();
  const calls = mockBackend({
    set_transform_api_key: (payload) => {
      preview = `${(payload as { key: string }).key.slice(0, 4)}••••••••`;
      return null;
    },
  });
  render(<Transformations />);

  await user.click(await screen.findByLabelText("Use transcription API key"));
  const key = screen.getByLabelText("API key");
  expect(key).toHaveAttribute("placeholder", "Not set");
  await user.type(key, "sk-llm");
  await user.click(screen.getByRole("button", { name: "Save" }));

  await screen.findByText("Saved");
  expect(savedPayload(calls).reuseApiKey).toBe(false);
  expect(calls.find((c) => c.cmd === "set_transform_api_key")?.payload).toEqual({ key: "sk-llm" });
  expect(key).toHaveValue("");
  expect(key).toHaveAttribute("placeholder", "sk-l••••••••");
});

test("adds a transformation in the editor", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Transformations />);

  await user.click(await screen.findByRole("button", { name: "Add transformation" }));
  const name = screen.getByLabelText("Name");
  await user.clear(name);
  await user.type(name, "Bullet points");
  await user.type(screen.getByLabelText("System prompt"), "Make a bullet list.");
  await user.type(screen.getByLabelText("User message"), "Notes: {{{{transcript}}");
  await user.type(screen.getByLabelText("Model"), "gpt-5-nano");
  await user.click(screen.getByRole("button", { name: "Save" }));

  await screen.findByText("Saved");
  const added = savedPayload(calls).items[2];
  expect({ ...added, id: typeof added.id }).toEqual({
    id: "string",
    name: "Bullet points",
    systemPrompt: "Make a bullet list.",
    userTemplate: "Notes: {{transcript}}",
    model: "gpt-5-nano",
    shortcut: null,
  });
});

test("edits made in the editor are kept when going back, and one Save stores them", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Transformations />);

  await user.click(await screen.findByRole("button", { name: "Edit Translate" }));
  expect(screen.getByLabelText("Model")).toHaveValue("gpt-5");
  expect(screen.getByLabelText("Model")).toHaveAttribute("placeholder", "gpt-6-luna (default)");
  await user.clear(screen.getByLabelText("Model"));
  await user.click(screen.getByRole("button", { name: "Back to transformations" }));

  expect(screen.getByText("Unsaved changes")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText("Saved");
  expect(savedPayload(calls).items[1].model).toBeNull();
});

test("records and removes a transformation's shortcut", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Transformations />);

  await user.click(await screen.findByRole("button", { name: "Edit Translate" }));
  await user.click(screen.getByRole("button", { name: /Shortcut/ }));
  await user.keyboard("{Control>}{Alt>}[KeyE]{/Alt}{/Control}");
  await user.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText("Saved");
  expect(savedPayload(calls).items[1].shortcut).toBe("Ctrl+Alt+E");

  await user.click(screen.getByRole("button", { name: "Remove shortcut" }));
  await user.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText("Saved");
  expect(savedPayload(calls).items[1].shortcut).toBeNull();
});

test("deletes a transformation", async () => {
  const user = userEvent.setup();
  const calls = mockBackend();
  render(<Transformations />);

  await user.click(await screen.findByRole("button", { name: "Edit Fix grammar" }));
  await user.click(screen.getByRole("button", { name: "Delete transformation" }));

  expect(screen.queryByRole("button", { name: "Edit Fix grammar" })).toBeNull();
  await user.click(screen.getByRole("button", { name: "Save" }));
  await screen.findByText("Saved");
  expect(savedPayload(calls).items.map((item) => item.name)).toEqual(["Translate"]);
});

test("shows the backend error when saving is rejected", async () => {
  const user = userEvent.setup();
  mockBackend({
    save_transformations: () => {
      throw '"Translate": the user message must contain {{transcript}}.';
    },
  });
  render(<Transformations />);

  await user.click(await screen.findByLabelText("Enabled"));
  await user.click(screen.getByRole("button", { name: "Save" }));

  expect(
    await screen.findByText('"Translate": the user message must contain {{transcript}}.'),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
});

test("Save is enabled only while there are unsaved changes", async () => {
  const user = userEvent.setup();
  mockBackend();
  render(<Transformations />);

  const save = await screen.findByRole("button", { name: "Save" });
  expect(save).toBeDisabled();
  await user.type(screen.getByLabelText("Default model"), "-2");
  expect(save).toBeEnabled();
  await user.click(save);

  await screen.findByText("Saved");
  expect(save).toBeDisabled();
});
