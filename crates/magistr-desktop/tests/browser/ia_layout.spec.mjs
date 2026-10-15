import {test,expect,root} from "./fixture.mjs";
import {mkdir} from "node:fs/promises";
import {resolve} from "node:path";
import {captureIteration} from "./iteration_evidence.mjs";
async function openWork(app,tab){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  if(tab)await app.getByRole("tab",{name:tab,exact:true}).click();
}
async function capture(app,name){
  const dir=resolve(root,".magistr/desktop-ia");await mkdir(dir,{recursive:true});
  await app.screenshot({path:resolve(dir,name+".png"),animations:"disabled"});
  await captureIteration(app,{root,name,scenario:test.info().file});
}
test("01 roadmap sections navigate complete content and retain focus",async({app})=>{
  await openWork(app,"Roadmap");
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  for(const theme of ["dark","light"]){
    await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);
    await app.getByLabel("Jump to section",{exact:true}).selectOption({label:"Requirements"});
    const heading=app.getByRole("heading",{name:"Requirements",exact:true});
    await expect(heading).toBeFocused();await expect(heading).toBeInViewport();
    await expect(app.locator(".roadmap-requirements")).toContainText("The same conversation and unsent message return after reopening.");
    await capture(app,"01-sections-"+theme);
  }
  await app.evaluate(()=>window.__fixture.publish("release",{workflow_revision:1}));
  await expect(app.getByRole("heading",{name:"Requirements",exact:true})).toBeFocused();
});
test("02 roadmap prose uses compact readable documents",async({app})=>{
  await openWork(app,"Roadmap");await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  for(const theme of ["dark","light"]){
    await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);
    await app.getByLabel("Jump to section",{exact:true}).selectOption({label:"Description"});
    const section=app.locator(".roadmap-document").first();
    expect((await section.boundingBox()).height).toBeLessThan(95);
    await expect(section).toContainText("Keep the last conversation available when opening a project.");
    await capture(app,"02-documents-"+theme);
  }
});
test("03 roadmap checklist states are compact static marks",async({app})=>{
  await openWork(app,"Roadmap");await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  await app.getByLabel("Jump to section",{exact:true}).selectOption({label:"What changes"});
  await expect(app.locator(".roadmap-checklist").getByRole("img",{name:"Completed",exact:true})).toBeVisible();
  await expect(app.locator(".roadmap-checklist").getByRole("img",{name:"To do",exact:true})).toBeVisible();
  await expect(app.locator(".roadmap-checklist input")).toHaveCount(0);
  await expect(app.locator(".roadmap-requirements>header .work-record-count")).toHaveText("1");
  await capture(app,"03-checklists");
});
test("04 related work shows counts and readable state rows",async({app})=>{
  await openWork(app,"Roadmap");await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  await expect(app.locator(".roadmap-relations header .work-record-count")).toHaveText(["1","0"]);
  await app.locator(".roadmap-relations").scrollIntoViewIfNeeded();
  await capture(app,"04-relations");
  await app.getByRole("button",{name:"Store conversations and drafts",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Store conversations and drafts",exact:true})).toBeVisible();
});
test("05 roadmap status and readiness remain distinct",async({app})=>{
  await openWork(app,"Roadmap");await app.setViewportSize({width:760,height:550});
  await expect(app.locator(".roadmap-readiness-tag")).toHaveText(["Ready","Waiting for other work","Blocked"]);
  await expect(app.locator(".roadmap-items")).toContainText("The release date needs approval.");
  for(const theme of ["dark","light"]){
    await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);
    expect(await app.locator(".roadmap-items").evaluate(node=>node.scrollWidth-node.clientWidth)).toBeLessThanOrEqual(1);
    await capture(app,"05-status-"+theme);
  }
});
test("06 track choices show owner counts and preserve the selection",async({app})=>{
  await openWork(app,"Roadmap");
  const tracks=app.getByLabel("Track",{exact:true});
  await expect(tracks.locator('option[value=\'"Desktop"\']')).toHaveText("Desktop · 3 open");
  await tracks.selectOption('"Platform"');
  await expect(tracks.locator("option:checked")).toHaveText("Platform · Past work");
  await app.evaluate(()=>window.__fixture.publish("release",{workflow_revision:1}));
  await expect(tracks).toHaveValue('"Platform"');
  await capture(app,"06-tracks");
});
test("07 work searches clear directly and restore the collection",async({app})=>{
  await openWork(app,"Roadmap");await app.setViewportSize({width:760,height:550});
  for(const [tab,label] of [["Roadmap","Search all roadmap"],["Reasoning","Search all reasoning"],["Feedback","Search feedback"],["Current and saved work","Find a task"]]){
    await app.getByRole("tab",{name:tab,exact:true}).click();
    const input=app.getByLabel(label,{exact:true});await input.fill("no-matching-record-xyz");
    const clear=app.getByRole("button",{name:"Clear "+label.toLowerCase(),exact:true});
    await expect(clear).toBeVisible();await clear.click();await expect(input).toHaveValue("");await expect(input).toBeFocused();await expect(clear).toBeHidden();
  }
  await app.getByLabel("Find a task",{exact:true}).fill("installer");await capture(app,"07-search-clear");
});
test("08 find shortcut selects the visible work search",async({app})=>{
  await openWork(app,"Roadmap");
  for(const [tab,label] of [["Roadmap","Search all roadmap"],["Reasoning","Search all reasoning"],["Feedback","Search feedback"],["Current and saved work","Find a task"]]){
    await app.getByRole("tab",{name:tab,exact:true}).click();await app.keyboard.press("Control+f");await expect(app.getByLabel(label,{exact:true})).toBeFocused();
    await app.getByLabel(label,{exact:true}).fill("installer");await app.keyboard.press("Meta+f");
    expect(await app.getByLabel(label,{exact:true}).evaluate(input=>input.selectionEnd-input.selectionStart)).toBe(9);
    await app.getByLabel(label,{exact:true}).fill("");
  }
  await capture(app,"08-find");
});
test("09 work subviews use directly selectable native controls",async({app})=>{
  await openWork(app);await app.setViewportSize({width:760,height:550});
  const plan=app.getByRole("radio",{name:"Plan and tasks",exact:true});await plan.focus();await app.keyboard.press("ArrowRight");
  await expect(app.getByRole("radio",{name:"Checks",exact:true})).toBeChecked();await expect(app.getByLabel("Find a check",{exact:true})).toBeVisible();
  await app.keyboard.press("ArrowRight");await expect(app.getByLabel("Find activity",{exact:true})).toBeVisible();
  await app.getByRole("radio",{name:"Plan and tasks",exact:true}).check();await expect(app.getByLabel("Find a task",{exact:true})).toBeVisible();
  expect(await app.locator(".work-browser-filters").evaluate(node=>node.scrollWidth-node.clientWidth)).toBeLessThanOrEqual(1);
  await capture(app,"09-subviews");
});
test("10 task overview shows actual progress without repeated totals",async({app})=>{
  await openWork(app);await app.setViewportSize({width:760,height:550});
  const overview=app.locator(".work-task-overview");await overview.scrollIntoViewIfNeeded();
  await expect(overview.locator("dt")).toHaveText(["Total","Completed","In progress","Planned"]);
  await expect(overview.locator("dd")).toHaveText(["3","1","1","1"]);
  await expect(app.locator(".work-record-section>.work-pages")).toHaveCount(0);
  await capture(app,"10-task-overview");
  await app.getByLabel("Task status",{exact:true}).selectOption("active");
  await expect(app.locator(".work-tasks>li")).toHaveCount(1);await expect(overview.locator("dd").first()).toHaveText("3");
});
test("11 checks present labeled facts with complete recorded times",async({app})=>{
  await openWork(app);await app.getByRole("radio",{name:"Checks",exact:true}).check();
  const check=app.locator(".work-check").first();await check.locator("summary").click();
  await expect(check.locator(".work-check-facts dt")).toHaveText(["Required","Recorded","Exit code"]);
  await expect(check.locator(".work-check-facts time")).toHaveAttribute("datetime",/.+/);
  await app.setViewportSize({width:760,height:550});await check.scrollIntoViewIfNeeded();await capture(app,"11-check-facts");
});
test("12 activity kinds sit beside dated entries",async({app})=>{
  await openWork(app);await app.getByRole("radio",{name:"Activity and produced items",exact:true}).check();await app.getByLabel("Entries",{exact:true}).selectOption("activity");
  await app.setViewportSize({width:760,height:550});const row=app.locator(".work-action-list>li").first();await row.scrollIntoViewIfNeeded();
  await expect(row.locator(".work-action-kind")).not.toBeEmpty();await expect(row.locator("time")).toHaveAttribute("datetime",/.+/);
  const kind=await row.locator(".work-action-kind").boundingBox(),body=await row.locator(".work-action-description").boundingBox();expect(kind.x+kind.width).toBeLessThan(body.x);
  await capture(app,"12-activity-kinds");
});
test("13 produced items identify type beside their title",async({app})=>{
  await openWork(app);await app.getByRole("radio",{name:"Activity and produced items",exact:true}).check();await app.getByLabel("Entries",{exact:true}).selectOption("items");
  await app.setViewportSize({width:760,height:550});const item=app.locator(".work-artifacts article").first();await item.scrollIntoViewIfNeeded();
  await expect(item.locator(".work-artifact-type")).not.toBeEmpty();
  const type=await item.locator(".work-artifact-type").boundingBox(),title=await item.locator("h5").boundingBox();expect(type.x+type.width).toBeLessThan(title.x);
  await expect(item.getByRole("button",{name:"Copy reference",exact:true})).toBeVisible();await capture(app,"13-artifact-types");
});
test("14 reasoning facts sit beside the decision",async({app})=>{
  await openWork(app,"Reasoning");await app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  const facts=app.locator(".work-reasoning-facts");await expect(facts).toContainText("41");await expect(facts.locator("time")).toHaveAttribute("datetime",/.+/);
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await facts.scrollIntoViewIfNeeded();await capture(app,"14-reasoning-"+theme);}
});
test("15 feedback research exposes sources and compact facts",async({app})=>{
  await openWork(app,"Feedback");await app.getByRole("button",{name:"Keep my place when reopening a project",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  const note=app.locator(".feedback-research details").first();await note.locator("summary").click();
  await expect(note.locator("summary")).toContainText(/source/);await expect(note.locator("time")).toHaveAttribute("datetime",/.+/);
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await note.scrollIntoViewIfNeeded();await capture(app,"15-research-"+theme);}
});
test("16 project rows keep conversations in a compact shared column",async({app})=>{
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await app.setViewportSize({width:760,height:550});
    const project=app.locator(".directory-project").first();await project.scrollIntoViewIfNeeded();
    const cover=await project.locator(".directory-cover").boundingBox(),list=await project.locator(".directory-conversations").boundingBox();expect(list.y).toBeGreaterThanOrEqual(cover.y+cover.height-1);
    expect(await project.evaluate(node=>node.scrollWidth-node.clientWidth)).toBeLessThanOrEqual(1);await capture(app,"16-directory-"+theme);
  }
  await app.getByRole("button",{name:"Continue Review the release plan in Magistr",exact:true}).click();await expect(app.getByRole("heading",{name:"Review the release plan",exact:true})).toBeVisible();
});
test("17 project search filters list and map without changing the catalog",async({app})=>{
  await app.setViewportSize({width:760,height:550});const input=app.getByRole("searchbox",{name:"Filter projects",exact:true});
  await input.fill("ＳＹＮＣ");await expect(app.locator(".directory-project")).toHaveCount(1);await expect(input).toBeFocused();await expect(app.locator(".directory-project h2")).toHaveText("Magistr");
  await app.getByRole("button",{name:"Map",exact:true}).click();await expect(app.locator(".atlas-project")).toHaveCount(1);
  await app.getByRole("button",{name:"List",exact:true}).click();
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await capture(app,"17-search-"+theme);}
  await input.fill("nothing-matches-this");await expect(app.locator("#directory-empty")).toHaveText("No projects match your search.");
  await input.fill("");await expect(app.locator(".directory-project")).toHaveCount(2);
});
