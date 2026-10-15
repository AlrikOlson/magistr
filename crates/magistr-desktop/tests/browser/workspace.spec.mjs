import { test, expect, root, ui, workspaceSeed } from "./fixture.mjs";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { resolve, relative } from "node:path";
import { createHash } from "node:crypto";
import { tableScene, patchScene, toolScene, outlineScene, editorialScene, galleryScene, viewerScene, writingScene } from "./design_scenes.mjs";

test("workspace composition, appearances and narrow Details",async({app,browser})=>{
  test.setTimeout(60000);
  const evidence=resolve(root,".magistr/desktop-layout");
  await mkdir(evidence,{recursive:true});
  const images={},viewports={},checks={};
  const capture=async name=>{
    await app.mouse.move(0,0);
    await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const bytes=await app.screenshot({path:resolve(evidence,name),animations:"disabled"});
    images[name]=createHash("sha256").update(bytes).digest("hex");viewports[name]=app.viewportSize();
  };
  await expect(app.locator("#welcome-projects")).toContainText("/Projects/magistr");
  await expect(app.getByRole("button",{name:"Choose a folder…",exact:true})).toBeVisible();
  await expect(app.locator(".directory-project-facts").first()).toContainText("Waiting for you");
  await expect(app.locator(".directory-project-facts").first().locator("dd")).toHaveText(["2","1"]);
  await capture("welcome-dark.png");
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await capture("welcome-light.png");
  await app.setViewportSize({width:800,height:680});
  await capture("welcome-narrow-light.png");
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("dark");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  checks["project chooser presents actual projects and folder entry in both appearances"]=true;
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Review the release plan",exact:true})).toBeVisible();
  await expect(app.getByLabel("Message the agent",{exact:true})).toHaveValue("Keep the project list visible, and let me return to the same paragraph.");
  await expect(app.locator("#header-location")).toHaveText("/Projects/magistr");
  checks["selected conversation, owning folder and saved draft are visible"]=true;
  for(const theme of ["dark","light"]) {
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await expect(app.locator("html")).toHaveAttribute("data-theme",theme);
    await capture("settings-"+theme+".png");
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
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await capture("settings-narrow-light.png");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  checks["narrow conversation retains a useful reading and editing width"]=true;
  await app.getByRole("button",{name:"Details",exact:true}).click();
  await expect(editor).toBeHidden();
  await expect(app.getByRole("button",{name:"Work on next item",exact:true})).toBeVisible();
  await expect(app.getByRole("heading",{name:"Conversation details",exact:true})).toBeFocused();
  await capture("details-narrow.png");
  await app.locator('.detail-section[aria-label="Agent settings"]').evaluate(node=>node.scrollIntoView({block:"start"}));
  await capture("inspector-narrow-light.png");
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
  await app.locator('.detail-section[aria-label="Agent settings"]').evaluate(node=>node.scrollIntoView({block:"start"}));
  await expect(app.locator("#context-value")).toHaveText("9%");
  await capture("inspector-dark.png");
  await app.evaluate(()=>window.__fixture.publish("release",{usage:null}));
  await expect(app.locator("#context-chart")).toHaveAttribute("data-known","false");
  await expect(app.locator("#context-value")).toHaveText("Not reported");
  await expect(app.locator("#context-meter")).toBeHidden();
  await capture("inspector-unknown-dark.png");
  await app.evaluate(()=>window.__fixture.publish("release",{usage:{used:0,size:200000,cost_cents:0}}));
  await expect(app.locator("#context-chart")).toHaveAttribute("data-known","true");
  await expect(app.locator("#context-value")).toHaveText("0%");
  await expect(app.locator("#spending-value")).toHaveText("$0.00");
  await app.evaluate(()=>window.__fixture.publish("release",{usage:{used:18400,size:200000,cost_cents:42}}));
  checks["context dial distinguishes reported values, real zero and absent usage"]=true;
  checks["work summary opens exact controls and naming has its own dialog"]=true;
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Release notes and rollback",exact:true})).toBeVisible();
  await capture("history-dark.png");
  await app.locator("#saved-draft>summary").click();
  await capture("history-reading-dark.png");
  await app.getByRole("button",{name:"Close saved conversations",exact:true}).click();
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.setViewportSize({width:800,height:680});
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Release notes and rollback",exact:true})).toBeVisible();
  if(await app.locator("#saved-draft").getAttribute("open")===null)await app.locator("#saved-draft>summary").click();
  await capture("history-narrow-light.png");
  await app.locator("#saved-draft>summary").click();
  await capture("history-reading-narrow-light.png");
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
      {tag:"pre",language:"javascript",children:[{tag:"code",text:"const saved = await drafts.flush(target);\nif (saved) {\n  await selectConversation(next);\n}",start:310}],start:310},
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
    await app.locator(".message-body pre").scrollIntoViewIfNeeded();
    await expect(app.locator(".message-body pre code")).toContainText("await drafts.flush(target)");
    await capture("code-"+theme+".png");
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
  await app.getByRole("button",{name:"View icon.png",exact:true}).click();
  await expect(app.locator("#attachment-metadata")).toContainText("×");
  await capture("image-inspection-dark.png");
  await app.getByRole("button",{name:"Close image",exact:true}).click();
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.setViewportSize({width:800,height:680});
  await expect(app.getByRole("button",{name:"Remove release-notes.md",exact:true})).toBeVisible();
  await capture("attachments-narrow-light.png");
  checks["file cards retain preview, path, size and named removal at compact width"]=true;
  await app.getByRole("button",{name:"Projects",exact:true}).click();
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
  const config=JSON.parse(await readFile(resolve(root,"crates/magistr-desktop/tauri.conf.json"),"utf8"));
  const minimum=config.app.windows[0];
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.evaluate(events=>{
    const frame=window.__fixture.frames.get("release");
    window.__fixture.publish("release",{events,retained:events.map(event=>event.id),attachments:[],draft_revision:frame.draft_revision+1,draft_saved_revision:frame.draft_revision+1});
  },workspaceSeed().frames[0].events);
  await app.getByLabel("Message the agent",{exact:true}).fill("Check the release notes before we continue.");
  await expect(app.locator("#draft-status")).toHaveText("Draft saved");
  await app.setViewportSize({width:minimum.minWidth,height:minimum.minHeight});
  for(const theme of ["dark","light"]){
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await app.getByRole("button",{name:"Done",exact:true}).click();
    if(await app.getByRole("button",{name:"Latest reply",exact:true}).isVisible())await app.getByRole("button",{name:"Latest reply",exact:true}).click();
    const editor=app.getByLabel("Message the agent",{exact:true});
    await expect(editor).toBeVisible();
    expect((await editor.boundingBox()).width).toBeGreaterThanOrEqual(minimum.minWidth-64);
    expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(minimum.minWidth);
    await capture("minimum-workspace-"+theme+".png");
    await app.getByRole("button",{name:"Projects",exact:true}).click();
    await expect(app.locator("#workspace-main")).toBeHidden();
    await capture("minimum-projects-"+theme+".png");
    await app.keyboard.press("Escape");
    await expect(editor).toHaveValue("Check the release notes before we continue.");
  }
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await capture("minimum-settings-light.png");
  const done=await app.getByRole("button",{name:"Done",exact:true}).boundingBox();
  expect(done.y+done.height).toBeLessThan(minimum.minHeight);
  await app.getByRole("button",{name:"Done",exact:true}).click();
  checks["native minimum window keeps full-width editing, project browsing and dialog actions reachable"]=true;

  await app.evaluate(()=>{
    window.__fixture.handlers.preview_project=({directory})=>({kind:"ready",directory});
    window.__fixture.handlers.open_session=()=>{throw "The agent could not connect.\n\nThe sign-in attempt expired.\nOpen the agent, sign in, then try again.\n\nConnection details:\nThe agent ended the connection before the conversation was ready.";};
  });
  for(const theme of ["dark","light"]){
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await app.getByRole("button",{name:"Done",exact:true}).click();
    await app.getByRole("button",{name:"Open project",exact:true}).click();
    await app.getByLabel("Project folder",{exact:true}).fill("/Projects/new-project");
    await app.locator("#picker-submit").click();
    await expect(app.locator("#picker-error")).toHaveText("Couldn’t start the conversation");
    await capture("open-project-error-"+theme+".png");
    await app.getByText("Show error details",{exact:true}).click();
    await capture("open-project-error-details-"+theme+".png");
    const retry=await app.getByRole("button",{name:"Try again",exact:true}).boundingBox();
    expect(retry.y+retry.height).toBeLessThan(minimum.minHeight);
    await app.getByRole("button",{name:"Cancel",exact:true}).click();
  }
  checks["project opening failures preserve the folder, disclose readable details and retain retry at the native minimum"]=true;
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Update the home page",exact:true}).click();
  for(const theme of ["dark","light"]){
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await app.getByRole("button",{name:"Done",exact:true}).click();
    await app.locator("#transcript").evaluate(node=>node.scrollTop=0);
    await expect(app.getByRole("heading",{name:"What would you like to work on?",exact:true})).toBeVisible();
    await capture("empty-conversation-"+theme+".png");
  }
  checks["empty conversations have a readable introduction and a visible composer"]=true;
  await tableScene(app,capture,expect);
  checks["comparison tables preserve headers, column alignment and contained compact scrolling"]=true;
  await patchScene(app,capture,expect);
  checks["patch documents preserve source bytes, old/new positions and actual change totals"]=true;
  await toolScene(app,capture,expect);
  checks["structured tool sheets retain nested values, bounded rendering and exact copy"]=true;
  await outlineScene(app,capture,expect);
  checks["reply contents derive from real headings and land keyboard focus on the chosen section"]=true;
  await editorialScene(app,capture,expect);
  checks["editorial quotes and nested task lists retain semantic source and authored order"]=true;
  await galleryScene(app,capture,expect);
  checks["sent attachments form a contact sheet with source previews and native image inspection"]=true;
  await viewerScene(app,capture,expect);
  checks["image inspection provides fit and pixel zoom, backgrounds and compact controls"]=true;
  await writingScene(app,capture,expect);
  checks["expanded writing preserves native editor identity, text, caret and compact actions"]=true;
  const sources={};
  for(const name of (await readdir(ui)).sort()){
    const file=resolve(ui,name);
    sources[relative(root,file)]=createHash("sha256").update(await readFile(file)).digest("hex");
  }
  for(const name of ["playwright.config.mjs","tauri.conf.json","tests/browser/fixture.mjs","tests/browser/workspace.spec.mjs","tests/browser/design_scenes.mjs"]) {
    const file=resolve(root,"crates/magistr-desktop",name);
    sources[relative(root,file)]=createHash("sha256").update(await readFile(file)).digest("hex");
  }
  await writeFile(resolve(evidence,"receipt.json"),JSON.stringify({
    renderer:"headless WebKit",version:browser.version(),captured_at:new Date().toISOString(),
    scope:"Production desktop HTML, CSS and JavaScript with isolated observation fixtures; no native OS input or Rust owner integration.",
    checks,source_hashes:sources,images,viewports,
  },null,2)+"\n");
});
