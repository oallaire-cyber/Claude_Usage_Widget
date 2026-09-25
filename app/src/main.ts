// Entry point for all three windows (popup, pinned, settings), chosen by window label.
// Outside the app (plain browser), a preview mode renders fixture states:
//   index.html?view=popup|pinned|settings&state=normal|warn|crit|stale|reset|nodata|extra|desktop
//             &theme=light|dark&lang=en|fr

import { call, inTauri, onEvent, onUiState, windowLabel } from "./host";
import { renderPinned } from "./pinned";
import { renderPopup } from "./popup";
import { renderSettings } from "./settings";
import type { Lang, SettingsDto, UiState, View } from "./types";

const root = document.querySelector<HTMLElement>("#app")!;
const DEFAULT_ACCENT = "#0078d4";

function applyTheme(theme: "light" | "dark" | null): void {
  const dark = theme ? theme === "dark" : window.matchMedia("(prefers-color-scheme: dark)").matches;
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

function applyAccent(accent: string | null): void {
  document.documentElement.style.setProperty("--accent", accent ?? DEFAULT_ACCENT);
}

function render(kind: string, ui: UiState): void {
  applyAccent(ui.accent);
  document.documentElement.lang = ui.lang;
  if (kind === "pinned") renderPinned(root, ui);
  else renderPopup(root, ui);
  if (kind === "popup") reportHeight();
}

let lastHeight = 0;
function reportHeight(): void {
  requestAnimationFrame(() => {
    const h = Math.ceil(root.getBoundingClientRect().height);
    if (h > 0 && h !== lastHeight) {
      lastHeight = h;
      void call("popup_resize", { height: h });
    }
  });
}

async function startApp(): Promise<void> {
  const kind = (await windowLabel()) || "popup";
  document.body.dataset.view = kind;
  applyTheme(null);
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => applyTheme(null));

  if (kind === "settings") {
    const dto = await call<SettingsDto>("get_settings");
    if (dto) renderSettings(root, dto);
    await onUiState((ui) => applyAccent(ui.accent));
    return;
  }

  let current = await call<UiState | null>("get_ui_state");
  if (current) render(kind, current);
  await onUiState((ui) => {
    current = ui;
    render(kind, ui);
  });
  if (kind === "popup") {
    await onEvent("popup-shown", () => current && render(kind, current));
    window.addEventListener("keydown", (e) => {
      if (e.key === "Escape") void call("hide_popup");
    });
  }
}

async function startPreview(): Promise<void> {
  const q = new URLSearchParams(location.search);
  const kind = q.get("view") ?? "popup";
  const lang = (q.get("lang") === "fr" ? "fr" : "en") as Lang;
  const theme = q.get("theme") === "dark" ? "dark" : "light";
  document.body.dataset.view = kind;
  document.body.dataset.preview = "";
  applyTheme(theme);

  const views = (await import("./preview/views.json")).default as unknown as Record<string, View>;
  if (kind === "settings") {
    renderSettings(
      root,
      {
        settings: {
          language: "auto",
          notifications: {
            enabled: true,
            thresholds: { five_hour: [80, 95], seven_day: [75, 90] },
            notify_reset: true,
          },
          pinned: false,
          pinned_position: null,
        },
        autostart: true,
        lang,
      },
      true,
    );
    return;
  }
  const view = views[q.get("state") ?? "normal"] ?? views.normal;
  render(kind, { view, lang, accent: q.get("accent"), pinned: q.get("pinned") === "1" });
}

void (inTauri ? startApp() : startPreview());
