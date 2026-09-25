// Popup, pinned widget and settings text in English and French, and the time formatting they use.
// (The tray tooltip and notifications are worded in Rust: crates/core/src/text.rs.)

import type { Lang, Source, WindowView } from "./types";

const en = {
  title: "Claude usage",
  session: "Session",
  sessionReset: "Session reset",
  sessionLong: "Session (5 hours)",
  weekly: "Weekly",
  weeklyLong: "Weekly (7 days)",
  week: "Week",
  extraUsage: "Extra usage",
  spendLimit: "Spend limit",
  resetsIn: (d: string, at: string) => `Resets in ${d} · ${at}`,
  resetsOn: (at: string, d: string) => `Resets ${at} · in ${d}`,
  resetUnknown: "Reset time unknown",
  resetWaiting: "Reset — waiting for fresh data",
  noUpdateSince: (at: string) => `No update since ${at}`,
  desktopOnly: "Countdown and pace need Claude Code data",
  justStarted: "Window just started — too early to judge pace",
  lightUse: (elapsed: string) => `Light use so far · ${elapsed} of the window gone`,
  onTrack: (r: string) => `On track (pace ${r})`,
  ahead: (r: string) => `Ahead of even pace (${r}) · should last until reset`,
  willExhaust: (at: string) => `At this rate, 100% around ${at}`,
  exhausted: "Limit reached",
  evenPace: "Even-spend mark",
  asOf: (at: string) => `Data as of ${at} (updates while Claude Code runs)`,
  source: (s: string) => `Source: ${s}`,
  sources: {
    status_line: "Claude Code status line",
    claude_code_cache: "Claude Code cache",
    desktop_history: "Claude Desktop",
  } as Record<Source, string>,
  staleBanner: (ago: string) =>
    `Not updated for ${ago}. Values refresh when Claude Code or Claude Desktop runs.`,
  noDataTitle: "No usage data yet",
  noDataBody:
    "Install the status-line bridge (see the README), then send a message in Claude Code.",
  pin: "Pin widget",
  unpin: "Unpin widget",
  openCard: "Open usage card",
  settings: "Settings",
  close: "Close",
  // Settings view
  sTitle: "Settings",
  sLanguage: "Language",
  sLanguageHint: "Automatic follows the Windows display language.",
  sAuto: "Automatic",
  sNotifications: "Notifications",
  sNotifyOn: "Show usage alerts",
  sSessionAt: "Session alerts at (%)",
  sWeeklyAt: "Weekly alerts at (%)",
  sThresholdHint: "Comma-separated, e.g. 80, 95. Each alert fires once per window.",
  sNotifyReset: "Tell me when a limit I was warned about resets",
  sStartup: "Start with Windows",
  sStartupHint: "Starts quietly in the notification area when you sign in.",
  sSaved: "Saved",
  sInvalid: "Use numbers between 1 and 100, separated by commas.",
  sPrivacy:
    "Reads only local files written by Claude Code and Claude Desktop. No network access.",
};

type Strings = typeof en;

const fr: Strings = {
  title: "Utilisation de Claude",
  session: "Session",
  sessionReset: "Session réinitialisée",
  sessionLong: "Session (5 heures)",
  weekly: "Hebdo",
  weeklyLong: "Hebdomadaire (7 jours)",
  week: "Semaine",
  extraUsage: "Usage supplémentaire",
  spendLimit: "Plafond de dépense",
  resetsIn: (d, at) => `Réinitialisation dans ${d} · ${at}`,
  resetsOn: (at, d) => `Réinitialisation ${at} · dans ${d}`,
  resetUnknown: "Heure de réinitialisation inconnue",
  resetWaiting: "Réinitialisée — en attente de nouvelles données",
  noUpdateSince: (at) => `Pas de mise à jour depuis ${at}`,
  desktopOnly: "Compte à rebours et rythme : données Claude Code requises",
  justStarted: "Fenêtre tout juste ouverte — trop tôt pour juger du rythme",
  lightUse: (elapsed) => `Faible utilisation · ${elapsed} de la fenêtre écoulés`,
  onTrack: (r) => `Dans les temps (rythme ${r})`,
  ahead: (r) => `En avance sur le rythme régulier (${r}) · devrait tenir jusqu’à la réinitialisation`,
  willExhaust: (at) => `À ce rythme, 100 % vers ${at}`,
  exhausted: "Limite atteinte",
  evenPace: "Repère de consommation régulière",
  asOf: (at) => `Données de ${at} (mises à jour quand Claude Code tourne)`,
  source: (s) => `Source : ${s}`,
  sources: {
    status_line: "ligne d’état de Claude Code",
    claude_code_cache: "cache de Claude Code",
    desktop_history: "Claude Desktop",
  },
  staleBanner: (ago) =>
    `Pas de mise à jour depuis ${ago}. Les valeurs se rafraîchissent quand Claude Code ou Claude Desktop tourne.`,
  noDataTitle: "Pas encore de données",
  noDataBody:
    "Installez le pont de ligne d’état (voir le README), puis envoyez un message dans Claude Code.",
  pin: "Épingler le widget",
  unpin: "Détacher le widget",
  openCard: "Ouvrir la carte d’utilisation",
  settings: "Paramètres",
  close: "Fermer",
  sTitle: "Paramètres",
  sLanguage: "Langue",
  sLanguageHint: "Automatique suit la langue d’affichage de Windows.",
  sAuto: "Automatique",
  sNotifications: "Notifications",
  sNotifyOn: "Afficher les alertes d’utilisation",
  sSessionAt: "Alertes de session à (%)",
  sWeeklyAt: "Alertes hebdomadaires à (%)",
  sThresholdHint: "Séparées par des virgules, ex. 80, 95. Chaque alerte ne sonne qu’une fois par fenêtre.",
  sNotifyReset: "Me prévenir quand une limite signalée se réinitialise",
  sStartup: "Lancer au démarrage de Windows",
  sStartupHint: "Démarre discrètement dans la zone de notification à l’ouverture de session.",
  sSaved: "Enregistré",
  sInvalid: "Utilisez des nombres entre 1 et 100, séparés par des virgules.",
  sPrivacy:
    "Lit uniquement des fichiers locaux écrits par Claude Code et Claude Desktop. Aucun accès réseau.",
};

