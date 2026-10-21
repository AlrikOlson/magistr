import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { readFile, readdir, mkdir, writeFile, stat } from "node:fs/promises";
import { dirname, extname, resolve, sep, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { webkit } from "playwright";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const dist = resolve(root, "dist");
const repo = resolve(root, "../..");
const scopeArg = process.argv.indexOf("--evidence-scope");
const evidenceScope = scopeArg < 0 ? "all" : process.argv[scopeArg + 1];
assert(["all", "search", "recovery", "refinement", "branding", "copy", "reading", "noscript", "recording", "recording-video", "revamp", "revamp-finish", "revamp-hero"].includes(evidenceScope), "Unknown evidence scope");
const evidenceArg = process.argv.indexOf("--evidence-dir");
const evidence = evidenceArg < 0 ? undefined : resolve(repo, process.argv[evidenceArg + 1]);
if (evidence && !evidence.startsWith(resolve(repo, "docs/evidence/desktop-iterations") + sep)) {
  throw new Error("Save captures under docs/evidence/desktop-iterations/<dated-iteration>.");
}
if (evidence) {
  await mkdir(evidence, { recursive: true });
  assert(!(await readdir(evidence)).some((name) => name.endsWith(".png")), "Use a new evidence directory; never overwrite captures.");
}
const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png", ".svg": "image/svg+xml", ".json": "application/json", ".wasm": "application/wasm", ".webm": "video/webm" };
const server = createServer(async (request, response) => {
  try {
    const path = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
    let file = resolve(dist, "." + path);
    assert(file.startsWith(dist + sep) || file === dist);
    if ((await stat(file)).isDirectory()) file = resolve(file, "index.html");
    const bytes=await readFile(file);
    const headers={"Content-Type":mime[extname(file)]??"application/octet-stream","Accept-Ranges":"bytes"};
    const range=/^bytes=(\d*)-(\d*)$/.exec(request.headers.range||"");
    if(range&&(range[1]||range[2])){
      const start=range[1]?Number(range[1]):Math.max(0,bytes.length-Number(range[2]));
      const end=range[1]&&range[2]?Math.min(Number(range[2]),bytes.length-1):bytes.length-1;
      if(start>=bytes.length||end<start){
        response.writeHead(416,{...headers,"Content-Range":"bytes */"+bytes.length});
        response.end();return;
      }
      response.writeHead(206,{...headers,"Content-Range":"bytes "+start+"-"+end+"/"+bytes.length,"Content-Length":end-start+1});
      response.end(bytes.subarray(start,end+1));return;
    }
    response.writeHead(200,{...headers,"Content-Length":bytes.length});
    response.end(bytes);
  } catch {
    response.writeHead(404);
    response.end("Not found");
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const baseArg = process.argv.indexOf("--base-url");
const origin = baseArg < 0 ? "http://127.0.0.1:" + server.address().port : new URL(process.argv[baseArg + 1]).origin;
const browser = await webkit.launch({ headless: true });
const checks = [];
const captures = [];
const paths = [];
const collect = async (directory) => {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = resolve(directory, entry.name);
    if (entry.isDirectory()) await collect(file);
    else paths.push(file);
  }
};
await collect(resolve(root, "src"));
await collect(resolve(root, "public/demo"));
await collect(resolve(root, "public/examples"));
for (const name of [
  "astro.config.mjs", "package.json", "package-lock.json", "tests/browser.mjs",
  "public/og-image.svg", "public/og-image.png", "public/github-social.png", "public/favicon.svg", "public/icon-192.png", "public/site.webmanifest",
  "public/screenshots/captures.json", "scripts/generate-favicons.mjs", "scripts/asset-url.mjs",
]) paths.push(resolve(root, name));
const sourceHashes = Object.fromEntries(await Promise.all(paths.sort().map(async (file) => [
  relative(repo, file), createHash("sha256").update(await readFile(file)).digest("hex"),
])));
const writeReport = async (status, failure) => {
  if (!evidence) return;
  const report = {
    completedAt: new Date().toISOString(), status, failure,
    renderer: "Playwright WebKit", rendererVersion: browser.version(),
    platform: process.platform + "-" + process.arch, node: process.version, origin,
    motion: "reduced", scale: 1, evidenceScope, checks, sourceHashes, captures,
    limitations: [
      "Headless browser only; no native OS-input or assistive-technology testing.",
      "Screenshots require separate recorded visual review.",
      ...(baseArg < 0 ? ["Local build; no deployed-site or registry publication claim."] : ["Deployed website only; no registry publication claim."]),
    ],
  };
  await writeFile(resolve(evidence, "browser-checks.json"), JSON.stringify(report, null, 2) + "\n");
};
const capture = async (page, name, state, fullPage = false, region) => {
  if (evidenceScope === "revamp-hero" && !/^home(?:-intro)?-(wide|compact)-(light|dark)$/.test(name) && !name.startsWith("home-menu")) return;
  if (evidenceScope === "revamp-finish") {
    const changedHome=/^home(?:-(?:intro|keep-going))?-(wide|compact)-(light|dark)$/.test(name);
    const changedTour=/^tour-roadmap-(wide|compact)-(light|dark)$/.test(name);
    const reflow=/^tour-(320|820)-/.test(name);
    if(!changedHome&&!changedTour&&!reflow&&!name.startsWith("home-menu")&&name!=="tour-without-javascript")return;
  }
  if (evidenceScope === "revamp") {
    const pageFrame=/^(home|installation|workflow|concepts|roadmap|reasoning|chunks|verification|not-found)-(wide|compact)-(light|dark)$/.test(name);
    const overview=/^(home-(intro|install|keep-going|agents)|tour-(roadmap|reasoning|chunks|checks))-(wide|compact)-(light|dark)$/.test(name);
    const detail=/-wide-light$|-compact-dark$/.test(name) && /^(tour-|home-(menu|evidence-[0-2])|(?:installation|workflow|concepts|roadmap|reasoning|chunks|verification)-reading)/.test(name);
    const recovery=/^installation-search-(empty|unavailable)-compact-dark$/.test(name);
    const reflow=/^tour-(320|820)-/.test(name);
    if(!pageFrame&&!overview&&!detail&&!recovery&&!reflow&&name!=="tour-without-javascript")return;
  }
  if (evidenceScope === "recording-video" && !name.startsWith("recording-video-")) return;
  if (evidenceScope === "recording" && !name.startsWith("recording-") && !name.startsWith("home-intro-")) return;
  if (!evidence || (evidenceScope === "search" && !name.includes("search"))) return;
  if (evidenceScope === "recovery" && !name.includes("search") && !/^not-found-(wide|compact)-(light|dark)$/.test(name)) return;
  if (evidenceScope === "noscript" && name !== "home-without-javascript") return;
  if (evidenceScope === "reading") {
    const overview = /^(home|installation|workflow|concepts|not-found)-(wide|compact)-(light|dark)$/.test(name);
    const home = /^home-(intro|product)-(wide|compact)-(light|dark)$/.test(name);
    const docs = /^(installation|workflow|concepts)-(?!search)[a-z-]+-wide-(light|dark)$/.test(name);
    if (!overview && !home && !docs) return;
  }
  if (evidenceScope === "copy") {
    // Full pages cover all four profiles; detail frames use a complementary
    // desktop/light and compact/dark pair to keep the review readable.
    const profile = /-wide-light$|-compact-dark$/.test(name);
    const overview = /^(home|installation|workflow|concepts|not-found)-(wide|compact)-(light|dark)$/.test(name);
    const detail = /^(home-(intro|product|footer|install|workflow-[0-3]|evidence-[0-2]|menu|copy-unavailable)|(?:installation|workflow|concepts)-(?!search)[a-z-]+)-(wide|compact)-(light|dark)$/.test(name);
    const states = /-(contents|contents-target)-(wide|compact)-(light|dark)$/.test(name)
      || /^home-(workflow-[0-3]|evidence-[0-2])-(wide|compact)-(light|dark)$/.test(name)
      || (profile && /^(installation-search|home-(transcript|capture|copy-success|menu-dismissed))/.test(name))
      || name === "home-without-javascript";
    if (!overview && !(profile && detail) && !states && !name.startsWith("workflow-320-")) return;
  }
  if (evidenceScope === "branding" && !name.includes("-intro-") && !name.startsWith("home-320-")) return;
  if (evidenceScope === "refinement" && !/^(home-(wide|compact|connection|workflow|capture|transcript)|home-evidence-focus|.*-320)/.test(name)) return;
  // screenshot waits for fonts and a stable render itself. A page-script frame
  // callback never fires when JavaScript is disabled.
  const file = name + ".png";
  if (region) {
    await page.locator(region).evaluate(e=>scrollTo({top:e.getBoundingClientRect().top+scrollY-90,behavior:"instant"}));
    await page.locator(region).screenshot({ path: resolve(evidence, file), animations: "disabled" });
  }
  else await page.screenshot({ path: resolve(evidence, file), fullPage, animations: "disabled" });
  const bytes = await readFile(resolve(evidence, file));
  captures.push({
    file, state, url: new URL(page.url()).pathname, capturedAt: new Date().toISOString(),
    viewport: page.viewportSize(), appearance: await page.evaluate(() => document.documentElement.classList.contains("dark") ? "dark" : "light"),
    fullPage, region, renderer: "Playwright WebKit", rendererVersion: browser.version(),
    width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20),
    sha256: createHash("sha256").update(bytes).digest("hex"), review: "pending",
  });
  await writeReport("running");
};
const noOverflow = async (page) => {
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "No horizontal page overflow");
  assert(await page.locator("main h1").count() === 1, "One primary heading");
  assert(await page.locator("img").evaluateAll((images) => images.filter((image) => image.getClientRects().length && !image.closest("details:not([open])")).every((image) => image.complete && image.naturalWidth > 0)), "All images loaded");
};
const legibility = async (page) => {
  const results = await page.evaluate(() => {
    const luminance = (color) => color.match(/[\d.]+/g).slice(0, 3)
      .map(Number).map((value) => value / 255)
      .map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4)
      .reduce((sum, value, index) => sum + value * [0.2126, 0.7152, 0.0722][index], 0);
    const ratio = (a, b) => (Math.max(luminance(a), luminance(b)) + 0.05) / (Math.min(luminance(a), luminance(b)) + 0.05);
    const primary = [...document.querySelectorAll(".button-primary")].map((element) => {
      const style = getComputedStyle(element);
      return ratio(style.color, style.backgroundColor);
    });
    const theme = getComputedStyle(document.querySelector("#theme-toggle"));
    const nav = getComputedStyle(document.querySelector(".site-nav"));
    const controls = [...document.querySelectorAll(".theme-toggle, .menu-toggle, .github-link")].filter((element) => element.getClientRects().length).map((element) => {
      const rect = element.getBoundingClientRect();
      return { width: rect.width, height: rect.height };
    });
    const code = [...document.querySelectorAll("pre")].map((element) => element.scrollWidth - element.clientWidth);
    return { primary, theme: ratio(theme.color, nav.backgroundColor), code, controls };
  });
  assert(results.controls.every(({ width, height }) => width >= 44 && height >= 44), "Navigation icon targets are at least 44 px");
  assert(results.primary.every((contrast) => contrast >= 4.5), "Primary text contrast: " + JSON.stringify(results));
  assert(results.theme >= 3, "Theme icon contrast: " + JSON.stringify(results));
  assert(results.code.every((overflow) => overflow <= 1), "Code is readable without horizontal clipping");
};
const scrollHeading = async (page, selector) => {
  await page.locator(selector).evaluate((element) => scrollTo({ top: element.getBoundingClientRect().top + scrollY - 90, behavior: "instant" }));
};
try {
  for (const viewport of [{ width: 1440, height: 1000 }, { width: 390, height: 844 }]) {
    for (const colorScheme of ["light", "dark"]) {
      const layout = viewport.width > 800 ? "wide" : "compact";
      const suffix = layout + "-" + colorScheme;
      const context = await browser.newContext({ viewport, deviceScaleFactor: 1, colorScheme, reducedMotion: "reduce" });
      await context.route("https://plausible.io/**", (route) => route.fulfill({ status: 204 }));
      const page = await context.newPage();
      const errors = [];
      const searchRequests = [];
      page.on("request", (request) => { if (new URL(request.url()).pathname.startsWith("/pagefind/")) searchRequests.push(request.url()); });
      page.on("pageerror", (error) => errors.push(error.message));
      for (const [name, path] of [["home", "/"], ...["installation","workflow","roadmap","reasoning","chunks","verification","concepts"].map(slug=>[slug,"/docs/"+slug+"/"]), ["not-found", "/404.html"]]) {
        const response = await page.goto(origin + path);
        assert.equal(response.status(), 200);
        await page.evaluate(() => document.fonts.ready);
        // Scroll lazy images into view before awaiting their actual load.
        for (const image of await page.locator("img").all()) {
          if (!(await image.isVisible())) continue;
          await image.scrollIntoViewIfNeeded();
          await page.waitForFunction((element) => element.complete && element.naturalWidth > 0,
            await image.elementHandle(), { timeout: 10000 });
          await image.evaluate((element) => element.decode());
        }
        await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
        await noOverflow(page);
        await legibility(page);
        await capture(page, name + "-" + suffix, name + " full page", true);
        if (name !== "home") await capture(page, name + "-intro-" + suffix, name + " navigation and opening");
        if (name === "home") {
          for (const link of await page.locator(".image-link").all()) {
            const response = await context.request.get(new URL(await link.getAttribute("href"), origin).href);
            assert.equal(response.status(), 200, "Full session link opens its original image");
            assert.match(response.headers()["content-type"], /image\/png/);
          }
          await capture(page, "home-intro-" + suffix, "Product description, setup action and project guide");
          for (const [area, selector] of [
            ["evidence", "#evidence"], ["workflow", "#workflow"], ["product", "#product"], ["footer", ".site-footer"],
          ]) {
            await scrollHeading(page, selector);
            await capture(page, "home-" + area + "-" + suffix, area + " section");
          }
          const tour=page.locator("[data-project-tour]");
          const tourTabs=tour.getByRole("tab");
          await tourTabs.first().waitFor();
          assert.equal(await tourTabs.count(),4);
          for(const [index,id] of ["roadmap","reasoning","chunks","checks"].entries()){
            await tourTabs.nth(index).click();
            assert.equal(await tour.getByRole("tabpanel").count(),1,"One feature view is exposed");
            await noOverflow(page);
            await capture(page,"tour-"+id+"-"+suffix,"Explore "+id+" with real records",false,".project-tour");
            const region=tour.locator("#tour-"+id);
            const selectors=region.locator("[data-record-select]");
            const defaultIndex=id==="roadmap"?4:id==="reasoning"?2:1;
            for(let record=0;record<await selectors.count();record++){
              await selectors.nth(record).click();
              assert.equal(await selectors.nth(record).getAttribute("aria-pressed"),"true");
              assert.equal(await region.locator("[data-record-detail]:visible").count(),1);
              assert.equal(await region.locator("[data-record-detail]:visible").getAttribute("data-record-detail"),String(record));
            }
            await selectors.nth(defaultIndex).click();
            if(id==="roadmap"){
              await region.locator("[data-record-detail]:visible summary").click();
              assert.match(await region.locator("[data-record-detail]:visible").innerText(),/11 recorded dependencies/);
              if(suffix==="wide-light"||suffix==="compact-dark")await capture(page,"tour-requirements-"+suffix,"Original requirements and dependencies remain inspectable",false,".project-tour");
              await region.locator("[data-record-detail]:visible summary").click();
            }
            if(id==="reasoning"){
              await selectors.first().click();
              if(suffix==="wide-light"||suffix==="compact-dark")await capture(page,"tour-alternative-"+suffix,"Inspect an actual recorded alternative",false,".project-tour");
            }
            if(id==="checks"){
              await selectors.first().click();
              assert.match(await region.locator("[data-record-detail]:visible").innerText(),/No module named 'store'/);
              if(suffix==="wide-light"||suffix==="compact-dark")await capture(page,"tour-failed-check-"+suffix,"The original failing check is retained",false,".project-tour");
              await selectors.last().click();
              assert.match(await region.locator("[data-record-detail]:visible").innerText(),/Ran 7 tests/);
            }
          }
          await tourTabs.first().focus();
          await page.keyboard.press("End");
          assert.equal(await tourTabs.last().getAttribute("aria-selected"),"true");
          await page.keyboard.press("ArrowRight");
          assert.equal(await tourTabs.first().getAttribute("aria-selected"),"true");
          assert(await tourTabs.first().evaluate(e=>e===document.activeElement));
          if(suffix==="wide-light")await capture(page,"tour-keyboard-"+suffix,"Keyboard exploration retains visible focus",false,".project-tour");
          await tourTabs.nth(2).click();
          await tour.locator('#tour-chunks [data-record-select="1"]').click();
          await tour.locator('#tour-chunks [data-record-detail="1"] [data-playback-scene]').click();
          await page.waitForFunction(()=>document.querySelector("[data-recording]").dataset.scene==="1");
          assert.equal(new URL(page.url()).hash,"#workflow","A task opens its real recorded chapter");
          await page.locator('[data-chapter="5"]').click();
          await tourTabs.first().click();
          for(const [area,selector] of [["keep-going","#keep-going"],["agents","#agents"]]){
            await capture(page,"home-"+area+"-"+suffix,area+" feature explanations",false,selector);
          }
          const recording = page.locator("[data-recording]");
          await page.waitForFunction(() => document.querySelector("[data-recording]").dataset.ready === "true");
          assert.equal(await recording.getAttribute("data-scene"), "5");
          assert.equal(await recording.locator("[data-chapter]").count(), 6);
          const controls=await recording.locator(".story-controls button:visible").evaluateAll(buttons=>buttons.map(b=>({text:b.textContent,...b.getBoundingClientRect().toJSON()})));
          assert(controls.every(b=>b.width>=44&&b.height>=44),"Playback controls keep usable targets");
          for(let i=0;i<controls.length;i++)for(let j=i+1;j<controls.length;j++){
            const a=controls[i],b=controls[j];
            assert(Math.min(a.right,b.right)-Math.max(a.left,b.left)<=1||Math.min(a.bottom,b.bottom)-Math.max(a.top,b.top)<=1,"Playback controls do not overlap: "+a.text+" / "+b.text);
          }
          const transcript = await (await context.request.get(new URL(await recording.getAttribute("data-recording-url"), origin).href)).json();
          assert.equal(transcript.events.length,47);
          await scrollHeading(page, ".build-recording");
          await capture(page, "recording-overview-" + suffix, "Finished inbox, guided entry and six build chapters", false, ".build-recording");
          const detailed = suffix === "wide-light" || suffix === "compact-dark";
          await recording.locator("[data-next]").click();
          assert.equal(await recording.getAttribute("data-scene"),"0","The entry starts with the real brief");
          for(let chapter=0;chapter<6;chapter++){
            await recording.locator('[data-chapter="'+chapter+'"]').click();
            assert.equal(await recording.getAttribute("data-scene"),String(chapter));
            assert.equal(await recording.locator(".story-scene:visible").count(),1);
            for(const img of await recording.locator(".story-scene:visible img:visible").all())await img.evaluate(img=>img.decode());
            await noOverflow(page);
            if(detailed&&chapter<5){
              await scrollHeading(page,".build-recording");
              await capture(page,"recording-chapter-"+chapter+"-"+suffix,"Guided chapter "+(chapter+1),false,".build-recording");
            }
          }
          await recording.locator('[data-chapter="4"]').click();
          await recording.locator('[data-review="before"]').click();
          assert.match(await recording.locator("[data-review-note]").innerText(),/Review failed/);
          assert(await recording.locator('[data-scene-panel="4"] [data-capture-version="before"]').isVisible());
          if(detailed)await capture(page,"recording-review-before-"+suffix,"Original failed screen review; recorded capture retained",false,".build-recording");
          await recording.locator('[data-review="after"]').click();
          assert(await recording.locator('[data-scene-panel="4"] [data-capture-version="after"]').isVisible());
          await recording.locator('[data-chapter="1"]').click();
          await recording.locator('[data-inspect="16"]').click();
          assert(await recording.locator(".recording-details").evaluate(e=>e.open));
          assert.equal(await recording.getAttribute("data-index"),"16");
          assert.match(await recording.locator("[data-output]").innerText(),/Ran 7 tests/);
          for(const action of [14,15,35,44]){
            await recording.getByLabel("Recorded action",{exact:true}).selectOption(String(action));
            assert.equal(await recording.locator("[data-action-title]").innerText(),transcript.events[action].title);
            if(action===14){
              assert.equal(await recording.locator("[data-action-status]").innerText(),"Failed");
              assert.match(await recording.locator("[data-output]").innerText(),/No module named 'store'/);
            }
            if(action===15)assert.match(await recording.locator("[data-output]").innerText(),/BEGIN IMMEDIATE/);
            if(action===35)assert.match(await recording.locator("[data-output]").innerText(),/Before[\s\S]*After/);
            if(action===44)assert.match(await recording.locator("[data-output]").innerText(),/Ran 10 tests/);
            if((suffix==="wide-light"&&action===14)||(suffix==="compact-dark"&&action===35)){
              await scrollHeading(page,".recording-inspector");
              await capture(page,"recording-action-"+action+"-"+suffix,"Full original recorded source and result",false,".recording-details");
            }
          }
          await recording.getByLabel("Recorded source or output").selectOption("examples/support-inbox/store.py");
          assert.match(await recording.locator("[data-output]").innerText(),/class Store/);
          if(suffix==="wide-light"){
            const titles=await recording.evaluate(root=>{
              const select=root.querySelector("[data-action-select]"),titles=[];
              for(let index=0;index<47;index++){select.value=String(index);select.dispatchEvent(new Event("change",{bubbles:true}));titles.push(root.querySelector("[data-action-title]").textContent);}
              return titles;
            });
            assert.deepEqual(titles,transcript.events.map(e=>e.title));
          }
          await recording.locator(".recording-details > summary").click();
          await recording.locator('[data-chapter="2"]').focus();
          await page.keyboard.press("Enter");
          assert.equal(await recording.getAttribute("data-scene"),"2");
          assert(await recording.locator('[data-chapter="2"]').evaluate(e=>e===document.activeElement));
          if(suffix==="wide-light"){
            await capture(page,"recording-keyboard-"+suffix,"Keyboard chapter selection retains visible focus",false,".build-recording");
            await recording.locator('[data-chapter="0"]').click();
            await recording.locator("[data-play]").click();
            await page.waitForFunction(()=>document.querySelector("[data-recording]").dataset.scene==="1",{},{timeout:10000});
            await recording.locator("[data-play]").click();
            assert.equal(await recording.getAttribute("data-playing"),"false");
            await page.waitForTimeout(150);
            assert.equal(await recording.getAttribute("data-scene"),"1","Pause stops chapter progression");
            await recording.locator("[data-play]").click();
            await recording.locator('[data-chapter="4"]').click();
            assert.equal(await recording.getAttribute("data-playing"),"false","Manual navigation stops automatic playback");
          }
          await recording.locator('[data-chapter="5"]').click();
          await recording.locator("[data-watch]").click();
          await page.waitForFunction(()=>document.querySelector(".build-recording video").readyState>=2);
          assert((await recording.locator("video").evaluate(video=>video.duration))>11);
          await recording.locator("video").evaluate(video=>{video.pause();video.currentTime=4;});
          await page.waitForFunction(()=>{
            const v=document.querySelector(".build-recording video");
            return v.readyState>=2&&!v.seeking&&Math.abs(v.currentTime-4)<.1;
          });
          const bright=await recording.locator("video").evaluate(v=>{
            const c=document.createElement("canvas");c.width=128;c.height=90;
            c.getContext("2d").drawImage(v,0,0,128,90);
            return [...c.getContext("2d").getImageData(0,0,128,90).data].filter((value,i)=>i%4!==3&&value>100).length;
          });
          assert(bright>5000,"The seeked video frame contains the actual light-theme inbox, not a black frame");
          await capture(page,"recording-video-"+suffix,"Original completed-app video with native playback controls",false,".build-recording");
          await recording.locator("[data-watch]").click();
          assert(!(await recording.locator("video").isVisible()));
          await page.locator("#evidence > summary").click();
          assert(await page.locator("#evidence").evaluate(e=>e.open));
          for (const [groupName, selector, labels] of [
            ["evidence", ".product-evidence", ["Codex", "Claude Code", "Magistr desktop"]],
          ]) {
            const group = page.locator(selector);
            const tabs = group.getByRole("tab");
            assert.equal(await tabs.count(), labels.length);
            for (let index = 0; index < labels.length; index++) {
              await tabs.nth(index).click();
              assert.equal(await tabs.nth(index).getAttribute("aria-selected"), "true");
              assert.equal(await group.getByRole("tabpanel").count(), 1, "Only the selected panel is exposed");
              const panel = group.getByRole("tabpanel");
              for (const img of await panel.locator("img").all()) {
                if (!(await img.isVisible())) continue;
                await img.scrollIntoViewIfNeeded();
                await img.evaluate((element) => element.decode());
              }
              await noOverflow(page);
              await scrollHeading(page, selector);
              await capture(page, "home-" + groupName + "-" + index + "-" + suffix, labels[index] + " selected");
              if (groupName === "evidence") {
                assert(await panel.locator(".capture-excerpt").isVisible(), "Recorded excerpts are readable at every viewport");
                if (layout === "compact") {
                  await scrollHeading(page, "#" + await panel.getAttribute("id") + " .capture-excerpt");
                  await capture(page, "home-transcript-" + index + "-" + suffix, labels[index] + " transcript for compact reading");
                }
                await panel.locator(".capture").scrollIntoViewIfNeeded();
                await capture(page, "home-capture-" + index + "-" + suffix, labels[index] + " actual capture");
              }
            }
            await tabs.first().focus();
            await page.keyboard.press("End");
            assert.equal(await tabs.last().getAttribute("aria-selected"), "true");
            assert(await tabs.last().evaluate((element) => element === document.activeElement));
            await page.keyboard.press("ArrowRight");
            assert.equal(await tabs.first().getAttribute("aria-selected"), "true", "Arrow navigation wraps");
            await page.keyboard.press("ArrowLeft");
            assert.equal(await tabs.last().getAttribute("aria-selected"), "true");
            await page.keyboard.press("Home");
            assert.equal(await tabs.first().getAttribute("aria-selected"), "true");
            await capture(page, "home-" + groupName + "-focus-" + suffix, "Keyboard selection and visible tab focus");
          }
          await page.locator("#install-btn").click();
          await scrollHeading(page, "#install .section-heading");
          assert.match(await page.locator("#install").innerText(), /Setup does not start implementation/);
          await capture(page, "home-install-" + suffix, "Preview installation and planning handoff");
          await page.evaluate(() => {
            Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value) => { window.__copiedCommands = value; } } });
          });
          await page.getByRole("button", { name: "Copy commands" }).click();
          await page.getByRole("status").filter({ hasText: "Commands copied." }).waitFor();
          assert((await page.evaluate(() => window.__copiedCommands)).includes("cargo install --locked --path crates/magistr-cli"));
          await capture(page, "home-copy-success-" + suffix, "Copy confirms the exact source commands");
          await page.evaluate(() => { navigator.clipboard.writeText = async () => { throw new Error("unavailable"); }; });
          await page.getByRole("button", { name: "Copy commands" }).click();
          await page.getByText("Copy is unavailable. Select the commands above to copy them.", { exact: true }).waitFor();
          await capture(page, "home-copy-unavailable-" + suffix, "Denied clipboard offers manual selection");
          // Theme switches persist across a new document.
          await page.getByRole("button", { name: "Toggle dark mode" }).click();
          assert.equal(await page.evaluate(() => document.documentElement.classList.contains("dark")), colorScheme !== "dark");
          await page.reload();
          assert.equal(await page.evaluate(() => document.documentElement.classList.contains("dark")), colorScheme !== "dark");
          await page.getByRole("button", { name: "Toggle dark mode" }).click();
          await legibility(page);
          if (layout === "compact") {
            const toggle = page.locator("[data-menu-toggle]");
            await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
            await toggle.click();
            assert.equal(await toggle.getAttribute("aria-expanded"), "true");
            await capture(page, "home-menu-" + suffix, "Expanded compact navigation");
            const firstLink = page.locator("#mobile-menu").getByRole("link", { name: "Explore Magistr", exact: true });
            await firstLink.focus();
            await page.keyboard.press("Escape");
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            assert(await toggle.evaluate((element) => element === document.activeElement));
            await capture(page, "home-menu-dismissed-" + suffix, "Escape closes menu and restores visible focus");
            await toggle.click();
            await firstLink.click();
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            assert.equal(new URL(page.url()).hash, "#product");
            assert(await page.locator("#product").evaluate((element) => element === document.activeElement));
            await toggle.click();
            await firstLink.focus();
            await page.setViewportSize({ width: 1024, height: 1000 });
            await page.waitForFunction(() => document.querySelector("[data-menu-toggle]").getAttribute("aria-expanded") === "false");
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            assert(await page.locator("[data-nav-links] a").first().evaluate((element) => element === document.activeElement));
            await page.setViewportSize(viewport);
            assert.equal(await toggle.getAttribute("aria-expanded"), "false");
            await toggle.click();
            await page.locator("#mobile-menu").getByRole("link", { name: "Docs", exact: true }).click();
            assert.equal(new URL(page.url()).pathname.replace(/\/$/, ""), "/docs/installation");
          }
        }
        if (["installation", "concepts", "workflow", "roadmap", "reasoning", "chunks", "verification"].includes(name)) {
          const guideList=page.locator(".docs-guide-list");
          if(layout==="compact"){
            assert(!(await guideList.evaluate(e=>e.open)));
            await guideList.locator("summary").click();
            assert.equal(await guideList.locator("a:visible").count(),7);
            if(suffix==="compact-dark")await capture(page,name+"-reading-navigation-"+suffix,"Open compact guide navigation");
            await guideList.locator("summary").click();
          }
          const contents = page.locator(".docs-contents");
          const summary = contents.locator("summary");
          await summary.focus();
          await page.keyboard.press("Enter");
          assert.equal(await contents.getAttribute("open"), "", "Contents opens with the keyboard");
          const sectionLinks = contents.getByRole("link");
          assert.equal(await sectionLinks.count(), await page.locator("article h2").count());
          await scrollHeading(page, ".docs-header");
          await capture(page, name + "-contents-" + suffix, "Expanded section navigation");
          const sectionLink = sectionLinks.first();
          const target = await sectionLink.getAttribute("href");
          await sectionLink.click();
          assert.equal(new URL(page.url()).hash, target);
          const targetY = await page.locator(target).evaluate((element) => element.getBoundingClientRect().top);
          const headerBottom = await page.locator(".site-nav").evaluate((element) => element.getBoundingClientRect().bottom);
          assert(targetY >= headerBottom, "Section link keeps its heading below the sticky header");
          await capture(page, name + "-contents-target-" + suffix, "Section link reaches a readable heading");
          await summary.click();
          assert.equal(await contents.getAttribute("open"), null);
          let headingIndex=0;
          for (const heading of await page.locator("article h2").all()) {
            const id = await heading.getAttribute("id");
            await scrollHeading(page, "#" + id);
            await capture(page, name + "-" + id + "-" + suffix, await heading.innerText());
            if((suffix==="wide-light"||suffix==="compact-dark") && headingIndex++===2)await capture(page,name+"-reading-detail-"+suffix,"Readable body text and concrete instructions");
          }
        }
        if (name === "concepts") {
          await capture(page, "concepts-search-idle-" + suffix, "Search is ready without starting its index");
        }
        if (name === "installation") {
          const body = await page.locator("article").innerText();
          assert.match(body, /Setup adds files only for the agents you choose/);
          assert.match(body, /magistr init \. --agent codex,claude --dry-run/);
          assert.match(body, /magistr init \. --agent portable --yes/);
          assert.match(body, /magistr remove \. --dry-run/);
          await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
          const search = page.getByRole("searchbox", { name: "Search documentation" });
          assert.deepEqual(searchRequests, [], "Reading docs does not start search downloads");
          await search.fill("repair");
          await page.locator(".docs-search-results a").first().waitFor();
          assert.match(await page.locator(".docs-search-results").innerText(), /Installation/);
          await capture(page, "installation-search-" + suffix, "Documentation search for repair");
          await search.fill("qzxvbnabcdef");
          await page.getByText("No results. Try a different word.", { exact: true }).waitFor();
          await capture(page, "installation-search-empty-" + suffix, "No results preserve a clear way to search again");
          await search.fill("repair");
          await search.fill("");
          assert.equal(await page.locator(".docs-search-results").innerText(), "", "Clearing cancels pending results");
          await search.fill("repair");
          await page.locator(".docs-search-results a").first().waitFor();
          await page.locator(".docs-search-results a").first().click();
          assert.equal(new URL(page.url()).pathname.replace(/\/$/, ""), "/docs/installation");
          // A failed module download must leave readable recovery and usable guide links.
          await context.route("**/pagefind/pagefind.js", (route) => route.fulfill({ status: 503, contentType: "text/javascript", body: "" }));
          await page.reload();
          await page.getByRole("searchbox", { name: "Search documentation" }).fill("repair");
          await page.getByText("Search couldn’t load. Try reloading, or use the guide links below.", { exact: true }).waitFor();
          await capture(page, "installation-search-unavailable-" + suffix, "Search failure retains navigation and explains recovery");
          await context.unroute("**/pagefind/pagefind.js");
          await page.reload();
          await page.getByRole("searchbox", { name: "Search documentation" }).fill("repair");
          await page.locator(".docs-search-results a").first().waitFor();
          assert.match(await page.locator(".docs-search-results").innerText(), /Installation/);
        }
      }
      // The 320 px reflow check catches overflow hidden by the normal mobile viewport.
      await page.setViewportSize({ width: 320, height: 844 });
      await page.goto(origin + "/docs/installation/");
      await noOverflow(page);
      if (layout === "compact") {
        await capture(page, "installation-320-" + colorScheme, "Installation at 320 CSS pixels", true);
        for (const heading of ["install-the-preview", "connect-another-agent", "start-work-when-you-are-ready"]) {
          await scrollHeading(page, "#" + heading);
          await capture(page, "installation-320-" + heading + "-" + colorScheme, "320 px " + heading);
        }
      }
      await page.goto(origin + "/docs/workflow/");
      await noOverflow(page);
      await legibility(page);
      if (layout === "compact") {
        await capture(page, "workflow-320-" + colorScheme, "Everyday-use guide at 320 CSS pixels", true);
        await scrollHeading(page, "#come-back-and-keep-going");
        await capture(page, "workflow-320-shortcuts-" + colorScheme, "Skill table at 320 CSS pixels");
      }
      await page.goto(origin + "/");
      await noOverflow(page);
      if (layout === "compact") {
        await capture(page, "home-320-" + colorScheme, "Home at 320 CSS pixels", true);
        await page.waitForFunction(()=>document.querySelector("[data-recording]").dataset.ready==="true");
        await capture(page,"tour-320-"+colorScheme,"Project explorer at 320 CSS pixels",false,".project-tour");
        await page.locator('[data-chapter="2"]').click();
        await noOverflow(page);
        await scrollHeading(page, ".build-recording");
        await capture(page, "recording-320-" + colorScheme, "Guided source excerpt at 320 CSS pixels", false, ".build-recording");
        await page.locator("#evidence > summary").click();
        for (const [index, tab] of (await page.locator(".product-evidence").getByRole("tab").all()).entries()) {
          await tab.click();
          const panel = page.locator(".evidence-panel:not([hidden])");
          await scrollHeading(page, "#" + await panel.getAttribute("id") + " .capture-excerpt");
          await capture(page, "home-transcript-320-" + index + "-" + colorScheme, "320 px transcript " + index);
        }
        await scrollHeading(page, ".walkthrough");
      }
      // A keyboard user can reach content without traversing the navigation.
      const skip = page.getByRole("link", { name: "Skip to content" });
      await skip.focus();
      await page.waitForFunction(() => document.querySelector(".skip-link").getBoundingClientRect().y >= 0, null, { timeout: 2000 });
      const skipState = await page.evaluate(() => ({ active: document.activeElement?.outerHTML, y: document.querySelector(".skip-link").getBoundingClientRect().y, scrollY }));
      assert(skipState.y >= 0, "Focused skip link: " + JSON.stringify(skipState));
      await page.keyboard.press("Enter");
      assert.equal(new URL(page.url()).hash, "#main-content");
      assert.deepEqual(errors, [], "No uncaught page errors");
      checks.push({ layout, appearance: colorScheme, passed: true, scenarios: ["nine static routes", "section navigation: keyboard, links and sticky-header clearance", "primary and theme-control contrast", "wrapped code and aligned documentation arrows", "recorded project: six guided chapters, exact source excerpts, recorded review comparison, all 47 actual actions, failed/passed checks, keyboard, pause and video seek; evidence tabs: pointer, arrows, Home, End, focus", "clipboard success and denial (browser stub)", "reflow at 320 px", "images", "install anchor", "theme persistence", "selected agent instructions", "lazy search, results, empty and failure recovery, cleared query", "keyboard skip link", ...(layout === "compact" ? ["mobile menu navigation, Escape, anchor focus and resize recovery"] : [])] });
      await context.close();
    }
  }
  for(const width of [820])for(const appearance of ["light","dark"]){
    const context=await browser.newContext({viewport:{width,height:1000},colorScheme:appearance,reducedMotion:"reduce"});
    const page=await context.newPage();await page.goto(origin+"/");
    await page.waitForFunction(()=>document.querySelector("[data-recording]").dataset.ready==="true");
    await capture(page,"tour-820-"+appearance,"Project explorer at the tablet breakpoint",false,".project-tour");
    await page.locator('[data-chapter="0"]').click();
    await noOverflow(page);
    await scrollHeading(page,".build-recording");
    await capture(page,"recording-"+width+"-"+appearance,"Saved project plan at the tablet breakpoint",false,".build-recording");
    await context.close();
  }
  const unavailableContext=await browser.newContext({viewport:{width:390,height:844},colorScheme:"light"});
  await unavailableContext.route("**/demo/inbox/recording.json?*",route=>route.fulfill({status:503,body:"Unavailable"}));
  const unavailablePage=await unavailableContext.newPage();
  await unavailablePage.goto(origin+"/");
  await unavailablePage.locator("[data-error]:visible").waitFor();
  assert.equal(await unavailablePage.locator(".story-controls:visible").count(),0);
  assert(await unavailablePage.locator(".build-recording .capture-window:visible").isVisible());
  await scrollHeading(unavailablePage,".build-recording");
  await capture(unavailablePage,"recording-unavailable","Failed recording download retains actual inbox capture and download links",false,".build-recording");
  await unavailableContext.close();
  const staticContext = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 390, height: 844 } });
  const staticPage = await staticContext.newPage();
  await staticPage.goto(origin + "/");
  assert.equal(await staticPage.locator(".recording-transcript li").count(),47,"Without JavaScript the full build transcript remains available");
  assert.equal(await staticPage.locator(".recording-toolbar:visible").count(),0,"No inert playback controls without JavaScript");
  assert(await staticPage.locator(".recording-footnote a").first().isVisible(),"Actual app video link remains available without JavaScript");
  await staticPage.locator("#evidence > summary").click();
  assert.equal(await staticPage.locator(".product-evidence [data-panel]:visible").count(), 3, "Recorded product evidence remains available without JavaScript");
  assert.equal(await staticPage.locator("[data-tablist]:visible").count(), 0, "No inert tab controls without JavaScript");
  assert.equal(await staticPage.locator("[data-project-tour] > [data-panel]:visible").count(),4,"All feature explanations remain readable without JavaScript");
  await capture(staticPage,"tour-without-javascript","Static project records and explanations without JavaScript",false,"#tour-roadmap");
  await scrollHeading(staticPage, ".walkthrough");
  await capture(staticPage, evidenceScope === "recording" ? "recording-without-javascript" : "home-without-javascript", "App recording and downloadable transcript remain available without JavaScript", false, evidenceScope === "recording" ? ".build-recording" : undefined);
  await staticContext.close();
  await writeReport("passed");
  console.log(JSON.stringify({ passed: true, profiles: checks.length, screenshots: captures.length, evidence }, null, 2));
} catch (error) {
  await writeReport("failed", error.message);
  throw error;
} finally {
  await browser.close();
  await new Promise((done) => server.close(done));
}
