import { invoke } from "@tauri-apps/api/core";

export type Mode = "hold" | "toggle";

export interface Settings {
  baseUrl: string;
  model: string;
  languages: string[];
  keywords: string[];
  inputDevice: string | null;
  shortcut: string;
  mode: Mode;
  sounds: boolean;
  maxMinutes: number;
}

export interface Transformation {
  id: string;
  name: string;
  systemPrompt: string;
  userTemplate: string;
  model: string | null;
  shortcut: string | null;
}

export interface Transformations {
  enabled: boolean;
  baseUrl: string | null;
  reuseApiKey: boolean;
  defaultModel: string;
  items: Transformation[];
}

export interface InputDevice {
  id: string;
  name: string;
}

export interface HistoryEntry {
  id: number;
  text: string;
  createdAt: number;
  costInUsd: number | null;
  durationInSeconds: number | null;
  sizeInBytes: number | null;
  encodeTimeInSeconds: number | null;
  transcribeTimeInSeconds: number | null;
  rawText: string | null;
  transformationName: string | null;
  transformError: string | null;
  transformCostInUsd: number | null;
  transformTimeInSeconds: number | null;
}

export type Status =
  | { state: "idle" }
  | { state: "recording" }
  | { state: "transcribing" }
  | { state: "transforming" }
  | { state: "error"; message: string };

export const HISTORY_CHANGED = "history-changed";
export const STATUS_CHANGED = "status-changed";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  apiKeyPreview: () => invoke<string | null>("api_key_preview"),
  setApiKey: (key: string) => invoke<void>("set_api_key", { key }),
  getTransformations: () => invoke<Transformations>("get_transformations"),
  saveTransformations: (transformations: Transformations) =>
    invoke<void>("save_transformations", { transformations }),
  transformApiKeyPreview: () => invoke<string | null>("transform_api_key_preview"),
  setTransformApiKey: (key: string) => invoke<void>("set_transform_api_key", { key }),
  getOpenAtLogin: () => invoke<boolean>("get_open_at_login"),
  setOpenAtLogin: (enabled: boolean) => invoke<void>("set_open_at_login", { enabled }),
  listHistory: () => invoke<HistoryEntry[]>("list_history"),
  listInputDevices: () => invoke<InputDevice[]>("list_input_devices"),
  deleteHistory: (id: number) => invoke<void>("delete_history", { id }),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  getStatus: () => invoke<Status>("get_status"),
  toggleRecording: () => invoke<void>("toggle_recording"),
  pauseShortcut: () => invoke<void>("pause_shortcut"),
  resumeShortcut: () => invoke<void>("resume_shortcut"),
};
