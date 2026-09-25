// The settings window. Every change is applied and saved immediately (Windows 11 style).

import { t } from "./i18n";
import { call } from "./host";
import type { Lang, Settings, SettingsDto } from "./types";

function h<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text != null) e.textContent = text;
  return e;
}

export function parseThresholds(text: string): number[] | null {
  const parts = text
    .split(/[,;\s]+/)
    .map((x) => x.trim())
    .filter(Boolean);
  const nums = parts.map((x) => Number(x.replace("%", "")));
  if (nums.some((n) => !Number.isFinite(n) || n < 1 || n > 100)) return null;
  return [...new Set(nums)].sort((a, b) => a - b);
}

function toggle(label: string, checked: boolean, onChange: (v: boolean) => void, hint?: string): HTMLElement {
  const item = h("label", "set-item");
  const text = h("div", "set-text");
  text.append(h("div", "set-label", label));
  if (hint) text.append(h("div", "set-hint", hint));
  const input = h("input", "switch");
  input.type = "checkbox";
  input.checked = checked;
  input.setAttribute("role", "switch");
  input.addEventListener("change", () => onChange(input.checked));
  item.append(text, input);
  return item;
}

export function renderSettings(root: HTMLElement, dto: SettingsDto, preview = false): void {
  let settings: Settings = structuredClone(dto.settings);
  let autostart = dto.autostart;
  const lang: Lang = dto.lang;
  const s = t(lang);
  const status = h("div", "set-status");

  const save = async () => {
    if (preview) return;
    try {
      await call("save_settings", { settings, autostart });
      status.textContent = s.sSaved;
      status.classList.add("show");
      window.setTimeout(() => status.classList.remove("show"), 1400);
      if (settings.language !== dto.settings.language) {
        // Re-render in the new language.
        const next = await call<SettingsDto>("get_settings");
        if (next) renderSettings(root, next);
      }
    } catch (e) {
      status.textContent = String(e);
      status.classList.add("show", "error");
    }
  };

  const page = h("div", "settings");
  page.append(h("h1", "set-title", s.sTitle));

  // Language
  const langGroup = h("section", "set-group");
  const langItem = h("label", "set-item");
  const lt = h("div", "set-text");
  lt.append(h("div", "set-label", s.sLanguage), h("div", "set-hint", s.sLanguageHint));
  const select = h("select", "select");
  for (const [value, label] of [
    ["auto", s.sAuto],
    ["en", "English"],
    ["fr", "Français"],
  ] as const) {
    const o = h("option", "", label);
    o.value = value;
    o.selected = settings.language === value;
    select.append(o);
  }
  select.addEventListener("change", () => {
    settings.language = select.value as Settings["language"];
    void save();
  });
  langItem.append(lt, select);
  langGroup.append(langItem);

  // Notifications
  const notif = h("section", "set-group");
  notif.append(h("h2", "set-group-title", s.sNotifications));
  const fields = h("div", "set-sub");
  const threshold = (id: string, label: string) => {
    const item = h("label", "set-item");
    const tx = h("div", "set-text");
    tx.append(h("div", "set-label", label));
    const input = h("input", "text-input");
    input.type = "text";
    input.inputMode = "decimal";
    input.value = (settings.notifications.thresholds[id] ?? []).join(", ");
    input.addEventListener("change", () => {
      const v = parseThresholds(input.value);
      if (v === null) {
        input.setAttribute("aria-invalid", "true");
        input.title = s.sInvalid;
        return;
      }
      input.removeAttribute("aria-invalid");
      input.title = "";
      input.value = v.join(", ");
      settings = {
        ...settings,
        notifications: {
          ...settings.notifications,
          thresholds: { ...settings.notifications.thresholds, [id]: v },
        },
      };
      void save();
    });
    item.append(tx, input);
    return item;
  };
  fields.append(
    threshold("five_hour", s.sSessionAt),
    threshold("seven_day", s.sWeeklyAt),
    h("div", "set-hint pad", s.sThresholdHint),
    toggle(s.sNotifyReset, settings.notifications.notify_reset, (v) => {
      settings.notifications.notify_reset = v;
      void save();
    }),
  );
  const setFieldsEnabled = (on: boolean) => fields.classList.toggle("disabled", !on);
  setFieldsEnabled(settings.notifications.enabled);
  notif.append(
    toggle(s.sNotifyOn, settings.notifications.enabled, (v) => {
      settings.notifications.enabled = v;
      setFieldsEnabled(v);
      void save();
    }),
    fields,
  );

  // Startup
  const startup = h("section", "set-group");
  startup.append(
    toggle(
      s.sStartup,
      autostart,
      (v) => {
        autostart = v;
        void save();
      },
      s.sStartupHint,
    ),
  );

  page.append(langGroup, notif, startup, h("p", "set-privacy", s.sPrivacy), status);
  root.replaceChildren(page);
}
