// Real PTY bytes rendered by xterm in headless WebKit. Commands arrive on stdin.
import {webkit} from "../../../../apps/website/node_modules/playwright/index.mjs";
import {createInterface} from "node:readline";
import {readFile,writeFile,stat} from "node:fs/promises";
import {createHash} from "node:crypto";
import {resolve} from "node:path";
const root=process.cwd(),directory=resolve(root,"docs/evidence/desktop-iterations/2026-10-19-release-product-captures");
const browser=await webkit.launch({headless:true}),pages=new Map();
for(const [host,port] of [["codex",8767],["claude",8768]]){
  const page=await browser.newPage({viewport:{width:1320,height:1000},deviceScaleFactor:2,reducedMotion:"reduce"});
  await page.goto("http://127.0.0.1:"+port);
  pages.set(host,page);
}
console.log(JSON.stringify({ready:true,renderer:"WebKit",version:browser.version()}));
for await(const line of createInterface({input:process.stdin})){
  try{
    const c=JSON.parse(line),page=pages.get(c.host);
    if(c.action==="state"){console.log(JSON.stringify({host:c.host,text:await page.evaluate(()=>screenText())}));}
    if(c.action==="send"){await page.evaluate(text=>send(text),c.text);console.log(JSON.stringify({sent:c.host}));}
    if(c.action==="capture"){
      const dest=resolve(directory,c.name+".png");
      try{await stat(dest);throw Error("Capture already exists");}catch(e){if(e.code!=="ENOENT")throw e;}
      await page.evaluate(()=>term.scrollToBottom());
      const image=await page.locator("#terminal").screenshot({path:dest,animations:"disabled"});
      const text=await page.evaluate(()=>screenText());
      await writeFile(resolve(directory,c.name+".txt"),text+"\n",{flag:"wx"});
      const receipt={file:c.name+".png",capturedAt:new Date().toISOString(),client:c.host,renderer:"xterm 6.0.0 in headless WebKit "+browser.version(),
        viewport:page.viewportSize(),deviceScaleFactor:2,terminal:await page.evaluate(()=>({columns:term.cols,rows:term.rows})),
        imageSha256:createHash("sha256").update(image).digest("hex"),method:"Unmodified bytes from the real interactive client and live Magistr MCP; no simulated replies."};
      await writeFile(resolve(directory,c.name+".json"),JSON.stringify(receipt,null,2)+"\n",{flag:"wx"});
      console.log(JSON.stringify(receipt));
    }
    if(c.action==="quit"){await browser.close();break;}
  }catch(error){console.log(JSON.stringify({error:String(error)}));}
}
