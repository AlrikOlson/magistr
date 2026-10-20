// Capture the changed product imagery from the actual website build or deployed site.
import assert from "node:assert/strict";
import {webkit} from "../../../../apps/website/node_modules/playwright/index.mjs";
import {createServer} from "node:http";
import {readFile,writeFile,mkdir,readdir,stat} from "node:fs/promises";
import {createHash} from "node:crypto";
import {resolve,extname,relative,sep} from "node:path";
const repo=process.cwd(),dist=resolve(repo,"apps/website/dist"),out=resolve(repo,process.argv[2]);
assert(out.startsWith(resolve(repo,"docs/evidence/desktop-iterations")+sep));
await mkdir(out,{recursive:true});
assert(!(await readdir(out)).some(name=>name.endsWith(".png")),"Use a fresh iteration folder.");
const mime={".html":"text/html",".js":"text/javascript",".css":"text/css",".png":"image/png",".svg":"image/svg+xml",".json":"application/json"};
const server=createServer(async(req,res)=>{
 try {
  let file=resolve(dist,"."+decodeURIComponent(new URL(req.url,"http://localhost").pathname));
  assert(file.startsWith(dist+sep)||file===dist);
  if((await stat(file)).isDirectory())file=resolve(file,"index.html");
  res.writeHead(200,{"Content-Type":mime[extname(file)]??"application/octet-stream"});res.end(await readFile(file));
 }catch{res.writeHead(404);res.end("Not found");}
});
await new Promise(done=>server.listen(0,"127.0.0.1",done));
const origin=process.argv[3]??"http://127.0.0.1:"+server.address().port;
const browser=await webkit.launch({headless:true}),captures=[],checks=[];
const hash=bytes=>createHash("sha256").update(bytes).digest("hex");
try {
 for(const viewport of [{width:1440,height:1200},{width:390,height:844}]){
  for(const colorScheme of ["light","dark"]){
   const context=await browser.newContext({viewport,deviceScaleFactor:1,colorScheme,reducedMotion:"reduce"});
   await context.route("https://plausible.io/**",route=>route.fulfill({status:204}));
   const page=await context.newPage(),errors=[];
   page.on("pageerror",error=>errors.push(error.message));
   assert.equal((await page.goto(origin+"/")).status(),200);
   await page.evaluate(()=>document.fonts.ready);
   for(const image of await page.locator("img").all()){
    if(await image.isVisible())await image.scrollIntoViewIfNeeded();
    await page.waitForFunction(element=>element.complete&&element.naturalWidth>0,await image.elementHandle(),{timeout:10000});
    await image.evaluate(element=>element.decode());
   }
   assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
   const suffix=(viewport.width>800?"wide":"compact")+"-"+colorScheme;
   for(const [name,selector,state] of [
    ["home",".hero-capture","Current native app and its website caption"],
    ["codex",".agent-session:first-child","Fresh Codex session recovers verified work"],
    ["claude",".agent-session:last-child","Fresh Claude Code session recovers verified work"]
   ]){
    const element=page.locator(selector);
    await element.evaluate(element=>scrollTo({top:element.getBoundingClientRect().top+scrollY-90,behavior:"instant"}));
    const file=name+"-"+suffix+".png";
    const png=await element.screenshot({path:resolve(out,file),animations:"disabled"});
    captures.push({file,state,viewport,appearance:colorScheme,selector,capturedAt:new Date().toISOString(),
     width:png.readUInt32BE(16),height:png.readUInt32BE(20),sha256:hash(png),review:"pending"});
   }
   for(const a of await page.locator(".capture a:visible").all()){
    const href=await a.getAttribute("href"),response=await context.request.get(new URL(href,origin).href);
    assert.equal(response.status(),200);assert.match(response.headers()["content-type"],/image\/png/);
    const bytes=await response.body(),image=a.locator("img");
    assert.equal(bytes.readUInt32BE(16),Number(await image.getAttribute("width")));
    assert.equal(bytes.readUInt32BE(20),Number(await image.getAttribute("height")));
   }
   assert.deepEqual(errors,[]);
   checks.push({viewport,appearance:colorScheme,passed:true,imagesLoaded:true,fullSizeLinks:true,overflow:false});
   await context.close();
  }
 }
 const sourceHashes={};
 for(const file of ["apps/website/src/pages/index.astro","apps/website/src/styles/global.css",
  "apps/website/public/screenshots/captures.json","apps/website/tests/build-output.mjs",
  "apps/website/tests/browser.mjs",relative(repo,new URL(import.meta.url).pathname)]){
  sourceHashes[file]=hash(await readFile(resolve(repo,file)));
 }
 await writeFile(resolve(out,"captures.json"),JSON.stringify({origin,capturedAt:new Date().toISOString(),
  renderer:"Playwright WebKit",rendererVersion:browser.version(),platform:process.platform+"-"+process.arch,
  deviceScaleFactor:1,reducedMotion:true,sourceHashes,captures,checks,
  limitations:["Headless browser screenshots; no native input or assistive-technology claim.","Each image requires separate visual review."]},null,2)+"\n");
 console.log(JSON.stringify({passed:true,images:captures.length,out,origin}));
}finally{await browser.close();await new Promise(done=>server.close(done));}
