import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();}
test("large task plans stay bounded and filters follow actual status updates",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,run=f.workflow.ship.data.value.current,base=run.tasks[0];run.tasks=Array.from({length:123},(_,n)=>({...base,id:"task-"+n,title:"Review "+n,status:n===122?"blocked":"planned",blocked_reason:n===122?"Need the result":null,started_at:"2026-10-15T08:00:00Z"}));run.task_instances=Object.fromEntries(run.tasks.map(t=>[t.id,"instance-"+t.id]));f.publish("release",{workflow_revision:1});});
  await expect(app.locator(".work-tasks>li")).toHaveCount(50);await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();await expect(app.locator(".work-tasks>li")).toHaveCount(23);
  await app.getByLabel("Task status",{exact:true}).selectOption("blocked");await expect(app.locator(".work-tasks>li")).toHaveCount(1);await expect(app.locator(".work-tasks")).toContainText("Review 122");await expect(app.locator(".work-tasks>li")).toHaveAttribute("data-position","123");await expect(app.locator(".work-tasks")).toContainText("Started");
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.ship.data.value.current.tasks[122].status="completed";f.publish("release",{workflow_revision:2});});await expect(app.locator("#work-browser-content")).toContainText("No tasks match these filters.");
  await app.getByLabel("Work to inspect",{exact:true}).selectOption("run:previous-run");await expect(app.getByLabel("Task status",{exact:true})).toHaveValue("all");await expect(app.locator(".work-tasks>li")).toHaveCount(3);
});
test("task text search preserves native input focus and complete matching details",async({app})=>{
  await open(app);const query=app.getByLabel("Find a task",{exact:true});await query.fill("installer");await expect(query).toBeFocused();await expect(app.locator(".work-tasks>li")).toHaveCount(1);await expect(app.locator(".work-tasks")).toContainText("Build and inspect the installer");
  await app.getByRole("tab",{name:"Roadmap",exact:true}).click();await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();await expect(query).toHaveValue("installer");await expect(app.locator(".work-tasks>li")).toHaveCount(1);
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.ship.data.value.current.run_id="replacement";f.publish("release",{workflow_revision:1});});
  await expect(query).toHaveValue("");await expect(app.locator(".work-tasks>li")).toHaveCount(3);
});
