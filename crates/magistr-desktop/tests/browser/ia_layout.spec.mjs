import {test,expect,root} from "./fixture.mjs";
import {mkdir} from "node:fs/promises";
import {resolve} from "node:path";
async function openWork(app,tab){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  if(tab)await app.getByRole("tab",{name:tab,exact:true}).click();
}
async function capture(app,name){
  const dir=resolve(root,".magistr/desktop-ia");await mkdir(dir,{recursive:true});
  await app.screenshot({path:resolve(dir,name+".png"),animations:"disabled"});
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
