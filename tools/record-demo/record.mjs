#!/usr/bin/env node
// Record a taliesin *live preview* demo to an optimized GIF + MP4.
//
//   node record.mjs [demos/<spec>.mjs]      (default: demos/sample.mjs)
//
// It starts `taliesin preview <doc>`, drives a real Chrome through the demo's
// steps (Playwright `recordVideo` → a smooth webm), then ffmpeg-encodes an MP4
// (H.264) and a palette-optimized GIF. Uses the system Google Chrome via
// playwright-core's `channel: 'chrome'`, so there is no browser download.
//
// Env: TALIESIN=<binary> (default ./target/release|debug/taliesin, else `taliesin`).
// A demo that edits its doc is restored afterward, so recording is non-destructive.

import { chromium } from "playwright-core";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, "../..");
const outDir = resolve(here, "out");
mkdirSync(outDir, { recursive: true });

const spec = (await import(resolve(here, process.argv[2] || "demos/sample.mjs"))).default;
// Optional 3rd arg overrides the theme ("light" | "dark") and suffixes the output
// name, so one spec records both variants: `node record.mjs demos/x.mjs light`.
const themeArg = process.argv[3];
if (themeArg === "light" || themeArg === "dark") spec.theme = themeArg;
const outName = (spec.name || "demo") + (themeArg ? `-${themeArg}` : "");
const viewport = spec.viewport || { width: 1000, height: 720 };
const port = spec.port || 4399;
const url = `http://127.0.0.1:${port}/${spec.path || ""}`;
const docPath = resolve(here, spec.doc);

function taliesinBinary() {
  if (process.env.TALIESIN) return process.env.TALIESIN;
  for (const p of ["target/release/taliesin", "target/debug/taliesin"]) {
    if (existsSync(join(repoRoot, p))) return join(repoRoot, p);
  }
  return "taliesin";
}

async function waitForServer(u, ms = 25000) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    try {
      if ((await fetch(u)).ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error(`preview did not come up at ${u}`);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Smooth eased scroll to a fraction of the page over `ms` (nice for a GIF).
async function smoothScroll(page, frac, ms) {
  await page.evaluate(
    ([frac, ms]) =>
      new Promise((res) => {
        const max = document.documentElement.scrollHeight - innerHeight;
        const target = max * frac;
        const start = scrollY;
        const t0 = performance.now();
        const ease = (k) => (k < 0.5 ? 2 * k * k : 1 - (-2 * k + 2) ** 2 / 2);
        (function step(t) {
          const k = Math.min(1, (t - t0) / ms);
          scrollTo(0, start + (target - start) * ease(k));
          k < 1 ? requestAnimationFrame(step) : res();
        })(t0);
      }),
    [frac, ms],
  );
}

console.log(`▶ starting preview: ${spec.doc} on :${port}`);
const server = spawn(taliesinBinary(), ["preview", docPath, String(port)], {
  cwd: repoRoot,
  stdio: "ignore",
  env: process.env,
});
const originalDoc = readFileSync(docPath, "utf8");

const cleanup = () => {
  try { writeFileSync(docPath, originalDoc); } catch {}
  try { server.kill("SIGTERM"); } catch {}
};
process.on("exit", cleanup);

try {
  await waitForServer(url);
  console.log("● recording…");
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  const ctx = await browser.newContext({
    viewport,
    deviceScaleFactor: 2,
    colorScheme: spec.theme === "light" ? "light" : "dark",
    recordVideo: { dir: outDir, size: viewport },
  });
  // taliesin pages ignore the OS preference (they default to dark), so drive the
  // theme explicitly via the saved choice the theme script reads on first paint.
  await ctx.addInitScript((t) => {
    try { localStorage.setItem("tali-theme", t); } catch (e) {}
  }, spec.theme === "light" ? "light" : "dark");
  const page = await ctx.newPage();
  await page.goto(url, { waitUntil: "networkidle" }).catch(() => {});

  const editDoc = async (transform) => writeFileSync(docPath, transform(readFileSync(docPath, "utf8")));
  await spec.steps(page, { page, sleep, smoothScroll: (f, ms) => smoothScroll(page, f, ms), editDoc });

  await ctx.close(); // flushes the .webm
  await browser.close();
} finally {
  cleanup();
}

// --- encode ---------------------------------------------------------------
const webm = readdirSync(outDir).filter((f) => f.endsWith(".webm")).map((f) => join(outDir, f)).sort().pop();
if (!webm) throw new Error("no video was recorded");
const base = join(outDir, outName);
const mp4 = `${base}.mp4`, gif = `${base}.gif`, palette = join(outDir, "_palette.png");
const ff = (args) => {
  const r = spawnSync("ffmpeg", ["-y", "-loglevel", "error", ...args], { stdio: "inherit" });
  if (r.status !== 0) throw new Error("ffmpeg failed: " + args.join(" "));
};
const mb = (p) => (statSync(p).size / 1e6).toFixed(1);

// MP4 (H.264) is the web deliverable — small and crisp; embed it in a
// <video autoplay muted loop playsinline>. The full demo goes here.
console.log("◼ encoding mp4…");
ff(["-i", webm, "-movflags", "+faststart", "-pix_fmt", "yuv420p", "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2", mp4]);

// GIF is opt-in (`gif:` in the spec) and meant for a SHORT clip (GitHub READMEs
// etc.) — a long scroll makes a huge GIF. `gif.clip: [start, end]` (seconds)
// trims it to the moment that matters while the MP4 keeps the whole demo.
const out = [`✓ ${mp4}  (${mb(mp4)} MB)`];
if (spec.gif) {
  const fps = spec.gif.fps ?? 12;
  const gw = spec.gif.width ?? 720;
  const clip = spec.gif.clip ? ["-ss", String(spec.gif.clip[0]), "-to", String(spec.gif.clip[1])] : [];
  console.log("◼ encoding gif…");
  ff([...clip, "-i", webm, "-vf", `fps=${fps},scale=${gw}:-1:flags=lanczos,palettegen=stats_mode=diff`, palette]);
  ff([...clip, "-i", webm, "-i", palette, "-lavfi", `fps=${fps},scale=${gw}:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle`, gif]);
  rmSync(palette);
  const sz = mb(gif);
  out.push(`✓ ${gif}  (${sz} MB)` + (sz > 5 ? "  ⚠ large — prefer the MP4 for web, or set gif.clip to a short range" : ""));
}
rmSync(webm);
console.log("\n" + out.join("\n"));
