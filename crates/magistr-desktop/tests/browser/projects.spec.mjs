import { test, expect } from "./fixture.mjs";

async function review(app) {
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await app.getByLabel("Project folder",{exact:true}).fill("/Projects/new-project");
  await app.locator("#picker-submit").click();
  await expect(app.getByRole("heading",{name:"Review project setup",exact:true})).toBeVisible();
}
test("file reading is separate from approval and pending or partial setup stays recoverable",async({app})=>{
  await app.setViewportSize({width:800,height:560});
  await review(app);
  await expect(app.locator("#setup-count")).toHaveText("12 of 12 changes selected");
  await app.getByRole("button",{name:"Read AGENTS.md",exact:true}).click();
  await expect(app.getByRole("region",{name:"Selected file change"})).toContainText("Only the Magistr instructions will change");
  expect(await app.locator("#setup-change .setup-source").textContent()).toBe("@@ -1 +1,3 @@\n # Project notes\n+\n+Use focused checks for this project.\n");
  await expect(app.getByLabel("2 lines added, 0 lines removed",{exact:true})).toBeVisible();
  await expect(app.getByLabel("Include AGENTS.md",{exact:true})).toBeChecked();
  await app.getByLabel("Include AGENTS.md",{exact:true}).uncheck();
  await app.getByRole("button",{name:"Read .claude/settings.json",exact:true}).click();
  await expect(app.getByRole("region",{name:"Selected file change"})).toContainText("current contents will be kept");
  await expect(app.locator("#setup-count")).toHaveText("11 of 12 changes selected");
  for(const id of ["picker-submit","picker-cancel"]){const bounds=await app.locator("#"+id).boundingBox();expect(bounds.y+bounds.height).toBeLessThan(560);}
  await app.evaluate(()=>window.__fixture.handlers.apply_setup=()=>new Promise(resolve=>window.finishSetup=resolve));
  await app.getByRole("button",{name:"Apply selected files",exact:true}).click();
  await expect(app.getByRole("button",{name:"Cancel",exact:true})).toBeDisabled();
  await app.keyboard.press("Escape");await expect(app.locator("#project-picker")).toBeVisible();
  const call=await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="apply_setup"));
  expect(call).toHaveLength(1);expect(call[0].args.approved).not.toContain(1);expect(call[0].args.approved).not.toContain(2);
  await app.evaluate(()=>window.finishSetup({complete:false,directory:"/Projects/new-project",setup_record:"Could not save the setup record.",files:[{path:"AGENTS.md",outcome:"skipped",detail:null},{path:".magistr/project.json",outcome:"failed",detail:"The folder is read-only."}]}));
  await expect(app.getByRole("heading",{name:"Setup needs attention",exact:true})).toBeVisible();
  await expect(app.locator("#setup-outcomes")).toContainText("The folder is read-only.");
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="open_session"))).toHaveLength(0);
  await app.getByRole("button",{name:"Review again",exact:true}).click();
  await expect(app.getByRole("button",{name:"Apply selected files",exact:true})).toBeVisible();
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="preview_project").at(-1).args.review)).toBe(true);
  await app.getByRole("button",{name:"Change folder",exact:true}).click();
  await expect(app.getByLabel("Project folder",{exact:true})).toBeFocused();
  await expect(app.locator("#setup-preview")).toBeHidden();
});
test("cancelled reads cannot reopen a chooser and folder picker errors remain visible",async({app})=>{
  await app.evaluate(()=>window.__fixture.handlers.preview_project=()=>new Promise(resolve=>window.finishPreview=resolve));
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await app.getByLabel("Project folder",{exact:true}).fill("/Projects/old");
  await app.locator("#picker-submit").click();
  await expect(app.locator("#picker-status")).toHaveText("Checking the folder…");
  await app.keyboard.press("Escape");
  await expect(app.locator("#project-picker")).toBeHidden();
  await app.evaluate(()=>window.finishPreview({kind:"setup",review:90,directory:"/Projects/old",setup_directory:"/Projects/old",files:[]}));
  await expect.poll(()=>app.evaluate(()=>window.__fixture.calls.some(c=>c.command==="discard_setup"&&c.args.review===90))).toBe(true);
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await app.evaluate(()=>window.__fixture.handlers.pick_folder=()=>{throw Error("The folder picker is unavailable. Enter a path instead.");});
  await app.getByRole("button",{name:"Browse…",exact:true}).click();
  await expect(app.locator("#picker-error-message")).toContainText("Enter a path instead.");
  await expect(app.getByLabel("Project folder",{exact:true})).toBeEnabled();
});
test("a ready folder returns to its resident conversation without starting another agent",async({app})=>{
  await app.evaluate(()=>window.__fixture.handlers.preview_project=({directory})=>({kind:"ready",directory}));
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await app.getByLabel("Project folder",{exact:true}).fill("/Projects/magistr");
  await app.locator("#picker-submit").click();
  await expect(app.locator("#project-picker")).toBeHidden();
  await expect(app.getByRole("heading",{name:"Review the release plan",exact:true})).toBeVisible();
  await expect(app.getByLabel("Message the agent",{exact:true})).toBeFocused();
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="open_session"))).toHaveLength(0);
});
test("startup errors preserve the folder, disclose exact details and retry without duplicate admission",async({app})=>{
  await app.setViewportSize({width:760,height:550});
  const detail="The agent could not start.\n\n"+Array.from({length:30},(_,i)=>"Attempt "+(i+1)+": a detailed provider diagnostic.").join("\n");
  await app.evaluate(detail=>{
    window.__fixture.handlers.preview_project=({directory})=>({kind:"ready",directory});
    window.__fixture.handlers.open_session=()=>{throw detail;};
  },detail);
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await app.getByLabel("Project folder",{exact:true}).fill("/Projects/new-project");
  await app.locator("#picker-submit").click();
  await expect(app.locator("#picker-error")).toHaveText("Couldn’t start the conversation");
  await expect(app.locator("#picker-error")).toBeFocused();
  await expect(app.getByLabel("Project folder",{exact:true})).toHaveValue("/Projects/new-project");
  await expect(app.locator("#picker-error-source")).toBeHidden();
  await app.getByText("Show error details",{exact:true}).click();
  await expect(app.locator("#picker-error-source")).toHaveText(detail);
  for(const id of ["picker-submit","picker-cancel"]){
    const rect=await app.locator("#"+id).boundingBox();expect(rect.y).toBeGreaterThan(0);expect(rect.y+rect.height).toBeLessThan(550);
  }
  expect(await app.locator("#project-picker").evaluate(node=>node.scrollWidth<=node.clientWidth)).toBe(true);
  await app.evaluate(()=>window.__fixture.handlers.open_session=()=>new Promise(resolve=>window.finishAdmission=resolve));
  await app.getByRole("button",{name:"Try again",exact:true}).click();
  await expect(app.locator("#picker-submit")).toBeDisabled();
  await expect(app.locator("#picker-failure")).toBeHidden();
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="open_session").length)).toBe(2);
  await app.evaluate(()=>window.finishAdmission(window.__fixture.frames.get("release").address));
  await expect(app.locator("#project-picker")).toBeHidden();
  await expect(app.getByLabel("Message the agent",{exact:true})).toBeFocused();
});

