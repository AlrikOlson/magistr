import { test, expect, root, ui } from "./fixture.mjs";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { resolve, relative } from "node:path";
import { createHash } from "node:crypto";

test("workspace composition, appearances and narrow Details",async({app,browser})=>{
  const evidence=resolve(root,".magistr/desktop-layout");
  await mkdir(evidence,{recursive:true});
  const images={},checks={};
  const capture=async name=>{
    const bytes=await app.screenshot({path:resolve(evidence,name),animations:"disabled"});
    images[name]=createHash("sha256").update(bytes).digest("hex");
  };
  await capture("welcome-dark.png");
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Review the release plan",exact:true})).toBeVisible();
  await expect(app.getByLabel("Message the agent",{exact:true})).toHaveValue("Keep the project list visible, and let me return to the same paragraph.");
  checks["selected conversation and saved draft are visible"]=true;
  for(const theme of ["dark","light"]) {
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await expect(app.locator("html")).toHaveAttribute("data-theme",theme);
    if(theme==="light")await capture("settings-light.png");
    await app.getByRole("button",{name:"Done",exact:true}).click();
    await app.locator("#transcript").evaluate(node=>node.scrollTop=0);
    for(const name of ["Open project","New conversation","Saved conversations"]){
      const bounds=await app.getByRole("button",{name,exact:true}).boundingBox();
      expect(bounds.y).toBeGreaterThanOrEqual(0);
      expect(bounds.y+bounds.height).toBeLessThanOrEqual(820);
    }
    checks[theme+" navigation actions stay within the window"]=true;
    await capture("workspace-"+theme+".png");
  }
  await app.getByLabel("Message the agent",{exact:true}).fill("A narrow window keeps this unsent message.");
  await app.setViewportSize({width:800,height:680});
  await expect(app.getByRole("button",{name:"Work on next item",exact:true})).toBeHidden();
  const editor=app.getByLabel("Message the agent",{exact:true});
  await expect(editor).toBeVisible();
  expect((await editor.boundingBox()).width).toBeGreaterThanOrEqual(500);
  await capture("workspace-narrow.png");
  checks["narrow conversation retains a useful reading and editing width"]=true;
  await app.getByRole("button",{name:"Details",exact:true}).click();
  await expect(editor).toBeHidden();
  await expect(app.getByRole("button",{name:"Work on next item",exact:true})).toBeVisible();
  await expect(app.getByRole("heading",{name:"Conversation details",exact:true})).toBeFocused();
  await capture("details-narrow.png");
  await app.keyboard.press("Escape");
  await expect(editor).toHaveValue("A narrow window keeps this unsent message.");
  await expect(editor).toBeVisible();
  await expect(app.getByRole("button",{name:"Details",exact:true})).toBeFocused();
  checks["Details and Escape preserve the draft and restore focus"]=true;
  await app.getByLabel("Conversation actions",{exact:true}).click();
  await app.getByRole("button",{name:"Rename conversation",exact:true}).click();
  await capture("rename-light.png");
  await app.getByRole("button",{name:"Cancel",exact:true}).click();
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("dark");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.evaluate(()=>window.__fixture.publish("release",{work:{
    id:{project:"magistr",local:"review"},phase:{kind:"paused"},iteration:3,limit:10,requested:null
  }}));
  await app.getByRole("button",{name:"Work paused",exact:true}).click();
  await capture("work-paused-dark.png");
  checks["work summary opens exact controls and naming has its own dialog"]=true;
  const sources={};
  for(const name of (await readdir(ui)).sort()){
    const file=resolve(ui,name);
    sources[relative(root,file)]=createHash("sha256").update(await readFile(file)).digest("hex");
  }
  for(const name of ["playwright.config.mjs","tests/browser/fixture.mjs","tests/browser/workspace.spec.mjs"]) {
    const file=resolve(root,"crates/magistr-desktop",name);
    sources[relative(root,file)]=createHash("sha256").update(await readFile(file)).digest("hex");
  }
  await writeFile(resolve(evidence,"receipt.json"),JSON.stringify({
    renderer:"headless WebKit",version:browser.version(),captured_at:new Date().toISOString(),
    scope:"Production desktop HTML, CSS and JavaScript with isolated observation fixtures; no native OS input or Rust owner integration.",
    checks,source_hashes:sources,images,
  },null,2)+"\n");
});
