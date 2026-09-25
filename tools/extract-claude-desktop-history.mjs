// Source C discovery: prints a sanitised view of Claude Desktop's `plan-usage-history.json` from both
// candidate locations (classic %APPDATA%\Claude and the MSIX package folder), plus which is newer.
// Reads no other file in those folders (they also hold MCP configs with API keys).
//
// Usage: node tools/extract-claude-desktop-history.mjs

import fs from "node:fs/promises";
import path from "node:path";
import { sanitize, statInfo } from "./lib/sanitize.mjs";

const NAME = "plan-usage-history.json";
const candidates = [];

if (process.env.APPDATA) {
  candidates.push({ label: "%APPDATA%\\Claude", file: path.join(process.env.APPDATA, "Claude", NAME) });
}
if (process.env.LOCALAPPDATA) {
  const pkgs = path.join(process.env.LOCALAPPDATA, "Packages");
  let dirs = [];
  try {
    dirs = (await fs.readdir(pkgs)).filter((d) => /^Claude_/i.test(d));
  } catch {
    /* no Packages folder */
  }
  for (const d of dirs) {
    candidates.push({
      label: `%LOCALAPPDATA%\\Packages\\${d}\\LocalCache\\Roaming\\Claude`,
      file: path.join(pkgs, d, "LocalCache", "Roaming", "Claude", NAME),
    });
  }
}

/** Structure summary across all samples: key names, counts and ranges. Prints no identifiers. */
function census(samples) {
  const sampleKeys = {};
  const uKeys = {};
  const orgs = new Set();
  const ts = [];
  for (const s of samples) {
    if (!s || typeof s !== "object") continue;
    for (const k of Object.keys(s)) sampleKeys[k] = (sampleKeys[k] ?? 0) + 1;
    if (typeof s.org === "string") orgs.add(s.org);
    if (typeof s.t === "number") ts.push(s.t);
    if (s.u && typeof s.u === "object") {
      for (const [k, v] of Object.entries(s.u)) {
        const e = (uKeys[k] ??= { count: 0, types: {}, min: null, max: null });
        e.count++;
        e.types[typeof v] = (e.types[typeof v] ?? 0) + 1;
        if (typeof v === "number") {
          e.min = e.min === null ? v : Math.min(e.min, v);
          e.max = e.max === null ? v : Math.max(e.max, v);
        }
      }
    }
  }
  ts.sort((a, b) => a - b);
  const gaps = ts.slice(1).map((t, i) => t - ts[i]).sort((a, b) => a - b);
  return {
    samples: samples.length,
    sampleKeys,
    uKeys,
    distinctOrgs: orgs.size,
    first: ts.length ? new Date(ts[0]).toISOString() : null,
    last: ts.length ? new Date(ts[ts.length - 1]).toISOString() : null,
    medianGapSeconds: gaps.length ? Math.round(gaps[Math.floor(gaps.length / 2)] / 1000) : null,
    minGapSeconds: gaps.length ? Math.round(gaps[0] / 1000) : null,
  };
}

const report = { candidates: [] };
for (const c of candidates) {
  const entry = { location: c.label, stat: await statInfo(fs, c.file) };
  if (entry.stat.exists) {
    try {
      const doc = JSON.parse(await fs.readFile(c.file, "utf8"));
      entry.topLevelType = Array.isArray(doc) ? `array(${doc.length})` : typeof doc;
      entry.content = sanitize(doc);
      if (Array.isArray(doc?.samples)) entry.census = census(doc.samples);
    } catch (e) {
      entry.error = `parse failed: ${e.name}`;
    }
  }
  report.candidates.push(entry);
}

console.log(JSON.stringify(report, null, 2));
