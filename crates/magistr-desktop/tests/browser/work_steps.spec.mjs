import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();}
test("large step plans stay bounded and filters follow actual status updates",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,run=f.workflow.run.data.value.current,base=run.steps[0];run.steps=Array.from({length:123},(_,n)=>({...base,id:"step-"+n,title:"Review "+n,status:n===122?"waiting":"planned",waiting_reason:n===122?"Need the result":null,started_at:"2026-10-15T08:00:00Z"}));run.step_instances=Object.fromEntries(run.steps.map(t=>[t.id,"instance-"+t.id]));f.publish("release",{workflow_revision:1});});
  await expect(app.locator(".work-steps>li")).toHaveCount(50);await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();await expect(app.locator(".work-steps>li")).toHaveCount(23);
  await app.getByLabel("Step status",{exact:true}).selectOption("waiting");await expect(app.locator(".work-steps>li")).toHaveCount(1);await expect(app.locator(".work-steps")).toContainText("Review 122");await expect(app.locator(".work-steps>li")).toHaveAttribute("data-position","123");await expect(app.locator(".work-steps")).toContainText("Started");
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.run.data.value.current.steps[122].status="done";f.publish("release",{workflow_revision:2});});await expect(app.locator("#work-browser-content")).toContainText("No steps match these filters.");
  await app.getByLabel("Work to inspect",{exact:true}).selectOption("run:previous-run");await expect(app.getByLabel("Step status",{exact:true})).toHaveValue("all");await expect(app.locator(".work-steps>li")).toHaveCount(3);
});
test("step text search preserves native input focus and complete matching details",async({app})=>{
  await open(app);const query=app.getByLabel("Find a step",{exact:true});await query.fill("installer");await expect(query).toBeFocused();await expect(app.locator(".work-steps>li")).toHaveCount(1);await expect(app.locator(".work-steps")).toContainText("Build and inspect the installer");
  await app.getByRole("tab",{name:"Plan",exact:true}).click();await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();await expect(query).toHaveValue("installer");await expect(app.locator(".work-steps>li")).toHaveCount(1);
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.run.data.value.current.run_id="replacement";f.publish("release",{workflow_revision:1});});
  await expect(query).toHaveValue("");await expect(app.locator(".work-steps>li")).toHaveCount(3);
});