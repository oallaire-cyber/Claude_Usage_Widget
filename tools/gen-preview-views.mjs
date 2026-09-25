// Regenerates app/src/preview/views.json from the core's preview example.
// The JSON is produced and validated in memory first, written to a temporary file, then moved into
// place, so a failed run never leaves a truncated fixture behind.
import { execFileSync } from "node:child_process";
import { renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = execFileSync(
  "cargo",
  ["run", "--quiet", "-p", "cuw-core", "--example", "preview_views"],
  { cwd: root, encoding: "utf8" },
);
const parsed = JSON.parse(out);
for (const k of ["normal", "warn", "crit", "stale", "reset", "nodata"]) {
  if (!parsed[k]) throw new Error(`missing preview state ${k}`);
}
const dest = join(root, "app", "src", "preview", "views.json");
writeFileSync(dest + ".tmp", JSON.stringify(parsed, null, 2) + "\n");
renameSync(dest + ".tmp", dest);
console.log(`wrote ${dest} (${Object.keys(parsed).join(", ")})`);
