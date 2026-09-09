#!/usr/bin/env node
// Capture component specimens with fixed fonts and paused CSS/SVG animation.
// Writes images and a size manifest for the design tests.

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

const VIEWPORT = { width: 1280, height: 900 };

/** `data-mode` of the volume's own theme toggle, per medium. */
const MEDIA = [
  { medium: "night", mode: "mocha" },
  { medium: "day", mode: "latte" },
];

// ── The specimens ─────────────────────────────────────────────────
//
// Each specimen is one DOM node, addressed as (figure index, CSS
// selector within that figure). The sheet's nodes carry no ids, so the
// selectors lean on the volume's own component classes and `:has()`
// where classes collide. The volume is the reference: a
// selector can only break at a re-bless, which re-cuts every capture
// anyway — and a broken selector throws by name, never captures the
// wrong node silently.
//
// `pad` widens the clip symmetrically. It exists for SVG nodes, whose
// getBoundingClientRect covers geometry but not stroke or the dash of
// a dashed ring; the padded size is the specimen's recorded size.
//
// `media`: "both" captures night and day. The two §II.e panels are
// pinned to their own medium and ignore the volume's theme toggle, so
// each is captured once, in the medium it renders.
//
// Figures 0–10 are §II's component sheet (II.a controls · II.b star ·
// II.c cards · II.d indicators · II.e two pinned panes · II.f
// second-edition · II.g text components · II.h icons · II.i life
// layers · II.j settings); 11 is F1 (the new-project overlay), 16 is
// view 01 (the galaxy, carrying the ⌘K palette), 17 is view 02 (the
// brief, carrying the three severity grounds).
const S = (id, figure, selector, opts = {}) => ({
  id,
  figure,
  selector,
  pad: opts.pad ?? 0,
  media: opts.media ?? "both",
});