export function t(lang: Lang): Strings {
  return lang === "fr" ? fr : en;
}

export function windowLabel(id: string, lang: Lang, long = false): string {
  const s = t(lang);
  switch (id) {
    case "five_hour":
      return long ? s.sessionLong : s.session;
    case "seven_day":
      return long ? s.weeklyLong : s.weekly;
    case "extra_usage":
      return s.extraUsage;
    case "spend_limit":
      return s.spendLimit;
  }
  const cap = (x: string) => x.charAt(0).toUpperCase() + x.slice(1);
  if (id.startsWith("seven_day_")) return `${s.weekly} · ${cap(id.slice(10))}`;
  if (id.startsWith("five_hour_")) return `${s.session} · ${cap(id.slice(10))}`;
  return cap(id.replace(/_/g, " "));
}

export function pct(p: number, lang: Lang): string {
  const n = Math.round(Math.max(0, p));
  return lang === "fr" ? `${n} %` : `${n}%`;
}

/** "3 h 49 min", "2 h", "49 min", "< 1 min", "4 d 17 h", "3 d". */
export function duration(secs: number, lang: Lang): string {
  const m = Math.floor(Math.max(0, secs) / 60);
  if (m < 1) return "< 1 min";
  if (m < 60) return `${m} min`;
  if (m < 24 * 60) {
    const h = Math.floor(m / 60);
    return m % 60 ? `${h} h ${m % 60} min` : `${h} h`;
  }
  const d = lang === "fr" ? "j" : "d";
  const hours = Math.floor(m / 60) % 24;
  return hours ? `${Math.floor(m / 1440)} ${d} ${hours} h` : `${Math.floor(m / 1440)} ${d}`;
}

/** Local "14:32". */
export function clock(ts: number, lang: Lang): string {
  return new Intl.DateTimeFormat(lang === "fr" ? "fr-FR" : "en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(new Date(ts * 1000));
}

/** Local "Wed 06:00", or just "06:00" when it is today. */
export function dayClock(ts: number, now: number, lang: Lang): string {
  const a = new Date(ts * 1000);
  const b = new Date(now * 1000);
  const sameDay =
    a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
  if (sameDay) return clock(ts, lang);
  const day = new Intl.DateTimeFormat(lang === "fr" ? "fr-FR" : "en-GB", { weekday: "short" }).format(a);
  return `${day} ${clock(ts, lang)}`;
}

export function resetLine(w: WindowView, now: number, lang: Lang): string {
  const s = t(lang);
  if (w.status === "window_reset") return s.resetWaiting;
  if (w.resets_at == null) return s.resetUnknown;
  const left = w.resets_at - now;
  const at = dayClock(w.resets_at, now, lang);
  return left < 24 * 3600 ? s.resetsIn(duration(left, lang), at) : s.resetsOn(at, duration(left, lang));
}

function ratio(r: number, lang: Lang): string {
  const v = r.toFixed(1);
  return `${lang === "fr" ? v.replace(".", ",") : v}×`;
}

/** The one-line pace message under each window. */
export function paceLine(w: WindowView, now: number, lang: Lang): string {
  const s = t(lang);
  if (w.status === "window_reset") return "";
  if (w.status === "stale") return s.noUpdateSince(clock(w.as_of, lang));
  const p = w.pace;
  if (!p) return w.source === "desktop_history" ? s.desktopOnly : "";
  if (w.used_percentage >= 100) return s.exhausted;
  if (p.guarded) {
    if (p.elapsed_fraction < 0.1) return s.justStarted;
    return s.lightUse(pct(p.elapsed_fraction * 100, lang));
  }
  if (p.status === "will_exhaust" && p.projected_exhaustion_at != null)
    return s.willExhaust(dayClock(p.projected_exhaustion_at, now, lang));
  if (p.status === "ahead" && p.pace_ratio != null) return s.ahead(ratio(p.pace_ratio, lang));
  return s.onTrack(ratio(p.pace_ratio ?? 0, lang));
}
