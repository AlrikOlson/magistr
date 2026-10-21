import assert from "node:assert/strict";
import {webkit} from "playwright";
import {mkdir,writeFile,readFile,readdir} from "node:fs/promises";
import {createHash} from "node:crypto";
import {resolve} from "node:path";
const args=process.argv,at=args.indexOf("--evidence-dir");
const evidence=at<0?null:resolve("../..",args[at+1]);
if(evidence){assert(evidence.includes("/docs/evidence/desktop-iterations/"));await mkdir(evidence,{recursive:true});assert(!(await readdir(evidence)).some(x=>x.endsWith(".png")),"Use a fresh capture folder");}
const base=args.includes("--base-url")?args[args.indexOf("--base-url")+1]:"http://127.0.0.1:4322";
const browser=await webkit.launch(),captures=[],errors=[];
const sourcePaths=["examples/bevy-tic-tac-toe/Cargo.toml","examples/bevy-tic-tac-toe/Cargo.lock","examples/bevy-tic-tac-toe/src/lib.rs","examples/bevy-tic-tac-toe/src/main.rs","examples/bevy-tic-tac-toe/web/index.html","apps/website/tests/game-browser.mjs"];
const sourceHashes=Object.fromEntries(await Promise.all(sourcePaths.map(async p=>[p,createHash("sha256").update(await readFile(resolve("../..",p))).digest("hex")])));
async function capture(page,name,state){if(!evidence)return;await page.screenshot({path:resolve(evidence,name+".png")});captures.push({file:name+".png",viewport:page.viewportSize(),appearance:"light",state});}
let status="failed";
try{
 for(const [profile,width]of [["desktop",720],["compact",320]]){
  const page=await browser.newPage({viewport:{width,height:700},deviceScaleFactor:1,reducedMotion:"reduce"});
  page.on("pageerror",e=>errors.push(String(e)));
  await page.goto(base);const state=page.locator("#game-status[data-cells]");
  await state.waitFor({state:"attached",timeout:45000});
  assert.equal(await state.getAttribute("data-cells"),".........");
  await capture(page,profile+"-start","Human is X; empty board and zero scores");
  await page.locator("#game").focus();
  // R cannot discard an unfinished game.
  await page.keyboard.press("r");await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));assert.equal(await state.getAttribute("data-human"),"X");
  // Play the first legal human square on each turn. The real Rust opponent replies.
  const pointer=async(x,y)=>{await page.mouse.move(x,y);await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));await page.mouse.down();await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));await page.mouse.up();};
  let move=0;
  while(await state.textContent()==="Your turn"){
   const before=await state.getAttribute("data-cells"),cell=before.indexOf(".");
   assert(cell>=0);if(move++===0)await pointer(width/2-90,220);else await page.keyboard.press(String(cell+1));
   await page.waitForFunction(old=>document.getElementById("game-status").getAttribute("data-cells")!==old,before);
   await page.waitForFunction(()=>document.getElementById("game-status").textContent!=="Computer's turn");
  }
  const score=await state.getAttribute("data-score"),ending=await state.getAttribute("data-cells");
  assert.equal(score.split(",").map(Number).reduce((a,b)=>a+b),1);
  await page.keyboard.press("9");assert.equal(await state.getAttribute("data-cells"),ending);
  assert.equal(await state.getAttribute("data-score"),score);
  await capture(page,profile+"-result","Completed round counted once; further input cannot change it");
  if(profile==="desktop")await pointer(width/2,500);else await page.keyboard.press("r");
  await page.waitForFunction(()=>document.getElementById("game-status").getAttribute("data-human")==="O");
  await page.waitForFunction(()=>document.getElementById("game-status").textContent==="Your turn");
  assert.equal(await state.getAttribute("data-cells"),"....X....");
  assert.equal(await state.getAttribute("data-score"),score);
  await capture(page,profile+"-rematch","Human is O; computer opens in the center; previous score retained");
  await page.close();
 }
 assert.deepEqual(errors,[]);status="passed";
 console.log("Bevy 0.20.0: real computer replies, terminal input protection, one score per round and alternating rematches passed at 720px and 320px.");
}finally{
 if(evidence)await writeFile(resolve(evidence,"browser-checks.json"),JSON.stringify({capturedAt:new Date().toISOString(),status,renderer:"Playwright WebKit",rendererVersion:browser.version(),sourceHashes,captures,errors,limitations:["Headless browser; native OS input and assistive technology not tested.","The game has one fixed paper appearance."]},null,2)+"\n");
 await browser.close();
}
