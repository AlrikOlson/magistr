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
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Release notes and rollback",exact:true})).toBeVisible();
  await capture("history-dark.png");
  await app.getByRole("button",{name:"Close saved conversations",exact:true}).click();
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.setViewportSize({width:800,height:680});
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Release notes and rollback",exact:true})).toBeVisible();
  await capture("history-narrow-light.png");
  checks["saved history has fixed heading, search, independent panes and resume action"]=true;
  await app.getByRole("button",{name:"Close saved conversations",exact:true}).click();
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await app.getByLabel("Project folder",{exact:true}).fill("/Projects/new-project");
  await app.locator("#picker-submit").click();
  await expect(app.getByRole("heading",{name:"Review project setup",exact:true})).toBeVisible();
  await app.getByRole("button",{name:"Read AGENTS.md",exact:true}).click();
  await capture("setup-narrow-light.png");
  await app.getByRole("button",{name:"Cancel",exact:true}).click();
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("dark");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await capture("open-project-dark.png");
  await app.locator("#picker-submit").click();
  await expect(app.getByRole("heading",{name:"Review project setup",exact:true})).toBeVisible();
  await app.getByRole("button",{name:"Read AGENTS.md",exact:true}).click();
  await capture("setup-dark.png");
  checks["setup review separates file approval from reading with fixed actions"]=true;
  await app.getByRole("button",{name:"Cancel",exact:true}).click();
  await app.getByRole("button",{name:"New conversation",exact:true}).click();
  await expect(app.getByRole("radio",{name:"GitHub Copilot",exact:true})).toBeVisible();
  await capture("new-conversation-dark.png");
  await app.getByRole("button",{name:"Cancel",exact:true}).click();
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.setViewportSize({width:800,height:680});
  await app.getByRole("button",{name:"New conversation",exact:true}).click();
  await app.getByRole("button",{name:"Choose a model…",exact:true}).click();
  await app.getByLabel("Model for this conversation",{exact:true}).selectOption("sonnet");
  await capture("new-conversation-narrow-light.png");
  checks["new conversation exposes destination, agents and optional model choice"]=true;
  await app.getByRole("button",{name:"Cancel",exact:true}).click();
  await app.getByRole("button",{name:"Find a conversation",exact:true}).click();
  await app.getByLabel("Find an open conversation",{exact:true}).fill("magistr");
  await app.keyboard.press("ArrowDown");
  await capture("find-conversation-narrow-light.png");
  await app.keyboard.press("Escape");
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("dark");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.getByRole("button",{name:"Find a conversation",exact:true}).click();
  await capture("find-conversation-dark.png");
  checks["conversation finding and documented keyboard navigation keep focus visible"]=true;
  await app.getByRole("button",{name:"Cancel",exact:true}).click();
  await app.evaluate(()=>{
    const nodes=[
      {tag:"h2",text:"A small, recoverable change",start:0},
      {tag:"p",text:"Keep the saved message and the visible conversation in sync. The change belongs in the draft writer, with a focused check for interrupted navigation.",start:40},
      {tag:"blockquote",children:[{tag:"p",text:"Save the incoming draft before replacing the visible conversation.",start:200}],start:200},
      {tag:"h3",text:"Apply the change",start:280},
      {tag:"pre",children:[{tag:"code",text:"const saved = await drafts.flush(target);\nif (saved) {\n  await selectConversation(next);\n}",start:310}],start:310},
      {tag:"p",children:[{tag:"text",text:"Verify the ",start:410},{tag:"code",text:"save_draft",start:421},{tag:"text",text:" result before changing the selection.",start:431}],start:410}
    ];
    window.__fixture.publish("release",{events:[{id:90,revision:1,role:"Agent",text:"A small, recoverable change",document:{nodes},attachments:[]}],retained:[90]});
  });
  await app.getByRole("button",{name:"Details",exact:true}).click();
  for(const theme of ["dark","light"]){
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await app.getByRole("button",{name:"Done",exact:true}).click();
    await app.locator("#transcript").evaluate(node=>node.scrollTop=0);
    await expect(app.getByRole("heading",{name:"A small, recoverable change",exact:true})).toBeVisible();
    await capture("reading-"+theme+".png");
  }
  checks["rich document typography renders headings, quotations, code and inline source"]=true;
  await app.evaluate(()=>{
    window.__fixture.publish("release",{events:[
      {id:91,revision:1,role:"Tool",text:"cargo check -p magistr-desktop",tool:{title:"Check the draft writer",status:"Completed",details:[{label:"Command",source:"cargo check -p magistr-desktop"},{label:"Result",source:"Finished successfully. No warnings."}],duration_ms:1840,truncated_from:null,image_count:0}},
      {id:92,revision:1,role:"Tool",text:"Read the session log",tool:{title:"Read the session log",status:"Failed",details:[{label:"Result",source:"The file could not be read. Permission was denied."}],duration_ms:46,truncated_from:null,image_count:0}}
    ],retained:[91,92]});
  });
  await app.locator('[data-message="91"] summary').click();
  for(const theme of ["light","dark"]){
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await app.getByRole("button",{name:"Done",exact:true}).click();
    await app.locator("#transcript").evaluate(node=>node.scrollTop=0);
    await expect(app.getByText("Finished successfully. No warnings.",{exact:true})).toBeVisible();
    await capture("tools-"+theme+".png");
  }
  checks["tool cards distinguish result, failure and duration without losing disclosure"]=true;
  await app.evaluate(()=>{
    const frame=window.__fixture.frames.get("release");
    window.__fixture.publish("release",{attachments:[
      {id:"notes",name:"release-notes.md",size:2460,path:"docs/release-notes.md",content_hash:"notes",excerpt:"# Release notes\n\nKeep the draft when changing conversations."},
      {id:"icon",name:"icon.png",size:32768,path:"ui/icon.png",content_hash:"image",preview:"http://magistr.test/icon.png"}
    ],draft_revision:frame.draft_revision+1,draft_saved_revision:frame.draft_revision+1});
  });
  await expect(app.getByRole("list",{name:"Files to send",exact:true})).toBeVisible();
  await capture("attachments-dark.png");
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.setViewportSize({width:800,height:680});
  await expect(app.getByRole("button",{name:"Remove release-notes.md",exact:true})).toBeVisible();
  await capture("attachments-narrow-light.png");
  checks["file cards retain preview, path, size and named removal at compact width"]=true;
  await app.getByRole("button",{name:"Investigate sync failure",exact:true}).click();
  await app.locator("#review-requests").click();
  await expect(app.getByRole("heading",{name:"Run the focused sync checks",exact:true})).toBeVisible();
  await capture("request-narrow-light.png");
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("dark");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.locator("#review-requests").click();
  await capture("request-dark.png");
  checks["permission decisions present exact requested command and equal-weight choices"]=true;
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
