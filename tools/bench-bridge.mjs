// Measures cuw-bridge wall-clock runtime (process spawn included, as Claude Code experiences it).
// Uses a throw-away data directory; never touches the real one.
//
// Usage: node tools/bench-bridge.mjs [path-to-cuw-bridge.exe] [runs]

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const exe = process.argv[2] ?? path.join("target", "release", "cuw-bridge.exe");
const runs = Number(process.argv[3] ?? 60);
const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), "cuw-bench-"));

const now = Math.floor(Date.now() / 1000);
const payload = JSON.stringify({
  model: { id: "claude-opus", display_name: "Opus" },
  workspace: { current_dir: "C:/x" },
  rate_limits: {
    five_hour: { used_percentage: 23.5, resets_at: now + 3600 },
    seven_day: { used_percentage: 41.2, resets_at: now + 86400 },
  },
});

function bench(label, input, setup) {
  setup?.();
  const times = [];
  for (let i = 0; i < runs; i++) {
    const t0 = process.hrtime.bigint();
    const r = spawnSync(exe, [], { input, env: { ...process.env, CUW_DATA_DIR: dataDir } });
    const ms = Number(process.hrtime.bigint() - t0) / 1e6;
    if (r.status !== 0) throw new Error(`${label}: exit ${r.status}`);
    times.push(ms);
  }
  times.sort((a, b) => a - b);
  const q = (p) => times[Math.min(times.length - 1, Math.floor(p * times.length))].toFixed(1);
  console.log(`${label.padEnd(34)} median ${q(0.5)} ms   p90 ${q(0.9)} ms   max ${q(1)} ms   (n=${runs})`);
}

const baseline = process.platform === "win32" ? "C:\\Windows\\System32\\whoami.exe" : "/bin/true";
{
  const times = [];
  for (let i = 0; i < runs; i++) {
    const t0 = process.hrtime.bigint();
    spawnSync(baseline, [], { stdio: "ignore" });
    times.push(Number(process.hrtime.bigint() - t0) / 1e6);
  }
  times.sort((a, b) => a - b);
  console.log(`${"baseline: spawn whoami.exe".padEnd(34)} median ${times[Math.floor(runs / 2)].toFixed(1)} ms`);
}

bench("payload with rate_limits", payload);
bench("payload without rate_limits", JSON.stringify({ model: { display_name: "Opus" } }));
bench("empty stdin", "");
bench("chained `echo hi` (via shell)", payload, () =>
  fs.writeFileSync(path.join(dataDir, "install.json"), JSON.stringify({ previous_status_line: { type: "command", command: "echo hi" } })),
);

fs.rmSync(dataDir, { recursive: true, force: true });
