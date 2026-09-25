// The popup card opened from the tray icon.

import { closeIcon, gearIcon, pinIcon } from "./icons";
import { clock, duration, paceLine, pct, resetLine, t, windowLabel } from "./i18n";
import { call } from "./host";
import { rings } from "./rings";
import type { UiState, WindowView } from "./types";

function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  cls?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
}

function iconButton(svg: string, label: string, onClick: () => void, pressed?: boolean): HTMLButtonElement {
  const b = h("button", "icon-btn");
  b.type = "button";
  b.innerHTML = svg; // static markup from icons.ts only
  b.title = label;
  b.setAttribute("aria-label", label);
  if (pressed != null) b.setAttribute("aria-pressed", String(pressed));
  b.addEventListener("click", onClick);
  return b;
}

function row(w: WindowView, now: number, ui: UiState): HTMLElement {
  const r = h("div", `row status-${w.status}`);
  const head = h("div", "row-head");
  const dot = h("span", `dot tone-${w.tone}`);
  const name = h("span", "row-name", windowLabel(w.id, ui.lang, true));
  const value = h("span", "row-pct", w.status === "window_reset" ? pct(0, ui.lang) : pct(w.display_percentage, ui.lang));
  head.append(dot, name, value);
  r.append(head, h("div", "row-reset", resetLine(w, now, ui.lang)));
  const pace = paceLine(w, now, ui.lang);
  if (pace) r.append(h("div", `row-pace pace-${w.tone}`, pace));
  return r;
}

export function renderPopup(root: HTMLElement, ui: UiState): void {
  const s = t(ui.lang);
  const v = ui.view;
  const card = h("div", "card popup");

  const header = h("header", "card-head");
  header.append(h("h1", "card-title", s.title));
  const tools = h("div", "tools");
  tools.append(
    iconButton(pinIcon, ui.pinned ? s.unpin : s.pin, () => void call("set_pinned", { pinned: !ui.pinned }), ui.pinned),
    iconButton(gearIcon, s.settings, () => void call("open_settings")),
    iconButton(closeIcon, s.close, () => void call("hide_popup")),
  );
  header.append(tools);
  card.append(header);

  const hero = h("section", "hero");
  const ringBox = h("div", "ring-box");
  ringBox.append(rings(v.windows, { size: 176, stroke: 13, gap: 5, minRings: 2 }));
  const centre = h("div", "ring-centre");
  const session = v.windows.find((w) => w.id === "five_hour");
  if (session) {
    centre.append(
      h("div", `big tone-text-${session.tone}`, pct(session.display_percentage, ui.lang)),
      h("div", "small", session.status === "window_reset" ? s.sessionReset : s.session),
    );
  } else {
    centre.append(h("div", "big muted", "—"));
  }
  ringBox.append(centre);
  hero.append(ringBox);
  card.append(hero);

  if (v.status === "no_data" || v.windows.length === 0) {
    const empty = h("section", "empty");
    empty.append(h("div", "empty-title", s.noDataTitle), h("p", "empty-body", s.noDataBody));
    card.append(empty);
  } else {
    // Measured from the oldest stale window (the "data as of" time is the freshest one).
    const staleSince = v.windows.filter((w) => w.status === "stale").map((w) => w.as_of);
    if (v.status === "stale" && staleSince.length) {
      card.append(h("div", "banner", s.staleBanner(duration(v.now - Math.min(...staleSince), ui.lang))));
    }
    const list = h("section", "rows");
    for (const w of v.windows) list.append(row(w, v.now, ui));
    card.append(list);
  }

  const foot = h("footer", "card-foot");
  if (v.as_of != null) foot.append(h("div", "", s.asOf(clock(v.as_of, ui.lang))));
  if (v.source) foot.append(h("div", "muted", s.source(s.sources[v.source])));
  if (foot.childElementCount) card.append(foot);

  root.replaceChildren(card);
}
