// Shared offscreen design capture helpers. Requires Node 22+ and Chrome.

import { spawn } from "node:child_process";
import { createServer } from "node:http";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { fileURLToPath } from "node:url";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export const VOLUME_URL_PATH = "/crates/magistr-gui2/design/almagest-seventh-edition.html";

// ── The faces ─────────────────────────────────────────────────────
//
// The weight breaks are the crate's, not CSS's defaults: the
// proportional family takes its second cut at 600 (`TypeScale::SEMIBOLD`
// — the volume's heaviest voice, so the ledger name's 550 stays
// regular) and the mono family only at CSS bold, since a mono voice
// asks for 400 and only a `BOLD` attribute reaches the bold cut.
// `font-synthesis: none` is the same refusal the painter makes: neither
// family ships a bold italic and faking one is the trick these faces
// exist to stop.
export const FACES = [
  ["Almagest Mono", "/crates/magistr-cockpit/fonts/AlmagestMono-Regular.ttf", "normal", "1 699"],
  ["Almagest Mono", "/crates/magistr-cockpit/fonts/AlmagestMono-Bold.ttf", "normal", "700 1000"],
  ["Almagest Mono", "/crates/magistr-cockpit/fonts/AlmagestMono-Italic.ttf", "italic", "1 1000"],
  ["Almagest UI", "/crates/magistr-gui2/fonts/HankenGrotesk-Regular.ttf", "normal", "1 599"],
  ["Almagest UI", "/crates/magistr-gui2/fonts/HankenGrotesk-SemiBold.ttf", "normal", "600 1000"],
  ["Almagest UI", "/crates/magistr-gui2/fonts/HankenGrotesk-Italic.ttf", "italic", "1 1000"],
];

export function fontCss() {
  const faces = FACES.map(
    ([family, url, style, weight]) => `@font-face{font-family:"${family}";` +
      `src:url("${url}") format("truetype");font-style:${style};` +
      `font-weight:${weight};font-display:block;}`,
  ).join("\n");
  // Only the family resolution is overridden. Every size, weight,
  // tracking and spread in the volume is left at the reference values —
  // this is a re-capture of the same design, not an edit of it.
  return `${faces}
:root{--mono:"Almagest Mono",monospace!important;--sans:"Almagest UI",sans-serif!important;}
.app7,.app7 *{font-synthesis:none;}`;
}

// A capture has to be the same picture every time it is taken. The
// volume's mockups move — sparks sweep, stars twinkle, halos breathe —
// so a run that let them run would measure the moment the shutter fell.
// Freezing to each element's base style (rather than pausing at a
// phase) is the one deterministic choice available in CSS.
export const FREEZE_CSS = `.app7,.app7 *,.app7 *::before,.app7 *::after{animation:none!important;transition:none!important;}`;

// ── A localhost origin for the volume and the faces ───────────────
//
// The page and the fonts are served over http rather than opened as
// file:// so the @font-face requests are same-origin: a file:// page
// fetching a file:// font is an opaque-origin request Chrome refuses
// without a flag, and a capture that silently fell back to the system
// stack would look exactly like a capture that worked.
const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".ttf": "font/ttf",
  ".css": "text/css",
};

