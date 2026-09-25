// Mirrors the Rust types sent by the app (cuw_core::view::View, engine::UiState, settings::Settings).

export type Status = "ok" | "stale" | "window_reset" | "no_data";
export type Tone = "blue" | "amber" | "red" | "grey";
export type Source = "status_line" | "claude_code_cache" | "desktop_history";
export type Lang = "en" | "fr";

export interface Pace {
  elapsed_fraction: number;
  pace_ratio: number | null;
  status: "on_track" | "ahead" | "will_exhaust";
  projected_exhaustion_at: number | null;
  basis: "history" | "average";
  guarded: boolean;
}

export interface WindowView {
  id: string;
  used_percentage: number;
  display_percentage: number;
  resets_at: number | null;
  as_of: number;
  source: Source;
  status: Status;
  pace: Pace | null;
  tone: Tone;
}

export interface View {
  now: number;
  status: Status;
  as_of: number | null;
  source: Source | null;
  windows: WindowView[];
}

export interface UiState {
  view: View;
  lang: Lang;
  accent: string | null;
  pinned: boolean;
}

export interface NotifyConfig {
  enabled: boolean;
  thresholds: Record<string, number[]>;
  notify_reset: boolean;
}

export interface Settings {
  language: "auto" | "en" | "fr";
  notifications: NotifyConfig;
  pinned: boolean;
  pinned_position: [number, number] | null;
}

export interface SettingsDto {
  settings: Settings;
  autostart: boolean;
  lang: Lang;
}
