import {test,expect} from "./fixture.mjs";
async function open(app){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
  await app.getByRole("tab",{name:"Plan",exact:true}).click();await app.getByLabel("Track",{exact:true}).selectOption('"Platform"');
  await app.getByLabel("Items",{exact:true}).selectOption("closed");
}
test("closed tracks expose complete item details and preserve history choice",async({app})=>{
  await open(app);await app.getByRole("button",{name:"Store conversations and drafts",exact:true}).click();
  await expect(app.locator(".plan-item-detail")).toContainText("Save the complete conversation and its draft.");
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await expect(app.getByRole("button",{name:"Store conversations and drafts",exact:true})).toBeFocused();
  await expect(app.getByLabel("Items",{exact:true})).toHaveValue("closed");
  await app.evaluate(()=>{const f=window.__fixture,c=f.workflow.plan.data.value.tasks.find(c=>c.id==="foundation");c.status="retired";c.retired_reason="Replaced after the storage review. <script>never()</script>";f.publish("release",{workflow_revision:1});});
  await expect(app.locator(".work-piece")).toContainText("Retired");
  await app.getByRole("button",{name:"Store conversations and drafts",exact:true}).click();
  await expect(app.locator(".plan-item-detail")).toContainText("<script>never()</script>");
  await expect(app.locator(".plan-item-detail script")).toHaveCount(0);
  await app.getByLabel("Track",{exact:true}).selectOption('"Desktop"');
  await expect(app.locator("#work-browser-content")).toContainText("This track has no completed or retired items.");
});
test("a newly completed item moves into history without replacing the selected track",async({app})=>{
  await open(app);await app.getByLabel("Track",{exact:true}).selectOption('"Desktop"');
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.plan.data.value.tasks[0].status="done";f.publish("release",{workflow_revision:1});});
  await expect(app.getByRole("button",{name:"Restore the project conversation and draft",exact:true})).toBeVisible();
  await expect(app.getByLabel("Track",{exact:true})).toHaveValue('"Desktop"');
  await app.evaluate(()=>{const f=window.__fixture,c=f.workflow.plan.data.value.tasks.find(c=>c.id==="guide");delete c.feature;c.status="done";f.publish("release",{workflow_revision:2});});
  await app.getByLabel("Track",{exact:true}).selectOption("null");
  await expect(app.getByRole("button",{name:"Improve the installation guide",exact:true})).toBeVisible();
});