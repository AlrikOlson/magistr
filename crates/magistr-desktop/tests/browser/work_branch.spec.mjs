import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Reasoning",exact:true}).click();}
test("investigation membership and lifecycle stay scoped to one reasoning session",async({app})=>{
  await open(app);await expect(app.locator(".roadmap-items>li")).toHaveCount(2);
  const select=app.getByLabel("Investigation",{exact:true});await select.selectOption("main");await expect(app.locator(".roadmap-items>li")).toHaveCount(1);
  await select.selectOption("branch:storage");await expect(app.locator(".roadmap-items>li")).toHaveCount(1);await expect(app.locator("#work-browser-content")).toContainText("Included in conclusions");
  await app.getByRole("button",{name:"Check whether reopening needs a second saved message",exact:true}).click();await app.getByRole("button",{name:"Back to reasoning",exact:true}).click();
  await expect(select).toHaveValue("branch:storage");
  await app.evaluate(()=>{const f=window.__fixture,h=f.workflow.think.data.value.sessions[1];h.steps[0].branch_id="storage";h.branches=[{id:"storage",name:"Different investigation",status:"abandoned",from_step:42,steps:[],created_at:"2026-10-15T08:00:00Z",depth:1}];f.publish("release",{workflow_revision:1});});
  await app.getByLabel("Reasoning to inspect",{exact:true}).selectOption('"review"');await expect(select).toHaveValue("all");
  await select.selectOption("branch:storage");await expect(app.locator("#work-browser-content")).toContainText("Different investigation");await expect(app.locator("#work-browser-content")).toContainText("Abandoned");
  await expect(app.getByRole("button",{name:"Inspect the installation result",exact:true})).toBeVisible();
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.think.data.value.sessions[1].branches=[];f.publish("release",{workflow_revision:2});});
  await expect(app.locator("#work-browser-content")).toContainText("This investigation is no longer available.");
});
