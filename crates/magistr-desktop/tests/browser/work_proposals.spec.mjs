import {test,expect} from "./fixture.mjs";
test("suggestions preserve current values and show complete reasons without applying them",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Roadmap",exact:true}).click();
  await app.evaluate(()=>{Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async value=>window.__copied=value}});const f=window.__fixture,c=f.workflow.roadmap.data.value.chunks[2];c.title_proposal={suggested_title:"New title <script>literal</script>",reason:"A clearer name. 日本語",source:"ext:tracker/42",proposed_at:"2026-10-15T08:00:00Z"};c.reprioritize={suggested_priority:50,reason:"Move this earlier.",proposed_at:"2026-10-15T08:00:00Z"};f.publish("release",{workflow_revision:1});});
  await app.getByRole("button",{name:"Publish the desktop release",exact:true}).click();
  const proposals=app.getByRole("region",{name:"Suggested changes",exact:true});await expect(proposals).toContainText("These suggestions have not changed the plan.");await expect(proposals.locator("dd")).toHaveText(["Publish the desktop release","New title <script>literal</script>","Blocked","Planned","120","50"]);
  await expect(proposals.locator("script")).toHaveCount(0);await proposals.getByRole("button",{name:"Copy reason",exact:true}).first().click();expect(await app.evaluate(()=>window.__copied)).toBe("A clearer name. 日本語");
  await app.evaluate(()=>{const f=window.__fixture,c=f.workflow.roadmap.data.value.chunks[2];delete c.title_proposal;delete c.status_proposal;delete c.reprioritize;f.publish("release",{workflow_revision:2});});
  await expect(proposals).toHaveCount(0);await expect(app.locator(".roadmap-item-detail")).toContainText("Blocked");
  expect(await app.evaluate(()=>window.__fixture.calls.some(c=>c.command==="session_action"||c.command==="control_work"))).toBe(false);
});
