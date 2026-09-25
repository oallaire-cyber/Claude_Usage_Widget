// Talks to the Rust side inside the app; does nothing in the browser preview.

import type { UiState } from "./types";

export const inTauri = "__TAURI_INTERNALS__" in window;

export async function call<T = void>(cmd: string, args?: Record<string, unknown>): Promise<T | undefined> {
  if (!inTauri) return undefined;
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export async function onUiState(cb: (s: UiState) => void): Promise<void> {
  if (!inTauri) return;
  const { listen } = await import("@tauri-apps/api/event");
  await listen<UiState>("ui-state", (e) => cb(e.payload));
}

export async function onEvent(name: string, cb: () => void): Promise<void> {
  if (!inTauri) return;
  const { listen } = await import("@tauri-apps/api/event");
  await listen(name, () => cb());
}

export async function windowLabel(): Promise<string> {
  if (!inTauri) return "";
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow().label;
}
