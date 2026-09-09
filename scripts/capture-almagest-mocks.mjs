#!/usr/bin/env node
// Capture all reference views in light and dark mode with embedded fonts.
// Options: --views 05,06; --out /tmp/captures; --no-fonts.

import fs from "node:fs";
import path from "node:path";
import {
  ROOT,
  VOLUME_URL_PATH,
  FREEZE_CSS,
  serve,
  launch,
  Cdp,
  evaluate,
  injectStyle,
  pinFaces,
} from "./almagest-capture-lib.mjs";

// ── The geometry ──────────────────────────────────────────────────
//
// The volume's `.app7` is a fixed 1100x640 window; the page's own
// script scales it by min(1, fit.clientWidth / 1100) and sets the
// `.fit` box's height to match. At any viewport at or over the wrap's
// 1180px the scale is 1, so the window is exactly 1100x640 at the
// `.fit` box's own origin and the clip is that box and nothing else.
//
// The captures this replaces were 1132x641 — the `.fit` box's full
// width, which carried a row of page background above the window and
// 32 columns of it to the right. That framing was not free: the still
// is rendered on the capture's own pixel size, so the cockpit was laid
// out 32px wider than the window the design draws and every
// right-anchored thing in it — the whole supervision rail — sat 25px
// right of where the volume puts it, inside a region the mask then
// excused as "browser page margin". Cropping to the window is what
// makes the two sides the same picture; it also retires two mask
// exclusions, since there is no browser chrome left to exclude.
const VIEWPORT = { width: 1280, height: 900 };
const WINDOW = { width: 1100, height: 640 };

// The volume carries 30 `<figure>`s: the first eleven are §II's
// component sheet, then the five-step new-project flow, then the
// fourteen numbered views. Naming them by index is what lets the gate
// walk `Fixture::VIEWS` and this script agree on which picture is which.
const FIRST_FIGURE = 11;
const VIEW_IDS = [
  "F1", "F2", "F3", "F4", "F5",
  "01", "02", "03", "04", "05", "06", "07",
  "08", "09", "10", "11", "12", "13", "14",
];

/** `data-mode` of the volume's own theme toggle, per medium. */
const MEDIA = [
  { prefix: "night", mode: "mocha" },
  { prefix: "day", mode: "latte" },
];

// ── Arguments ─────────────────────────────────────────────────────

function parseArgs(argv) {
  const opts = {
    out: path.join(ROOT, "crates", "magistr-gui2", "tests", "mocks"),
    views: VIEW_IDS,
    fonts: true,
    freeze: true,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--out") opts.out = path.resolve(argv[(i += 1)]);
    else if (arg === "--views") opts.views = argv[(i += 1)].split(",").map((v) => v.trim());
    else if (arg === "--no-fonts") opts.fonts = false;
    else if (arg === "--no-freeze") opts.freeze = false;
    else throw new Error(`unknown argument ${arg}`);
  }
  for (const view of opts.views) {
    if (!VIEW_IDS.includes(view)) throw new Error(`no such view: ${view}`);
  }
  return opts;
}

// ── The capture ───────────────────────────────────────────────────

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  fs.mkdirSync(opts.out, { recursive: true });
  const server = await serve();
  const origin = `http://127.0.0.1:${server.address().port}`;
  const { chrome, profile, port } = await launch();
  let cdp;
  try {
    const version = await fetch(`http://127.0.0.1:${port}/json/version`).then((r) => r.json());
    cdp = await Cdp.open(version.webSocketDebuggerUrl);
    const { targetId } = await cdp.send("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await cdp.send("Target.attachToTarget", { targetId, flatten: true });
    await cdp.send("Page.enable", {}, sessionId);
    await cdp.send("Runtime.enable", {}, sessionId);
    await cdp.send(
      "Emulation.setDeviceMetricsOverride",
      { ...VIEWPORT, deviceScaleFactor: 1, mobile: false },
      sessionId,
    );

    const loaded = cdp.once("Page.loadEventFired");
    await cdp.send("Page.navigate", { url: `${origin}${VOLUME_URL_PATH}` }, sessionId);
    await loaded;

    // The volume's own script picks a mode from the OS; pin the page
    // chrome to light so the margin the clip takes either side of the
    // window is one colour in both media, at the reference values.
    await evaluate(cdp, sessionId, `document.documentElement.setAttribute("data-theme","light")`);
    if (opts.fonts) {
      await pinFaces(cdp, sessionId);
    }
    if (opts.freeze) {
      await injectStyle(cdp, sessionId, "almagest-freeze", FREEZE_CSS);
    }
    // The page lays the windows out on load; re-run its own fit() so a
    // face swap that changed nothing about the 1100x640 box is still
    // measured after the swap rather than before it.
    await evaluate(cdp, sessionId, `fit(); document.fonts.status`);

    for (const { prefix, mode } of MEDIA) {
      await evaluate(
        cdp,
        sessionId,
        `document.querySelector('.seg button[data-mode="${mode}"]').click()`,
      );
      for (const view of opts.views) {
        const index = FIRST_FIGURE + VIEW_IDS.indexOf(view);
        const rect = await evaluate(
          cdp,
          sessionId,
          `(() => { const f = document.querySelectorAll("figure")[${index}].querySelector(".fit");` +
            ` const r = f.getBoundingClientRect();` +
            ` return { x: r.x + scrollX, y: r.y + scrollY, w: r.width, h: r.height }; })()`,
        );
        if (Math.round(rect.h) !== WINDOW.height) {
          throw new Error(`view ${view}: the window box is ${rect.h}px tall, not ${WINDOW.height}`);
        }
        const { data } = await cdp.send(
          "Page.captureScreenshot",
          {
            format: "png",
            captureBeyondViewport: true,
            clip: {
              x: Math.round(rect.x),
              y: Math.round(rect.y),
              width: WINDOW.width,
              height: WINDOW.height,
              scale: 1,
            },
          },
          sessionId,
        );
        const file = path.join(opts.out, `${prefix}-${view}.png`);
        fs.writeFileSync(file, Buffer.from(data, "base64"));
        process.stdout.write(`${path.relative(ROOT, file)}\n`);
      }
    }
  } finally {
    cdp?.socket.close();
    chrome.kill();
    server.close();
    // Chrome may still be flushing its profile as it dies; retry the
    // sweep rather than failing a finished capture on the race.
    fs.rmSync(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}

await main();
