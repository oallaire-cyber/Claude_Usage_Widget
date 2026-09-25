// Screenshots of the browser preview for every state, light and dark → docs/screenshots/.
// Usage (from app/): npm run build && node scripts/screenshots.mjs [--lang fr]
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { preview } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "..", "..", "docs", "screenshots");
mkdirSync(outDir, { recursive: true });

const langIdx = process.argv.indexOf("--lang");
const lang = langIdx > 0 ? process.argv[langIdx + 1] : "en";
const suffix = lang === "en" ? "" : `-${lang}`;

const STATES = ["normal", "warn", "crit", "stale", "reset", "nodata", "extra", "desktop"];
const THEMES = ["light", "dark"];

const server = await preview({ root: join(here, ".."), preview: { port: 4173, strictPort: false } });
const base = server.resolvedUrls.local[0];
// Microsoft Edge: the same engine as WebView2, which renders the real app.
const browser = await chromium.launch({ channel: "msedge" });
const ctx = await browser.newContext({
  deviceScaleFactor: 2,
  timezoneId: "Europe/Paris",
  locale: lang === "fr" ? "fr-FR" : "en-GB",
  viewport: { width: 520, height: 900 },
});
const page = await ctx.newPage();

async function shot(query, file, selector, pad = 0) {
  await page.goto(`${base}?${query}`);
  await page.waitForSelector(selector);
  await page.evaluate(() => document.fonts.ready);
  const box = await (await page.$(selector)).boundingBox();
  await page.screenshot({
    path: join(outDir, file),
    clip: { x: box.x - pad, y: box.y - pad, width: box.width + 2 * pad, height: box.height + 2 * pad },
  });
  console.log(file);
}

try {
  for (const theme of THEMES) {
    for (const state of STATES) {
      await shot(`view=popup&state=${state}&theme=${theme}&lang=${lang}`, `popup-${state}-${theme}${suffix}.png`, "#app", 16);
    }
    await shot(`view=pinned&state=normal&theme=${theme}&lang=${lang}`, `pinned-normal-${theme}${suffix}.png`, ".pinned", 16);
    await shot(`view=pinned&state=crit&theme=${theme}&lang=${lang}`, `pinned-crit-${theme}${suffix}.png`, ".pinned", 16);
    await shot(`view=settings&theme=${theme}&lang=${lang}`, `settings-${theme}${suffix}.png`, "#app");
  }
} finally {
  await browser.close();
  server.httpServer.close();
}
