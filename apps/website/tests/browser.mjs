import assert from "node:assert/strict";
import {createServer} from "node:http";
import {readFile,readdir,mkdir,writeFile,stat} from "node:fs/promises";
import {createHash} from "node:crypto";
import {resolve,dirname,extname,sep} from "node:path";
import {fileURLToPath} from "node:url";
import {webkit} from "playwright";
import {chapters} from "../src/data/build-recording.mjs";
import {explanationFor} from "../src/data/build-explanations.mjs";
const root=resolve(dirname(fileURLToPath(import.meta.url)),".."),repo=resolve(root,"../.."),dist=resolve(root,"dist");
const arg=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
const captureOnly=arg("--capture-only")?new RegExp(arg("--capture-only")):null;
const evidence=arg("--evidence-dir")?resolve(repo,arg("--evidence-dir")):null;
if(evidence){assert(evidence.startsWith(resolve(repo,"docs/evidence/desktop-iterations")+sep));await mkdir(evidence,{recursive:true});assert(!(await readdir(evidence)).some(n=>n.endsWith(".png")),"Use a new iteration folder");}
const mime={".html":"text/html",".css":"text/css",".js":"text/javascript",".json":"application/json",".png":"image/png",".svg":"image/svg+xml",".wasm":"application/wasm"};
const server=createServer(async(req,res)=>{try{let path=resolve(dist,"."+decodeURIComponent(new URL(req.url,"http://localhost").pathname));assert(path.startsWith(dist+sep)||path===dist);if((await stat(path)).isDirectory())path=resolve(path,"index.html");res.setHeader("Content-Type",mime[extname(path)]??"application/octet-stream");res.end(await readFile(path));}catch{res.writeHead(404);res.end("Not found");}});
let origin=arg("--base-url");if(!origin){await new Promise(r=>server.listen(0,"127.0.0.1",r));origin="http://127.0.0.1:"+server.address().port;}
const browser=await webkit.launch(),captures=[],checks=[],errors=[];
const externalAssets=new Map();
// Test the shipped fonts, but download each resource only once across isolated
// browser contexts. Analytics is unrelated to these checks and must not hold
// page.load open or count CI visits as real traffic.
async function configureNetwork(context){
 await context.route("https://plausible.io/**",route=>route.fulfill({status:204,body:""}));
 await context.route(/^https:\/\/fonts\.(?:googleapis|gstatic)\.com\//,async route=>{
  const url=route.request().url();
  if(!externalAssets.has(url))externalAssets.set(url,(async()=>{
   const response=await route.fetch({timeout:15000});
   assert(response.ok(),"Font request failed: "+url);
   return {status:response.status(),headers:{"content-type":response.headers()["content-type"],"access-control-allow-origin":"*"},body:await response.body()};
  })());
  await route.fulfill(await externalAssets.get(url));
 });
}
const recording=JSON.parse(await readFile(resolve(root,"public/demo/tic-tac-toe/recording.json"),"utf8")),parts=chapters(recording);
async function tree(path){const out=[];for(const e of await readdir(resolve(repo,path),{withFileTypes:true})){const p=path+"/"+e.name;if(e.isDirectory())out.push(...await tree(p));else if(/\.(astro|mjs|ts|css|mdx|json)$/.test(e.name))out.push(p);}return out;}
const sourcePaths=evidence?[...await tree("apps/website/src"),...await tree("crates/magistr-desktop/ui"),...await tree("apps/website/scripts"),"apps/website/astro.config.mjs","apps/website/package.json","apps/website/package-lock.json","apps/website/tests/browser.mjs","apps/website/public/demo/tic-tac-toe/recording.json","apps/website/public/demo/tic-tac-toe/manifest.json"]:[];
const sourceHashes=Object.fromEntries(await Promise.all(sourcePaths.map(async p=>[p,createHash("sha256").update(await readFile(resolve(repo,p))).digest("hex")])));
const stable=async p=>{await p.evaluate(()=>document.fonts.ready);await p.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));};
async function capture(page,name,state,{full=false,region,settle=true}={}){
 if(!evidence||(captureOnly&&!captureOnly.test(name)))return;
 if(region&&region!=="#build-details")await page.locator(region).evaluate(e=>scrollTo({top:e.getBoundingClientRect().top+scrollY-90,behavior:"instant"}));
 if(settle)await stable(page);await page.screenshot({path:resolve(evidence,name+".png"),fullPage:full});
 captures.push({file:name+".png",state,viewport:page.viewportSize(),appearance:await page.evaluate(()=>document.documentElement.classList.contains("dark")?"dark":"light"),capturedAt:new Date().toISOString()});
}
async function playbackLayout(page){
 const metrics=await page.evaluate(()=>{
  const rect=selector=>document.querySelector(selector).getBoundingClientRect();
  const transport=rect(".build-transport"),annotation=rect(".build-annotation"),slider=rect(".build-timeline input");
  return {width:innerWidth,transportBottom:transport.bottom,annotationTop:annotation.top,sliderWidth:slider.width};
 });
 if(metrics.width<=1000)assert(metrics.transportBottom<=metrics.annotationTop+1,"Playback controls precede the mobile annotation");
 if(metrics.width<=600)assert(metrics.sliderWidth>230,"Compact timeline has room to scrub precisely");
}
async function noOverflow(page,label){assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),label+" has no horizontal overflow");}
async function seek(page,index){await page.locator('[aria-label="Recorded action"]').evaluate((e,n)=>{e.value=String(n);e.dispatchEvent(new Event("input",{bubbles:true}));},index);await page.waitForFunction(i=>document.querySelector("[data-terminal]").dataset.renderedIndex===String(i),index);}
const changeIndex=recording.events.findLastIndex(e=>e.file?.path==="src/main.rs");
const checkIndex=recording.events.findIndex(e=>e.check?.verified&&!e.check.passed);
const diagnosticIndex=recording.events.findIndex(e=>e.output.includes("collapsible_if"));
const openView=async(page,name)=>{const button=page.locator('[data-view="'+name+'"]');if(await button.getAttribute("aria-expanded")!=="true")await button.click();};
let status="failed",failure="";
try{
 for(const [profile,width,height]of [["wide",1280,1000],["compact",390,844]]){
  for(const appearance of ["light","dark"]){
   const profileStarted=performance.now();
   const context=await browser.newContext({viewport:{width,height},colorScheme:appearance,reducedMotion:"reduce",deviceScaleFactor:1});
   await configureNetwork(context);
   await context.addInitScript(theme=>{
    localStorage.setItem("theme",theme);
    Object.defineProperty(navigator,"clipboard",{value:{writeText:async text=>{window.__copied=text;}}});
   },appearance);
   const page=await context.newPage();page.on("pageerror",e=>errors.push(String(e)));
   await page.goto(origin);await page.locator('[data-build][data-ready="true"]').waitFor();await stable(page);
   const suffix=profile+"-"+appearance;
   assert.equal(await page.locator("[data-build]").getAttribute("data-index"),"0","The session opens with the human request");
   await capture(page,"conversation-start-"+suffix,"First request, fixed transport and matching explanation",{region:".build-window"});
   await seek(page,recording.events.findIndex(e=>e.id==="message-5"));
   await capture(page,"conversation-followup-"+suffix,"Human version follow-up at its real point in the build",{region:".build-window"});
   assert(await page.locator(".xterm").isVisible(),"A terminal emulator is mounted");
   await noOverflow(page,"Home "+suffix);await playbackLayout(page);
   const annotation=page.locator("[data-annotation]");
   const scenes=[
    ["action-11","recheck","all"],["action-5","plan","a"],["change-65","source","b"],
    ["action-16","rerun","a"],["action-19","experiment","b"],["action-21","decision","a"],
    ["action-24","conclusion","b"],["action-31","layout","a"],["action-46","timeout","b"],
    ["action-49","cleanup","a"],["action-56","complete","b"]
   ];
   const pair=(profile==="wide"&&appearance==="light")||(profile==="compact"&&appearance==="dark")?"a":"b";
   for(const [id,name,group]of scenes){
    if(group!=="all"&&group!==pair)continue;
    await seek(page,recording.events.findIndex(e=>e.id===id));await noOverflow(page,"Annotation "+id+" "+suffix);
    const explanation=explanationFor(id);
    assert.equal(await page.locator("[data-action-title]").textContent(),explanation.title);
    const text=await page.locator("[data-action-explanation]").textContent();
    assert(text.includes(explanation.summary.replaceAll("`","")),id+" explains the selected action");
    assert(!/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f-]{23}|ttt-(rules|opponent|board|rematch)|current tree/.test(await annotation.textContent()));
    assert(await annotation.evaluate(e=>e.scrollWidth<=e.clientWidth+1));
    if(name==="recheck")await capture(page,"terminal-recheck-"+suffix,"Selected completion refusal with hanging output and matching explanation",{region:".build-window"});
    await capture(page,"annotation-"+name+"-"+suffix,"Plain-language explanation for "+id,{region:width>1000?".build-window":"[data-annotation]"});
   }
   if(profile==="wide"&&appearance==="dark"){
    const columns=await page.locator("[data-terminal]").getAttribute("data-columns");
    await page.setViewportSize({width:390,height:844});
    await page.waitForFunction(n=>Number(document.querySelector("[data-terminal]").dataset.columns)<Number(n),columns);
    await noOverflow(page,"Live terminal resize");
    await page.setViewportSize({width,height});
    await page.waitForFunction(n=>document.querySelector("[data-terminal]").dataset.columns===n,columns);
   }
   await seek(page,parts[0].index);
   await capture(page,"terminal-plan-"+suffix,"Terminal scrollback beside the four planned pieces",{region:".build-window"});

   await seek(page,changeIndex);
   await capture(page,"terminal-diff-"+suffix,"Actual Rust changes, syntax colors and synchronized annotation",{region:".build-window"});
   // Exercise the full journey once per size, including both code palettes.
   // Every profile still checks layout, annotations and diffs; capture runs
   // retain every reader state in every profile for direct visual review.
   if(pair==="a"||evidence){
   await page.locator("[data-open-action]").click();
   assert(await page.locator("#build-details").evaluate(e=>e.open&&e.matches(":modal")),"Details use a native modal dialog");
   const scrollBefore=await page.evaluate(()=>scrollY);
   await page.keyboard.press("Tab");
   assert(await page.locator("#build-details").evaluate(e=>e.contains(document.activeElement)),"Focus remains in the inspector");
   assert(await page.locator(".record-reader .diff-add").count()>0);
   assert(await page.locator(".record-reader .diff-remove").count()>0);
   assert(await page.locator(".record-reader .diff-line span[style]").count()>10);
   await capture(page,"diff-reader-"+suffix,"Added and removed Rust lines with hunk context",{region:"#build-details"});
   await page.getByRole("button",{name:"Full file",exact:true}).click();
   assert.equal(await page.locator(".record-reader pre").textContent(),recording.events[changeIndex].file.content);
   await page.getByRole("button",{name:"Copy file",exact:true}).click();
   assert.equal(await page.evaluate(()=>window.__copied),recording.events[changeIndex].file.content);
   await capture(page,"source-reader-"+suffix,"Highlighted recorded Rust with exact source copying",{region:"#build-details"});
   if(profile==="compact")assert(await page.locator(".record-reader pre").evaluate(e=>{e.scrollLeft=80;const moved=e.scrollLeft>0;e.scrollLeft=0;return moved;}),"Long source lines scroll within the file reader");
   const codeSpan=page.locator(".record-reader pre span[style]").first();
   const beforeColor=await codeSpan.evaluate(e=>getComputedStyle(e).color);
   await page.evaluate(()=>document.documentElement.classList.toggle("dark"));
   await page.waitForFunction(theme=>document.querySelector("[data-terminal]").dataset.appearance!==theme,appearance);
   assert.notEqual(await codeSpan.evaluate(e=>getComputedStyle(e).color),beforeColor,"Open code reader responds to the theme toggle");
   await page.evaluate(()=>document.documentElement.classList.toggle("dark"));
   await page.waitForFunction(theme=>document.querySelector("[data-terminal]").dataset.appearance===theme,appearance);
   await page.getByRole("button",{name:"Close details",exact:true}).click();
   assert(await page.locator("#build-details").isHidden());
   assert.equal(await page.evaluate(()=>scrollY),scrollBefore,"Closing the dialog preserves page position");
   assert(await page.locator("[data-open-action]").evaluate(e=>e===document.activeElement));

   await seek(page,recording.events.findLastIndex(e=>e.operation==="roadmap_add_chunk"));await openView(page,"roadmap");
   assert(await page.locator(".xterm").isVisible(),"Project visuals never replace the terminal");
   await capture(page,"roadmap-"+suffix,"Roadmap remains attached to the terminal at the selected action",{region:"#build-details"});
   await page.locator("[data-record-view]").getByRole("button",{name:recording.events.find(e=>e.chunk).chunk.title,exact:width>1000}).click();
   assert((await page.locator(".record-reader").textContent()).includes("Done when"));
   await page.keyboard.press("Escape");assert(await page.locator("#build-details").isHidden());
   assert(await page.locator('[data-view="roadmap"]').evaluate(e=>e===document.activeElement));

   await seek(page,parts[2].index);await openView(page,"reasoning");
   await capture(page,"reasoning-"+suffix,"Recorded opponent investigation and its conclusion",{region:"#build-details"});
   await page.locator("[data-record-view]").getByRole("button",{name:"The short policy misses a fork when it plays second",exact:width>1000}).click();
   assert((await page.locator(".record-reader").textContent()).includes("Why"));
   await page.getByRole("button",{name:"Close details",exact:true}).click();

   await seek(page,checkIndex);await page.locator("[data-open-action]").click();
   assert(await page.locator(".record-reader .output-command span[style]").count()>0);
   assert(await page.locator(".record-reader .output-error").count()>0);
   await capture(page,"check-reader-"+suffix,"Failed command with highlighted shell input and actual failure output",{region:"#build-details"});
   await page.getByRole("button",{name:"Close details",exact:true}).click();
   await seek(page,diagnosticIndex);
   await capture(page,"terminal-diagnostic-"+suffix,"Compiler error with highlighted Rust and synchronized failed check",{region:".build-window"});
   await page.locator("[data-open-action]").click();
   assert(await page.locator(".record-reader .output-code span[style]").count()>10);
   await capture(page,"diagnostic-reader-"+suffix,"Rust code inside the complete compiler diagnostic",{region:"#build-details"});
   await page.getByRole("button",{name:"Close details",exact:true}).click();
   await seek(page,recording.events.length-1);await openView(page,"work");
   assert.equal(await page.locator(".record-list>li").count(),4);
   await page.locator(".record-list button").first().click();
   await page.locator(".record-reader button").filter({hasText:"Passed:"}).first().click();
   await page.getByRole("button",{name:"Back to work",exact:false}).click();
   assert.equal(await page.locator(".record-list>li").count(),4);
   await page.getByRole("button",{name:"Close details",exact:true}).click();

   // Coalesce quick seeks; neither the terminal nor project readers may retain the future.
   await page.locator('[aria-label="Recorded action"]').evaluate(e=>{for(const i of [60,5,45,0]){e.value=String(i);e.dispatchEvent(new Event("input",{bubbles:true}));}});
   await page.waitForFunction(()=>document.querySelector("[data-terminal]").dataset.renderedIndex==="0");
   assert(!(await page.locator("[data-terminal]").textContent()).includes("Tactical policy as"));
   await capture(page,"terminal-start-"+suffix,"Opening human request in the Codex prompt gutter",{region:".build-window"});
   await openView(page,"checks");assert((await page.locator("[data-record-view]").textContent()).includes("No checks"));
   await page.getByRole("button",{name:"Close details",exact:true}).click();
   await page.locator(".xterm-helper-textarea").focus();await page.keyboard.press("ArrowRight");
   await page.waitForFunction(()=>document.querySelector("[data-terminal]").dataset.renderedIndex==="1");
   await page.keyboard.press(" ");
   await page.waitForFunction(()=>Number(document.querySelector("[data-terminal]").dataset.renderedIndex)>1,undefined,{timeout:10000}).catch(async error=>{
    const state=await page.evaluate(()=>({active:document.activeElement?.outerHTML,hidden:document.hidden,play:document.querySelector("[data-play]")?.outerHTML,index:document.querySelector("[data-build]")?.dataset.index,rendered:document.querySelector("[data-terminal]")?.dataset.renderedIndex}));
    throw new Error(suffix+" keyboard playback stalled: "+JSON.stringify(state),{cause:error});
   });
   await page.getByRole("button",{name:"Pause",exact:true}).click();
   assert.equal(await page.getByRole("button",{name:"Play",exact:true}).getAttribute("aria-pressed"),"false");

   // Keep pointer targets stationary through every message, long annotation,
   // failed check and source change. One batch avoids 98 browser round trips.
   await seek(page,0);
   await page.locator(".build-transport").scrollIntoViewIfNeeded();
   const stability=await page.evaluate(async count=>{
    const target=document.querySelector("[data-next]"),slider=document.querySelector('[aria-label="Recorded action"]');
    const first=target.getBoundingClientRect(),failures=[];
    for(let i=0;i<count;i++){
     slider.value=String(i);slider.dispatchEvent(new Event("input",{bubbles:true}));
     for(let wait=0;document.querySelector("[data-terminal]").dataset.renderedIndex!==String(i);wait++){
      if(wait>120)throw Error("Seek stalled at "+i);
      await new Promise(r=>requestAnimationFrame(r));
     }
     const r=target.getBoundingClientRect();
     if(Math.abs(r.top-first.top)>1||Math.abs(r.left-first.left)>1)failures.push({i,top:r.top,left:r.left});
    }
    return failures;
   },recording.events.length);
   assert.deepEqual(stability,[],"Step controls stay put through the whole session");
   await page.locator("[data-open-transcript]").click();
   assert.equal(await page.locator(".session-conversation>li").count(),recording.events.length);
   assert.equal(await page.locator('.session-conversation [data-role="user"]').count(),2);
   await capture(page,"session-dialog-"+suffix,"Full conversation with timestamps that return to the selected moment",{region:"#build-details"});
   await page.keyboard.press("Escape");

   if(pair==="a"){
    await page.emulateMedia({reducedMotion:"no-preference"});
    await seek(page,1);await page.locator("[data-speed]").selectOption("1");
    await page.locator("[data-play]").click();
    await page.evaluate(()=>{const play=document.querySelector("[data-play]");for(let i=0;i<10;i++){play.click();play.click();}});
    await page.waitForFunction(()=>Number(document.querySelector("[data-terminal]").dataset.revealed)>5);
    await page.locator("[data-play]").click();
    await page.waitForTimeout(80); // Allow the single acknowledged write to settle.
    const paused=await page.locator("[data-terminal]").getAttribute("data-revealed");
    await page.waitForTimeout(160);
    assert.equal(await page.locator("[data-terminal]").getAttribute("data-revealed"),paused,"Pause freezes partial text");
    await page.locator("[data-open-action]").click();
    await page.keyboard.press("Escape");
    await page.waitForTimeout(80);
    assert.equal(await page.locator("[data-terminal]").getAttribute("data-revealed"),paused,"Opening and closing details preserves partial text");
    await capture(page,"stream-paused-"+suffix,"Paused reply after opening and closing its message inspector",{region:".build-window"});
    await page.locator("[data-play]").click();
    await page.waitForFunction(n=>Number(document.querySelector("[data-terminal]").dataset.revealed)>Number(n),paused);
    await page.locator("[data-speed]").selectOption("4");
    await seek(page,0);
    await page.waitForTimeout(100);
    assert.equal(await page.locator("[data-terminal]").getAttribute("data-rendered-index"),"0","Scrubbing cancels an active stream");
    assert(!(await page.locator("[data-terminal]").textContent()).includes("playable board"),"Canceled text cannot arrive after a seek");
    await seek(page,recording.events.length-1);await page.locator("[data-speed]").selectOption("1");
    await page.locator("[data-play]").click();
    await page.waitForFunction(()=>Number(document.querySelector("[data-terminal]").dataset.revealed)>5);
    await page.locator("[data-play]").click();
    await capture(page,"stream-final-paused-"+suffix,"Pause the final handover and resume it without restarting the session",{region:".build-window"});
    await page.locator("[data-speed]").selectOption("4");await page.locator("[data-play]").click();
    assert.equal(await page.locator("[data-build]").getAttribute("data-index"),String(recording.events.length-1),"Resume stays on the incomplete final reply");
    await page.getByRole("button",{name:"Play",exact:true}).waitFor();
    await page.locator("[data-play]").click();
    await page.waitForFunction(()=>document.querySelector("[data-build]").dataset.index==="0");
    await page.locator("[data-play]").click();
    await page.emulateMedia({reducedMotion:"reduce"});
   }
   checks.push("Modal focus/return, complete conversation, stationary controls, streaming pause/resume/cancel, reduced motion, copy and keyboard playback: "+suffix);
   }

   await capture(page,"install-"+suffix,"Highlighted installation commands",{region:"#install"});
   await page.getByRole("button",{name:"Copy commands",exact:true}).click();
   assert.equal(await page.evaluate(()=>window.__copied),await page.locator("#source-install").textContent());
   await capture(page,"install-copied-"+suffix,"Readable copy confirmation and focused copy control",{region:"#install"});
   if(profile==="compact"){
    await page.locator("[data-menu-toggle]").click();await page.keyboard.press("Escape");
    assert.equal(await page.locator("[data-menu-toggle]").getAttribute("aria-expanded"),"false");
   }
   checks.push("Terminal rendering, real diffs, layout and annotations: "+suffix);
   for(const slug of ["installation","workflow","roadmap","reasoning","chunks","verification","concepts"]){
    await page.goto(origin+"/docs/"+slug+"/");await stable(page);await noOverflow(page,slug+" "+suffix);
    assert.equal(await page.locator("main h1").count(),1);
    if(["installation","chunks","verification","reasoning"].includes(slug)){
     assert(await page.locator(".astro-code").count()>0);
     await capture(page,"docs-"+slug+"-"+suffix,"Theme-aware code in the "+slug+" guide",{full:true});
    }
   }
   const search=page.locator('input[type="search"]');await search.fill("minimax");
   await page.waitForFunction(()=>document.querySelectorAll(".docs-search-results a").length>0);
   await search.fill("zzzz-no-such-doc");await page.waitForFunction(()=>document.querySelector(".docs-search-results")?.textContent?.includes("No results"));
   await page.goto(origin+"/404.html");assert(await page.getByRole("heading",{name:"Page not found"}).isVisible());
   checks.push("Seven guides, code themes, search, reflow and 404: "+suffix);await context.close();
   console.log(suffix+" browser checks passed in "+Math.round(performance.now()-profileStarted)+"ms");
  }
 }
 for(const width of [320,820])for(const appearance of ["light","dark"]){
  const page=await browser.newPage({viewport:{width,height:1000},colorScheme:appearance,reducedMotion:"reduce"});
  await configureNetwork(page.context());
  await page.addInitScript(theme=>localStorage.setItem("theme",theme),appearance);await page.goto(origin);await page.locator('[data-build][data-ready="true"]').waitFor();
  await seek(page,changeIndex);await noOverflow(page,"Terminal "+width);await playbackLayout(page);
  await capture(page,"reflow-"+width+"-"+appearance,"Readable terminal and controls at "+width+" pixels",{region:".build-window"});
  await page.locator("[data-open-action]").click();await noOverflow(page,"Diff "+width);
  await page.close();
 }
 const nojs=await browser.newPage({javaScriptEnabled:false,viewport:{width:390,height:844}});
 await configureNetwork(nojs.context());
 await nojs.goto(origin);
 await capture(nojs,"terminal-static","Codex-style grouped output without JavaScript",{region:".build-window",settle:false});
 await nojs.locator(".build-transcript>summary").click();
 await nojs.locator('[data-event="'+recording.events[changeIndex].id+'"]>details>summary').click();
 assert(await nojs.locator('[data-event="'+recording.events[changeIndex].id+'"] .diff-add').count()>0);
 await capture(nojs,"no-javascript","Highlighted source and diff transcript without JavaScript",{region:'[data-event="'+recording.events[changeIndex].id+'"]',settle:false});
 await nojs.locator('[data-event="action-11"]>details>summary').click();
 assert((await nojs.locator('[data-event="action-11"] .action-explanation').textContent()).includes("Why Magistr stopped"));
 await capture(nojs,"annotation-transcript","Formatted explanation is available without JavaScript",{region:'[data-event="action-11"]',settle:false});
 await nojs.close();
 const offline=await browser.newPage({viewport:{width:1280,height:1000},colorScheme:"dark"});
 await configureNetwork(offline.context());
 await offline.route("**/demo/tic-tac-toe/playback.json*",r=>r.abort());await offline.goto(origin);
 await offline.locator('[data-build][data-ready="error"]').waitFor();assert(await offline.locator(".build-error").isVisible());
 await capture(offline,"recording-unavailable","Readable fallback when playback fails to load",{region:".build-window"});await offline.close();
 assert.deepEqual(errors,[]);status="passed";
 console.log("Website browser checks passed: terminal emulation, synchronized annotations, highlighting, rapid scrubbing, exact copy, four profiles, all guides and recovery.");
}catch(error){failure=String(error);throw error;}finally{
 if(evidence)await writeFile(resolve(evidence,"browser-checks.json"),JSON.stringify({status,failure,capturedAt:new Date().toISOString(),renderer:"Playwright WebKit",rendererVersion:browser.version(),origin,sourceHashes,checks,captures,limitations:["Headless WebKit only; native OS input and assistive technology not tested.","Clipboard contents verified through a browser stub; OS clipboard not changed.","Screenshots require a separate visual review."]},null,2)+"\n");
 await browser.close();server.close();
}
