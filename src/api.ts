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
}

export interface InputDevice {
  id: string;
  name: string;
}

export interface HistoryEntry {
  id: number;
  text: string;
  createdAt: number;
  /** USD, when the endpoint reported it. */
  cost: number | null;
}

export type Status =
  | { state: "idle" }
  | { state: "recording" }
  | { state: "transcribing" }
  | { state: "error"; message: string };

export const HISTORY_CHANGED = "history-changed";
export const STATUS_CHANGED = "status-changed";

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  apiKeyPreview: () => invoke<string | null>("api_key_preview"),
  setApiKey: (key: string) => invoke<void>("set_api_key", { key }),
  listHistory: () => invoke<HistoryEntry[]>("list_history"),
  listInputDevices: () => invoke<InputDevice[]>("list_input_devices"),
  deleteHistory: (id: number) => invoke<void>("delete_history", { id }),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  getStatus: () => invoke<Status>("get_status"),
  toggleRecording: () => invoke<void>("toggle_recording"),
  pauseShortcut: () => invoke<void>("pause_shortcut"),
  resumeShortcut: () => invoke<void>("resume_shortcut"),
};
