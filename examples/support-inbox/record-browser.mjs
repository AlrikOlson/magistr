import assert from "node:assert/strict";
import {spawn} from "node:child_process";
import {createHash} from "node:crypto";
import {readFile,writeFile,mkdir,mkdtemp,rm,access} from "node:fs/promises";
import {tmpdir} from "node:os";
import {resolve} from "node:path";
import {webkit} from "../../apps/website/node_modules/playwright/index.mjs";

const root=resolve("examples/support-inbox");
const directory=resolve(process.argv[2] || "docs/evidence/desktop-iterations/2026-10-20-inbox-build");
await mkdir(directory,{recursive:true});
const sources={};
for(const file of ["README.md","store.py","server.py","static/index.html","static/style.css","static/app.js","tests/test_store.py","tests/test_api.py","record-browser.mjs"])
 sources[file]=createHash("sha256").update(await readFile(resolve(root,file))).digest("hex");
const browser=await webkit.launch({headless:true});
const report={capturedAt:new Date().toISOString(),renderer:"Headless WebKit "+browser.version(),sources,captures:[],checks:[],video:null};
const snap=async(page,name,appearance,viewport)=>{
 const file=name+".png",path=resolve(directory,file);
 try {await access(path);throw new Error("Capture already exists: "+file);} catch(error){if(error.code!=="ENOENT")throw error;}
 await page.screenshot({path,fullPage:true});
 report.captures.push({file,appearance,viewport,capturedAt:new Date().toISOString(),sha256:createHash("sha256").update(await readFile(path)).digest("hex"),review:"pending"});
 await writeFile(resolve(directory,"captures.json"),JSON.stringify(report,null,2)+"\n");
};
try {
 for(const width of [1280,390])for(const appearance of ["light","dark"]){
  const temporary=await mkdtemp(resolve(tmpdir(),"magistr-inbox-"));
  const process=spawn("python3",[resolve(root,"server.py"),"--db",resolve(temporary,"inbox.sqlite"),"--port","0"],{stdio:["ignore","pipe","pipe"]});
  const origin=await new Promise((done,reject)=>{
   let output="";
   const timer=setTimeout(()=>reject(new Error("Inbox did not start")),10000);
   process.stdout.on("data",bytes=>{output+=bytes;const match=output.match(/http:\/\/127\.0\.0\.1:\d+/);if(match){clearTimeout(timer);done(match[0]);}});
   process.once("exit",code=>{clearTimeout(timer);reject(new Error("Inbox exited "+code));});
  });
  const viewport={width,height:900};
  const filming=width===1280 && appearance==="light";
  const context=await browser.newContext({viewport,colorScheme:appearance,reducedMotion:"reduce",...(filming?{recordVideo:{dir:resolve(directory,"video"),size:viewport}}:{})});
  const page=await context.newPage();
  const errors=[];page.on("pageerror",error=>errors.push(error.message));
  const pause=()=>filming?page.waitForTimeout(1200):Promise.resolve();
  const suffix=(width===1280?"wide":"compact")+"-"+appearance;
  const delivery={external_id:"sample-reply",thread_id:"brakes",sender:"Nora Ellis <nora@example.test>",subject:"Re: Can you look at my brakes?",body:"Friday at 9 works. Thank you! I can leave it with you until lunchtime."};
  try{
   await page.goto(origin);
   await page.getByText("Your inbox is empty.",{exact:false}).waitFor();
   await snap(page,"inbox-empty-"+suffix,appearance,viewport);await pause();
   await page.getByRole("button",{name:"Load sample mail"}).click();
   await page.getByText("3 sample conversations received.",{exact:true}).waitFor();await pause();
   await page.getByRole("button",{name:"Load sample mail"}).click();
   await page.getByText("All three messages were already received. No duplicates added.",{exact:true}).waitFor();
   assert.equal(await page.locator(".conversation").count(),3);
   await page.getByRole("searchbox").fill("Friday");
   await page.waitForFunction(()=>document.querySelectorAll(".conversation").length===1);await pause();
   await page.locator(".conversation").click();
   await page.locator("#conversation-assignee").selectOption("Maya");
   await page.getByText("Assigned to Maya.",{exact:true}).waitFor();await pause();
   assert.equal(await page.locator("#conversation-assignee").inputValue(),"Maya");
   await snap(page,"inbox-assigned-"+suffix,appearance,viewport);
   await page.getByRole("button",{name:"Mark resolved"}).click();
   await page.getByText("Conversation resolved.",{exact:true}).waitFor();await pause();
   assert.equal(await page.locator("#conversation-status").innerText(),"Reopen conversation");
   const response=await context.request.post(origin+"/api/mail",{data:delivery});
   assert.equal(response.status(),201);
   await page.getByRole("button",{name:"Refresh inbox"}).click();
   await page.waitForFunction(()=>document.querySelectorAll(".message").length===2);
   assert.equal(await page.locator(".state").innerText(),"Open");await pause();
   const duplicate=await context.request.post(origin+"/api/mail",{data:delivery});
   assert.equal(duplicate.status(),200);
   const current=(await duplicate.json()).conversation;
   await context.request.patch(origin+"/api/conversations/"+current.id,{data:{version:current.version,assignee:"Leo"}});
   await page.locator("#conversation-assignee").selectOption("Sam");
   await page.getByText("Someone else updated this conversation. Latest changes loaded; choose again.",{exact:true}).waitFor();await pause();
   assert.equal(await page.locator("#conversation-assignee").inputValue(),"Leo");
   if(filming)await snap(page,"inbox-conflict-"+suffix,appearance,viewport);
   await page.getByRole("button",{name:"Refresh inbox"}).click();
   await page.getByRole("searchbox").fill("");
   await page.waitForFunction(()=>document.querySelectorAll(".conversation").length===3);await pause();
   await snap(page,"inbox-finished-"+suffix,appearance,viewport);
   await page.reload();
   await page.waitForFunction(()=>document.querySelectorAll(".conversation").length===3);
   await page.getByRole("searchbox").fill("no-such-conversation");
   await page.getByText("No conversations match. Try a different search or filter.",{exact:true}).waitFor();
   if(filming)await snap(page,"inbox-search-empty-"+suffix,appearance,viewport);
   assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),"No horizontal overflow");
   assert.deepEqual(errors,[]);
   report.checks.push({viewport,appearance,passed:true,scenarios:["empty inbox","real mail delivery","duplicate retry","search","assignment","resolution","reply reopens thread","concurrent update conflict","reload persistence","empty search"]});
  }finally{
   const video=page.video();
   await context.close();
   if(video)report.video={file:"inbox-workflow.webm",sha256:null,viewport,description:"Unmodified browser recording of the real local API workflow; sample customer data; no audio."};
   if(video){await video.saveAs(resolve(directory,"inbox-workflow.webm"));report.video.sha256=createHash("sha256").update(await readFile(resolve(directory,"inbox-workflow.webm"))).digest("hex");}
   process.kill("SIGTERM");await new Promise(done=>process.once("exit",done));
   await rm(temporary,{recursive:true,force:true});
  }
 }
 report.passed=true;
}finally{
 await browser.close();
 await writeFile(resolve(directory,"captures.json"),JSON.stringify(report,null,2)+"\n");
}
console.log(JSON.stringify({passed:report.passed,profiles:report.checks.length,screenshots:report.captures.length,video:report.video,evidence:directory}));
