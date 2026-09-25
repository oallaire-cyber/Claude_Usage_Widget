// The pinned widget: small, always on top, draggable, semi-transparent.

import { closeIcon, expandIcon } from "./icons";
import { duration, pct, t } from "./i18n";
import { call } from "./host";
import { rings } from "./rings";
import type { Lang, UiState, WindowView } from "./types";

function line(w: WindowView | undefined, name: string, now: number, lang: Lang): HTMLElement {
  const e = document.createElement("div");
  e.className = "pin-line";
  const n = document.createElement("span");
  n.className = "pin-name";
  n.textContent = name;
  const v = document.createElement("span");
  v.className = `pin-pct tone-text-${w?.tone ?? "grey"}`;
  const r = document.createElement("span");
  r.className = "pin-reset";
  if (!w) {
    v.textContent = "—";
  } else if (w.status === "window_reset") {
    v.textContent = pct(0, lang);
    r.textContent = "↺";
  } else {
    v.textContent = pct(w.display_percentage, lang);
    if (w.resets_at != null) {
      const left = w.resets_at - now;
      r.textContent = duration(left, lang);
    }
  }
  e.append(n, v, r);
  return e;
}

export function renderPinned(root: HTMLElement, ui: UiState): void {
  const s = t(ui.lang);
  const v = ui.view;
  const card = document.createElement("div");
  card.className = `card pinned status-${v.status}`;
  card.setAttribute("data-tauri-drag-region", "");

  const ring = rings(v.windows.slice(0, 2), { size: 52, stroke: 6, gap: 2, minRings: 2, markers: false });
  ring.setAttribute("data-tauri-drag-region", "");
  const text = document.createElement("div");
  text.className = "pin-text";
  text.setAttribute("data-tauri-drag-region", "");
  const session = v.windows.find((w) => w.id === "five_hour");
  const week = v.windows.find((w) => w.id === "seven_day");
  text.append(line(session, s.session, v.now, ui.lang), line(week, s.week, v.now, ui.lang));
  for (const c of text.children) c.setAttribute("data-tauri-drag-region", "");

  const tools = document.createElement("div");
  tools.className = "pin-tools";
  const open = document.createElement("button");
  open.className = "icon-btn tiny";
  open.innerHTML = expandIcon;
  open.title = s.openCard;
  open.setAttribute("aria-label", s.openCard);
  open.addEventListener("click", () => void call("open_popup"));
  const unpin = document.createElement("button");
  unpin.className = "icon-btn tiny";
  unpin.innerHTML = closeIcon;
  unpin.title = s.unpin;
  unpin.setAttribute("aria-label", s.unpin);
  unpin.addEventListener("click", () => void call("set_pinned", { pinned: false }));
  tools.append(open, unpin);

  card.append(ring, text, tools);
  root.replaceChildren(card);
}