const SPECIMENS = [
  // II.a — the §5 control classes.
  S("control-data-chip-live", 0, ".chip7.acc"),
  S("control-data-chip-gates", 0, ".chip7.ok"),
  S("control-data-chip-num", 0, ".chip7:has(.num)"),
  S("control-nav-chip", 0, ".chip7:not(.acc):has(.dot7)"),
  S("control-nav-crumb", 0, ".crumb"),
  S("control-action-primary", 0, ".btn7.pri"),
  S("control-action-secondary", 0, ".btn7:not(.pri):not(.neg)"),
  S("control-action-negative", 0, ".btn7.neg"),
  S("control-key-cap", 0, ".btn7.pri .kbd7"),
  S("control-slot", 0, 'div[style*="dashed"]'),

  // II.b — the star, in every state (SVG geometry: pad covers stroke).
  S("star-pending", 1, "svg > g:nth-of-type(1)", { pad: 3 }),
  S("star-ready", 1, "svg > circle:nth-of-type(2)", { pad: 3 }),
  S("star-executing", 1, "svg > circle:nth-of-type(3)", { pad: 3 }),
  S("star-stuck", 1, "svg > circle:nth-of-type(4)", { pad: 3 }),
  S("star-blocked", 1, "svg > g:nth-of-type(2)", { pad: 3 }),
  S("star-done", 1, "svg > circle:nth-of-type(6)", { pad: 3 }),
  S("edge-dependency-done", 1, "svg > line:nth-of-type(1)", { pad: 3 }),
  S("edge-dependency-ahead", 1, "svg > line:nth-of-type(2)", { pad: 3 }),
  S("star-hue-core", 1, "svg > circle:nth-of-type(7)", { pad: 3 }),
  S("star-hue-api", 1, "svg > circle:nth-of-type(8)", { pad: 3 }),
  S("star-hue-ops", 1, "svg > circle:nth-of-type(9)", { pad: 3 }),

  // II.c — cards & blocks; the peach/green severity treatments.
  S("card-plan", 2, '.float7[style*="width:330px"]'),
  S("card-ground-attention", 2, '.float7[style*="--peach"]'),
  S("card-toast-shipped", 2, '.float7[style*="padding:12px 15px"]:not([style*="--peach"])'),
  S("cap-severity-attention", 2, '.cap7[style*="--peach"]'),
  S("cap-severity-positive", 2, '.cap7[style*="--green"]'),
  S("block-diff", 2, 'div[style*="overflow:hidden"][style*="border-radius:11px"]'),
  S("row-diff-del", 2, ".diffline.d"),
  S("row-diff-add", 2, ".diffline.a"),
  S("row-diff-ctx", 2, ".diffline.c"),
  S("block-receipts-ledger", 2, 'div.mono[style*="line-height:2"]'),

  // II.d — indicators & inputs.
  S("indicator-phases", 3, '.mono[style*="gap:12px"]'),
  S("indicator-spark", 3, 'svg[aria-label="activity"]'),
  S("bar-budget", 3, 'div[style*="height:3px"]'),
  // The counter's rolling digits hang below the line box: `.cnt` is an
  // inline-block whose children are all absolute, so it collapses to
  // height 0 and `top:0` puts "211"/"212" under the baseline — in the
  // reference document too, not just here. The pad keeps them in frame;
  // the quirk itself is the volume's to fix at an eighth edition.
  S("indicator-counter", 3, "span.mono.num", { pad: 10 }),
  S("control-composer", 3, 'div[style*="border-radius:999px"]'),
  S("row-cmdk-result", 3, 'div[style*="--glow-m"]'),

  // II.e — the two pinned panes (each ignores the theme toggle).
  S("pane-sky", 4, ".app7", { media: "night" }),
  S("pane-chart", 5, ".app7", { media: "day" }),

  // II.f — second-edition components.
  S("control-segment", 6, 'div[style*="display:inline-flex"]'),
  S("card-think-pin", 6, ".float7:has(.spc)"),
  S("card-signal", 6, '.float7:has(.btn7.acc):not([style*="--red"])'),
  S("card-ground-danger", 6, '.float7[style*="--red"]'),
  S("cap-severity-danger", 6, '.cap7[style*="--red"]'),
  S("control-search-field", 6, 'div[style*="border-radius:11px"][style*="display:flex"]'),

  // II.g — text always lives in a component.
  S("row-status-executing", 7, ".srow"),
  S("row-status-blocked", 7, '.srow:has([style*="--red"])'),
  S("meter", 7, ".meter"),
  S("row-receipt-commit", 7, '.rr:has(use[href="#ic-commit"])'),
  S("row-receipt-gates", 7, ".rr:has(.gled)"),
  S("led-gate-on", 7, ".gled.on"),
  S("led-gate-off", 7, ".gled:not(.on)"),
  S("badge-mode-off", 7, ".mode7.never"),
  S("badge-mode-auto", 7, ".mode7.auto"),
  S("row-rule-off", 7, ".rr:has(.mode7.never)"),
  S("key-hint", 7, ".khint7"),

  // II.i — life layers.
  S("viz-ring-gauge", 9, 'svg[height="44"]'),
  S("viz-heatmap", 9, ".heat"),
  S("viz-skeleton-row", 9, ".skel"),
  S("viz-ghost-stars", 9, 'svg[height="34"]', { pad: 2 }),

  // II.j — settings controls.
  S("control-theme-card", 10, ".tcard:not(.on)"),
  S("control-theme-card-chosen", 10, ".tcard.on"),
  S("control-switch-on", 10, ".tgl.on"),
  S("control-switch-off", 10, ".tgl:not(.on)"),
  S("settings-nav-active", 10, ".sni.on"),
  S("settings-nav-idle", 10, ".sni:not(.on)"),

  // The elevation levels §6 names that the sheet itself does not carry:
  // overlay (F1's new-project modal) and popover (view 01's ⌘K
  // palette), both on the glass surface `.ov7`. Canvas, row and raised
  // are pane-sky/pane-chart, the status rows and the floats above.
  S("elevation-overlay", 11, ".float7.ov7"),
  S("elevation-popover", 16, ".float7.ov7"),

  // View 02's brief cards — the three severity grounds side by side.
  S("ground-severity-danger", 17, '.float7.rise[style*="--red"]'),
  S("ground-severity-attention", 17, '.float7.rise[style*="--peach"]'),
  S("ground-severity-positive", 17, '.float7.rise[style*="--green"]'),
];

// ── Arguments ─────────────────────────────────────────────────────

function parseArgs(argv) {
  const opts = {
    out: path.join(ROOT, "crates", "magistr-gui2", "tests", "mocks", "specimens"),
    ids: SPECIMENS.map((s) => s.id),
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--out") opts.out = path.resolve(argv[(i += 1)]);
    else if (arg === "--ids") opts.ids = argv[(i += 1)].split(",").map((v) => v.trim());
    else throw new Error(`unknown argument ${arg}`);
  }
  const known = new Set(SPECIMENS.map((s) => s.id));
  for (const id of opts.ids) {
    if (!known.has(id)) throw new Error(`no such specimen: ${id}`);
  }
  return opts;
}

