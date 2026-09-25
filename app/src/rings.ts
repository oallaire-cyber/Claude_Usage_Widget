// Concentric usage rings as inline SVG: session outer, weekly inside it, other windows further in.
// Each ring: a track, an arc filled to the displayed %, and a tick at the even-spend point.

import type { WindowView } from "./types";

const NS = "http://www.w3.org/2000/svg";

function el<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number>,
): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e;
}

export interface RingOptions {
  size: number;
  stroke: number;
  gap: number;
  /** Draw at least this many (empty) rings, so "no data" still shows the shape. */
  minRings?: number;
  markers?: boolean;
}

export function rings(windows: WindowView[], o: RingOptions): SVGSVGElement {
  const svg = el("svg", {
    viewBox: `0 0 ${o.size} ${o.size}`,
    width: o.size,
    height: o.size,
    class: "rings",
    "aria-hidden": "true",
  });
  const c = o.size / 2;
  const count = Math.max(windows.length, o.minRings ?? 0);
  for (let i = 0; i < count; i++) {
    const w = windows[i];
    const r = c - o.stroke / 2 - i * (o.stroke + o.gap) - 1;
    if (r <= o.stroke) break;
    const circ = 2 * Math.PI * r;
    svg.append(
      el("circle", { cx: c, cy: c, r, class: "ring-track", "stroke-width": o.stroke, fill: "none" }),
    );
    if (!w) continue;
    const frac = Math.min(1, Math.max(0, w.display_percentage / 100));
    if (frac > 0) {
      const arc = el("circle", {
        cx: c,
        cy: c,
        r,
        fill: "none",
        "stroke-width": o.stroke,
        "stroke-linecap": frac >= 1 ? "butt" : "round",
        "stroke-dasharray": `${Math.max(0.001, frac * circ)} ${circ}`,
        transform: `rotate(-90 ${c} ${c})`,
        class: `ring-arc tone-${w.tone}`,
      });
      svg.append(arc);
    }
    if (o.markers !== false && w.pace && w.status !== "window_reset") {
      // Even-spend tick: where usage would be if spent evenly across the window.
      const a = w.pace.elapsed_fraction * 2 * Math.PI - Math.PI / 2;
      const r1 = r - o.stroke / 2 - 1.5;
      const r2 = r + o.stroke / 2 + 1.5;
      svg.append(
        el("line", {
          x1: c + r1 * Math.cos(a),
          y1: c + r1 * Math.sin(a),
          x2: c + r2 * Math.cos(a),
          y2: c + r2 * Math.sin(a),
          class: "ring-marker",
        }),
      );
    }
  }
  return svg;
}