export function serve() {
  return new Promise((resolve) => {
    const server = createServer((req, res) => {
      const rel = decodeURIComponent(new URL(req.url, "http://x").pathname);
      const file = path.join(ROOT, rel);
      if (!file.startsWith(ROOT) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
        res.writeHead(404).end("not found");
        return;
      }
      res.writeHead(200, { "content-type": TYPES[path.extname(file)] ?? "application/octet-stream" });
      fs.createReadStream(file).pipe(res);
    });
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}

// ── The browser ───────────────────────────────────────────────────

export function findChrome() {
  if (process.env.CHROME) return process.env.CHROME;
  const cache = path.join(os.homedir(), "Library", "Caches", "ms-playwright");
  if (fs.existsSync(cache)) {
    const builds = fs
      .readdirSync(cache)
      .filter((d) => d.startsWith("chromium-"))
      .sort((a, b) => Number(b.slice(9)) - Number(a.slice(9)));
    for (const build of builds) {
      for (const exe of [
        "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        "chrome-mac/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        "chrome-linux/chrome",
      ]) {
        const candidate = path.join(cache, build, exe);
        if (fs.existsSync(candidate)) return candidate;
      }
    }
  }
  for (const candidate of [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
  ]) {
    if (fs.existsSync(candidate)) return candidate;
  }
  throw new Error("no Chrome found — set CHROME to a browser binary");
}

export async function launch(extraArgs = []) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), "almagest-capture-"));
  const chrome = spawn(
    findChrome(),
    [
      ...extraArgs,
      "--headless=new",
      "--remote-debugging-port=0",
      `--user-data-dir=${profile}`,
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-extensions",
      "--disable-background-networking",
      "--hide-scrollbars",
      "--force-color-profile=srgb",
      // Grayscale antialiasing, no hinting: the same coverage the
      // native painter's own rasterizer produces, so the diff is not
      // measuring two subpixel strategies against each other.
      "--disable-lcd-text",
      "--font-render-hinting=none",
      "about:blank",
    ],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  const portFile = path.join(profile, "DevToolsActivePort");
  const started = Date.now();
  while (Date.now() - started < 30_000) {
    if (fs.existsSync(portFile)) {
      const text = fs.readFileSync(portFile, "utf8").split("\n");
      if (text.length >= 2) {
        return { chrome, profile, port: Number(text[0]) };
      }
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  chrome.kill();
  throw new Error("Chrome did not report a DevTools port");
}

/** One CDP connection: send a command, await its reply. */
export class Cdp {
  constructor(socket) {
    this.socket = socket;
    this.next = 1;
    this.pending = new Map();
    this.events = new Map();
    socket.addEventListener("message", (event) => {
      const msg = JSON.parse(event.data);
      if (msg.id !== undefined) {
        const entry = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) entry.reject(new Error(`${msg.error.message} (${JSON.stringify(msg.error.data ?? null)})`));
        else entry.resolve(msg.result);
      } else {
        const waiters = this.events.get(msg.method) ?? [];
        this.events.set(msg.method, []);
        for (const waiter of waiters) waiter(msg.params);
      }
    });
  }

  static async open(url) {
    const socket = new WebSocket(url);
    await new Promise((resolve, reject) => {
      socket.addEventListener("open", resolve, { once: true });
      socket.addEventListener("error", () => reject(new Error(`cannot reach ${url}`)), { once: true });
    });
    return new Cdp(socket);
  }

  send(method, params = {}, sessionId) {
    const id = this.next++;
    this.socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  once(method) {
    return new Promise((resolve) => {
      this.events.set(method, [...(this.events.get(method) ?? []), resolve]);
    });
  }
}

/** Evaluate `expression` in the page and return its value. */
export async function evaluate(cdp, session, expression, awaitPromise = false) {
  const { result, exceptionDetails } = await cdp.send(
    "Runtime.evaluate",
    { expression, returnByValue: true, awaitPromise },
    session,
  );
  if (exceptionDetails) {
    throw new Error(exceptionDetails.exception?.description ?? exceptionDetails.text);
  }
  return result.value;
}

/** Inject a `<style>` with `css` under `id`, once. */
export async function injectStyle(cdp, session, id, css) {
  await evaluate(
    cdp,
    session,
    `(() => { if (document.getElementById(${JSON.stringify(id)})) return;` +
      ` const s = document.createElement("style"); s.id = ${JSON.stringify(id)};` +
      ` s.textContent = ${JSON.stringify(css)}; document.head.append(s); })()`,
  );
}

/**
 * Pin the embedded faces through @font-face and verify all six loaded.
 *
 * A face is fetched lazily, when something is set in it — so
 * "did the six arrive" cannot be read off `document.fonts.ready`
 * (the mono italic is used by nothing in the volume and would sit
 * unloaded for ever). Ask for each by name instead: a face whose
 * file 404s or fails to parse answers with no match, which is the
 * failure mode that would otherwise fall back to the system stack
 * and look exactly like a capture that worked.
 */
export async function pinFaces(cdp, session) {
  await injectStyle(cdp, session, "almagest-faces", fontCss());
  const missing = await evaluate(
    cdp,
    session,
    `Promise.all(${JSON.stringify(FACES.map(([family, , style, weight]) =>
        `${style} ${weight.split(" ")[0]} 12px "${family}"`,
      ))}.map(q => document.fonts.load(q).then(m => m.length ? null : q)))` +
      `.then(r => r.filter(Boolean))`,
    true,
  );
  if (missing.length > 0) {
    throw new Error(`face(s) the browser could not load: ${missing.join(", ")}`);
  }
  await evaluate(cdp, session, `document.fonts.ready.then(() => true)`, true);
}