// ── The capture ───────────────────────────────────────────────────

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  fs.mkdirSync(opts.out, { recursive: true });
  const server = await serve();
  const origin = `http://127.0.0.1:${server.address().port}`;
  // The GPU rasterizer's antialiasing jitters by ±1 LSB between runs —
  // invisible to the view gate's percentage threshold, fatal to an
  // oracle that promises byte-identity. SwiftShader's software
  // rasterization is deterministic; the view script keeps the GPU so
  // its output stays byte-identical to the reference captures.
  const { chrome, profile, port } = await launch(["--disable-gpu"]);
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

    await evaluate(cdp, sessionId, `document.documentElement.setAttribute("data-theme","light")`);
    await pinFaces(cdp, sessionId);
    await injectStyle(cdp, sessionId, "almagest-freeze", FREEZE_CSS);
    // CSS freeze does not reach SMIL: pause every SVG timeline at t=0.
    await evaluate(
      cdp,
      sessionId,
      `document.querySelectorAll("svg").forEach(s => { s.pauseAnimations(); s.setCurrentTime(0); })`,
    );
    // Re-run the page's own layout after the face swap, as the view
    // capture does, so every box is measured in the embedded type.
    await evaluate(cdp, sessionId, `fit(); document.fonts.status`);

    const chosen = SPECIMENS.filter((s) => opts.ids.includes(s.id));
    const sizes = new Map();
    for (const { medium, mode } of MEDIA) {
      await evaluate(
        cdp,
        sessionId,
        `document.querySelector('.seg button[data-mode="${mode}"]').click()`,
      );
      for (const spec of chosen) {
        if (spec.media !== "both" && spec.media !== medium) continue;
        const rect = await evaluate(
          cdp,
          sessionId,
          `(() => { const fig = document.querySelectorAll("figure")[${spec.figure}];` +
            ` if (!fig) return null;` +
            ` const el = fig.querySelector(${JSON.stringify(spec.selector)});` +
            ` if (!el) return null;` +
            ` const r = el.getBoundingClientRect();` +
            ` return { x: r.x + scrollX, y: r.y + scrollY, w: r.width, h: r.height }; })()`,
        );
        if (!rect) {
          throw new Error(`specimen ${spec.id}: nothing matches figure ${spec.figure} ${spec.selector}`);
        }
        const clip = {
          x: Math.round(rect.x) - spec.pad,
          y: Math.round(rect.y) - spec.pad,
          width: Math.round(rect.w) + 2 * spec.pad,
          height: Math.round(rect.h) + 2 * spec.pad,
        };
        if (clip.width <= 0 || clip.height <= 0) {
          throw new Error(`specimen ${spec.id}: degenerate clip ${clip.width}x${clip.height}`);
        }
        const prior = sizes.get(spec.id);
        if (prior && (prior.width !== clip.width || prior.height !== clip.height)) {
          throw new Error(
            `specimen ${spec.id}: ${prior.width}x${prior.height} in one medium, ` +
              `${clip.width}x${clip.height} in the other — the layout must not depend on the theme`,
          );
        }
        sizes.set(spec.id, { width: clip.width, height: clip.height });
        const { data } = await cdp.send(
          "Page.captureScreenshot",
          { format: "png", captureBeyondViewport: true, clip: { ...clip, scale: 1 } },
          sessionId,
        );
        const file = path.join(opts.out, `${spec.id}-${medium}.png`);
        fs.writeFileSync(file, Buffer.from(data, "base64"));
        process.stdout.write(`${path.relative(ROOT, file)}\n`);
      }
    }

    // The manifest is only rewritten by a full run: a partial `--ids`
    // run is for eyeballing one specimen, not for re-blessing the set.
    if (opts.ids.length === SPECIMENS.length) {
      const manifest = SPECIMENS.map((s) => ({
        id: s.id,
        figure: s.figure,
        selector: s.selector,
        pad: s.pad,
        media: s.media,
        ...sizes.get(s.id),
      }));
      const file = path.join(opts.out, "manifest.json");
      fs.writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`);
      process.stdout.write(`${path.relative(ROOT, file)}\n`);
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