test("short folder errors stay readable and cancellation restores focus",async({app})=>{
  await app.evaluate(()=>window.__fixture.handlers.preview_project=()=>{throw "This folder no longer exists. Choose another folder.";});
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await app.getByLabel("Project folder",{exact:true}).fill("/missing");
  await app.locator("#picker-submit").click();
  await expect(app.locator("#picker-error")).toHaveText("Couldn’t check this folder");
  await expect(app.locator("#picker-error-message")).toHaveText("This folder no longer exists. Choose another folder.");
  await expect(app.locator("#picker-error-details")).toBeHidden();
  await app.keyboard.press("Escape");
  await expect(app.getByRole("button",{name:"Open project",exact:true})).toBeFocused();
});

test("a short multiline diagnostic keeps its original line breaks in the disclosure",async({app})=>{
  const detail="Sign-in expired.\n\nOpen the agent and sign in.";
  await app.evaluate(detail=>{
    window.__fixture.handlers.preview_project=({directory})=>({kind:"ready",directory});
    window.__fixture.handlers.open_session=()=>{throw detail;};
  },detail);
  await app.getByRole("button",{name:"Open project",exact:true}).click();
  await app.getByLabel("Project folder",{exact:true}).fill("/Projects/new-project");
  await app.locator("#picker-submit").click();
  await expect(app.locator("#picker-error-message")).toBeHidden();
  await app.getByText("Show error details",{exact:true}).click();
  expect(await app.locator("#picker-error-source").textContent()).toBe(detail);
});
