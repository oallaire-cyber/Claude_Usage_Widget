// Source B discovery: prints ONLY the `cachedUsageUtilization` block of %USERPROFILE%\.claude.json,
// sanitised, plus the NAMES (never values) of other top-level keys that look usage-related.
//
// Usage: node tools/extract-claude-code-cache.mjs [path-to-.claude.json]
// Never pipe the whole file anywhere: it can contain MCP server API keys.

import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { sanitize, statInfo } from "./lib/sanitize.mjs";

const file = process.argv[2] ?? path.join(os.homedir(), ".claude.json");
const BLOCK = "cachedUsageUtilization";
const USAGE_KEY = /usage|limit|quota|rate|utili[sz]ation/i;

const report = { file: "~/.claude.json", stat: await statInfo(fs, file) };

if (report.stat.exists) {
  try {
    const doc = JSON.parse(await fs.readFile(file, "utf8"));
    report.hasBlock = Object.prototype.hasOwnProperty.call(doc, BLOCK);
    report[BLOCK] = report.hasBlock ? sanitize(doc[BLOCK]) : null;
    report.otherUsageLikeTopLevelKeyNames = Object.keys(doc).filter((k) => k !== BLOCK && USAGE_KEY.test(k));
  } catch (e) {
    report.error = `parse failed: ${e.name}`;
  }
}

console.log(JSON.stringify(report, null, 2));
