import assert from "node:assert/strict";
import {createServer} from "node:http";
import {readFile,readdir,mkdir,writeFile,stat} from "node:fs/promises";
import {createHash} from "node:crypto";
import {resolve,dirname,extname,sep} from "node:path";
import {fileURLToPath} from "node:url";
import {webkit} from "playwright";
import {chapters} from "../src/data/build-recording.mjs";
const root=resolve(dirname(fileURLToPath(import.meta.url)),".."),repo=resolve(root,"../.."),dist=resolve(root,"dist");
const arg=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
const captureOnly=arg("--capture-only")?new RegExp(arg("--capture-only")):null;
const evidence=arg("--evidence-dir")?resolve(repo,arg("--evidence-dir")):null;
if(evidence){assert(evidence.startsWith(resolve(repo,"docs/evidence/desktop-iterations")+sep));await mkdir(evidence,{recursive:true});assert(!(await readdir(evidence)).some(n=>n.endsWith(".png")),"Use a new iteration folder");}
const mime={".html":"text/html",".css":"text/css",".js":"text/javascript",".json":"application/json",".png":"image/png",".svg":"image/svg+xml",".wasm":"application/wasm"};
const server=createServer(async(req,res)=>{try{let path=resolve(dist,"."+decodeURIComponent(new URL(req.url,"http://localhost").pathname));assert(path.startsWith(dist+sep)||path===dist);if((await stat(path)).isDirectory())path=resolve(path,"index.html");res.setHeader("Content-Type",mime[extname(path)]??"application/octet-stream");res.end(await readFile(path));}catch{res.writeHead(404);res.end("Not found");}});
let origin=arg("--base-url");if(!origin){await new Promise(r=>server.listen(0,"127.0.0.1",r));origin="http://127.0.0.1:"+server.address().port;}
const browser=await webkit.launch(),captures=[],checks=[],errors=[];
const recording=JSON.parse(await readFile(resolve(root,"public/demo/tic-tac-toe/recording.json"),"utf8")),parts=chapters(recording);
async function tree(path){const out=[];for(const e of await readdir(resolve(repo,path),{withFileTypes:true})){const p=path+"/"+e.name;if(e.isDirectory())out.push(...await tree(p));else if(/\.(astro|mjs|ts|css|mdx|json)$/.test(e.name))out.push(p);}return out;}
const sourcePaths=[...await tree("apps/website/src"),...await tree("crates/magistr-desktop/ui"),"apps/website/tests/browser.mjs","apps/website/public/demo/tic-tac-toe/recording.json","apps/website/public/demo/tic-tac-toe/manifest.json"];
const sourceHashes=Object.fromEntries(await Promise.all(sourcePaths.map(async p=>[p,createHash("sha256").update(await readFile(resolve(repo,p))).digest("hex")])));
const stable=async p=>{await p.evaluate(()=>document.fonts.ready);await p.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));};
async function capture(page,name,state,{full=false,region}={}){
 if(!evidence||(captureOnly&&!captureOnly.test(name)))return;
 if(region)await page.locator(region).evaluate(e=>scrollTo({top:e.getBoundingClientRect().top+scrollY-90,behavior:"instant"}));
 if(name!=="no-javascript")await stable(page);await page.screenshot({path:resolve(evidence,name+".png"),fullPage:full});
 captures.push({file:name+".png",state,viewport:page.viewportSize(),appearance:await page.evaluate(()=>document.documentElement.classList.contains("dark")?"dark":"light"),capturedAt:new Date().toISOString()});
}
async function noOverflow(page,label){assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),label+" has no horizontal overflow");}
async function seek(page,index){await page.locator('[aria-label="Recorded action"]').evaluate((e,n)=>{e.value=String(n);e.dispatchEvent(new Event("input",{bubbles:true}));},index);await page.waitForFunction(i=>document.querySelector("[data-build]").dataset.index===String(i),index);}
let status="failed",failure="";
try{
 for(const [profile,width,height]of [["wide",1280,1000],["compact",390,844]]){
  for(const appearance of ["light","dark"]){
   const context=await browser.newContext({viewport:{width,height},colorScheme:appearance,reducedMotion:"reduce",deviceScaleFactor:1});
   await context.addInitScript(theme=>localStorage.setItem("theme",theme),appearance);
   const page=await context.newPage();page.on("pageerror",e=>errors.push(String(e)));
   await page.goto(origin);await page.locator('[data-build][data-ready="true"]').waitFor();await stable(page);
   const suffix=profile+"-"+appearance;
   await noOverflow(page,"Home "+suffix);
   await capture(page,"home-"+suffix,"Hero, favorite-agent subtitle and game introduction");
   await capture(page,"session-"+suffix,"Codex session at the first complete roadmap",{region:".build-window"});
   await page.getByRole("tab",{name:"Roadmap",exact:true}).click();
   await capture(page,"roadmap-"+suffix,"Recorded four-piece roadmap and actual prerequisites",{region:".build-window"});
   const chunk=page.locator("[data-record-view]").getByRole("button",{name:recording.events.find(e=>e.chunk).chunk.title,exact:width>1000});
   await chunk.click();assert(await page.locator(".record-reader").textContent().then(t=>t.includes("Done when")));
   await capture(page,"roadmap-reader-"+suffix,"Read the actual rules chunk and its acceptance criteria",{region:".build-window"});
   await page.getByRole("tab",{name:"Session",exact:true}).click();
   await seek(page,parts[2].index);await page.getByRole("tab",{name:"Reasoning",exact:true}).click();
   await capture(page,"reasoning-"+suffix,"Opponent investigation and its recorded conclusion",{region:".build-window"});
   const finding=page.locator("[data-record-view]").getByRole("button",{name:"The short policy misses a fork when it plays second",exact:width>1000});
   await finding.click();assert((await page.locator(".record-reader").textContent()).includes("Why"));
   await capture(page,"reasoning-reader-"+suffix,"Read the losing-policy finding and its rationale",{region:".build-window"});
   await seek(page,recording.events.length-1);await page.getByRole("tab",{name:"Work",exact:true}).click();
   assert.equal(await page.locator(".record-list>li").count(),4);
   await capture(page,"work-"+suffix,"Four completed implementation tasks",{region:".build-window"});
   await page.locator(".record-list button").first().click();
   await capture(page,"work-reader-"+suffix,"Read the rules task and its actual checks",{region:".build-window"});
   await page.locator(".record-reader button").filter({hasText:"Passed:"}).first().click();
   await capture(page,"work-check-"+suffix,"Inspect a check from Work and return to that view",{region:".build-window"});
   await page.getByRole("button",{name:"Back to work",exact:false}).click();assert.equal(await page.locator(".record-list>li").count(),4);
   await page.getByRole("tab",{name:"Checks",exact:true}).click();
   assert((await page.locator(".record-list").textContent()).includes("Failed"));
   await capture(page,"checks-"+suffix,"Executed checks and written reviews, including earlier failures",{region:".build-window"});
   const failed=page.locator(".record-list button").filter({hasText:"Failed"}).first();await failed.click();
   await capture(page,"check-reader-"+suffix,"Earlier failed check retained with its actual output",{region:".build-window"});
   await page.getByRole("tab",{name:"Session",exact:true}).click();
   const change=recording.events.findIndex(e=>e.kind==="change");await seek(page,change);
   await page.getByRole("button",{name:"Read the recorded file",exact:true}).click();assert((await page.locator(".record-reader pre").textContent()).length>50);
   await page.getByRole("button",{name:"Back to session",exact:false}).click();assert(await page.locator("[data-session]").isVisible());
   await seek(page,0);await page.getByRole("tab",{name:"Checks",exact:true}).click();assert((await page.locator("[data-record-view]").textContent()).includes("No checks"));
   await page.getByRole("tab",{name:"Session",exact:true}).focus();await page.keyboard.press("ArrowRight");assert.equal(await page.getByRole("tab",{name:"Roadmap",exact:true}).getAttribute("aria-selected"),"true");
   await page.getByRole("button",{name:"Play steps",exact:true}).click();await page.waitForFunction(()=>Number(document.querySelector("[data-build]").dataset.index)>0);await page.getByRole("button",{name:"Pause",exact:true}).click();
   await capture(page,"followthrough-"+suffix,"Saved work, feedback and source tools",{region:"#product"});
   await capture(page,"install-"+suffix,"Agent choice and source-preview installation",{region:"#agents"});
   if(profile==="compact"){
    await page.locator("[data-menu-toggle]").click();assert.equal(await page.locator("[data-menu-toggle]").getAttribute("aria-expanded"),"true");await capture(page,"menu-open-"+suffix,"Mobile navigation open");await page.keyboard.press("Escape");assert.equal(await page.locator("[data-menu-toggle]").getAttribute("aria-expanded"),"false");await capture(page,"menu-closed-"+suffix,"Escape closes navigation and returns focus to its button");
   }
   checks.push("Playback, records, keyboard tabs, future-state protection, source reader and navigation: "+suffix);
   for(const slug of ["installation","workflow","roadmap","reasoning","chunks","verification","concepts"]){
    await page.goto(origin+"/docs/"+slug+"/");await stable(page);await noOverflow(page,slug+" "+suffix);assert.equal(await page.locator("main h1").count(),1);
    await capture(page,"docs-"+slug+"-"+suffix,"Complete "+slug+" guide",{full:true});
   }
   const search=page.locator('input[type="search"]');await search.fill("minimax");
   await page.waitForFunction(()=>document.querySelectorAll(".docs-search-results a").length>0);assert((await page.locator(".docs-search-results").textContent()).length>5);
   await search.fill("zzzz-no-such-doc");await page.waitForFunction(()=>document.querySelector(".docs-search-results")?.textContent?.includes("No results"));
   await page.goto(origin+"/404.html");assert(await page.getByRole("heading",{name:"Page not found"}).isVisible());
   checks.push("Seven guides, reflow, real search and 404: "+suffix);await context.close();
  }
 }
 for(const width of [320,820])for(const appearance of ["light","dark"]){
  const page=await browser.newPage({viewport:{width,height:900},colorScheme:appearance,reducedMotion:"reduce"});await page.addInitScript(theme=>localStorage.setItem("theme",theme),appearance);await page.goto(origin);await page.locator('[data-build][data-ready="true"]').waitFor();
  await page.getByRole("tab",{name:"Roadmap",exact:true}).click();await noOverflow(page,"Roadmap "+width);
  assert(await page.locator(".build-views").evaluate(e=>{const bounds=e.getBoundingClientRect();return [...e.querySelectorAll("button")].every(b=>{const r=b.getBoundingClientRect();return r.left>=bounds.left&&r.right<=bounds.right;});}),"All tabs stay inside the session at "+width);
  await capture(page,"reflow-"+width+"-"+appearance,"Roadmap and transport at "+width+" pixels",{region:".build-window"});await page.close();
 }
 const nojs=await browser.newPage({javaScriptEnabled:false,viewport:{width:390,height:844}});await nojs.goto(origin);await nojs.locator(".build-transcript>summary").click();assert(await nojs.locator(".build-transcript ol").isVisible());await capture(nojs,"no-javascript","Recorded actions remain readable without scripts",{region:".build-transcript"});await nojs.close();
 const offline=await browser.newPage({viewport:{width:390,height:844}});await offline.route("**/demo/tic-tac-toe/recording.json*",r=>r.abort());await offline.goto(origin);await offline.locator('[data-build][data-ready="error"]').waitFor();assert(await offline.locator(".build-error").isVisible());await capture(offline,"recording-unavailable","Readable fallback when the recording request fails",{region:".build-window"});await offline.close();
 assert.deepEqual(errors,[]);status="passed";console.log("Website browser checks passed: four profiles, all guides, playback, actual records, search, keyboard controls, reflow and recovery.");
}catch(error){failure=String(error);throw error;}finally{
 if(evidence)await writeFile(resolve(evidence,"browser-checks.json"),JSON.stringify({status,failure,capturedAt:new Date().toISOString(),renderer:"Playwright WebKit",rendererVersion:browser.version(),origin,sourceHashes,checks,captures,limitations:["Headless WebKit only; native OS input and assistive technology not tested.","Screenshots require a separate visual review."]},null,2)+"\n");
 await browser.close();server.close();
}
