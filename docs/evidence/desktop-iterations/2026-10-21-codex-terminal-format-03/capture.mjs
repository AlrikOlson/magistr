// Focused recapture for the static terminal's hanging-list indentation.
// Run from the repository: node docs/evidence/desktop-iterations/2026-10-21-codex-terminal-format-03/capture.mjs
import assert from "node:assert/strict";
import {createServer} from "node:http";
import {readFile,readdir,writeFile,stat} from "node:fs/promises";
import {createHash} from "node:crypto";
import {createRequire} from "node:module";
import {resolve,dirname,extname,sep} from "node:path";
import {fileURLToPath} from "node:url";
const folder=dirname(fileURLToPath(import.meta.url)),repo=resolve(folder,"../../../.."),dist=resolve(repo,"apps/website/dist");
const {webkit}=createRequire(resolve(repo,"apps/website/package.json"))("playwright");
assert(!(await readdir(folder)).some(f=>f.endsWith(".png")),"Do not overwrite iteration evidence");
const mime={".html":"text/html",".css":"text/css",".js":"text/javascript",".json":"application/json",".woff2":"font/woff2"};
const server=createServer(async(req,res)=>{try{
 let p=resolve(dist,"."+new URL(req.url,"http://localhost").pathname);assert(p.startsWith(dist+sep)||p===dist);
 if((await stat(p)).isDirectory())p=resolve(p,"index.html");
 res.setHeader("Content-Type",mime[extname(p)]??"application/octet-stream");res.end(await readFile(p));
}catch{res.writeHead(404);res.end();}});
await new Promise(r=>server.listen(0,"127.0.0.1",r));
const origin="http://127.0.0.1:"+server.address().port,browser=await webkit.launch(),captures=[];
async function tree(path){const files=[];for(const e of await readdir(resolve(repo,path),{withFileTypes:true})){const p=path+"/"+e.name;if(e.isDirectory())files.push(...await tree(p));else files.push(p);}return files;}
const sources=[...await tree("apps/website/src"),...await tree("apps/website/scripts"),"apps/website/package.json","apps/website/package-lock.json","apps/website/public/demo/tic-tac-toe/recording.json"];
const sourceHashes=Object.fromEntries(await Promise.all(sources.map(async p=>[p,createHash("sha256").update(await readFile(resolve(repo,p))).digest("hex")])));
try{
 for(const [profile,width,height]of [["wide",1280,1000],["compact",390,844]])for(const appearance of ["light","dark"]){
  const nojs=appearance==="light",page=await browser.newPage({viewport:{width,height},colorScheme:appearance,javaScriptEnabled:!nojs,reducedMotion:"reduce"});
  if(!nojs)await page.route("**/demo/tic-tac-toe/playback.json*",r=>r.abort());
  await page.goto(origin);await page.waitForLoadState("networkidle");
  if(!nojs)await page.locator('[data-build][data-ready="error"]').waitFor();
  assert(await page.locator("[data-terminal-poster]").isVisible());
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
  assert(await page.locator(".terminal-row").evaluateAll(rows=>rows.some(row=>getComputedStyle(row.lastElementChild).textIndent==="-15.6px"||parseFloat(getComputedStyle(row.lastElementChild).textIndent)<0)));
  await page.locator(".build-window").evaluate(e=>scrollTo(0,e.getBoundingClientRect().top+scrollY-90));
  const file="terminal-static-"+profile+"-"+appearance+".png";await page.screenshot({path:resolve(folder,file)});
  captures.push({file,viewport:{width,height},appearance,capturedAt:new Date().toISOString(),state:nojs?"JavaScript disabled":"Recording request fails"});await page.close();
 }
 await writeFile(resolve(folder,"browser-checks.json"),JSON.stringify({status:"passed",renderer:"Playwright WebKit",rendererVersion:browser.version(),capturedAt:new Date().toISOString(),origin,sourceHashes,captures,checks:["Static result retained in all profiles","No horizontal overflow","Hanging-list indentation applied"],limitations:["Headless WebKit only; no native OS input or assistive-technology coverage.","Separate direct visual review required."]},null,2)+"\n");
 console.log("Static terminal checks and four captures passed.");
}finally{await browser.close();server.close();}
