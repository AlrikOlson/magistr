import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Feedback",exact:true}).click();}
test("feedback reads full authored context and copies literal long text",async({app})=>{
  await open(app);await app.evaluate(()=>{Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async value=>window.__copied=value}});const f=window.__fixture;f.workflow.signals.data.value[0].body="日本語\n".repeat(4000)+"<img onerror=alert(1)>";f.publish("release",{workflow_revision:1});});
  await app.getByRole("button",{name:"Keep my place when reopening a project",exact:true}).click();
  await expect(app.locator(".feedback-detail")).toContainText("Project owner");await expect(app.locator(".feedback-detail")).toContainText("After closing and reopening");
  await expect(app.locator(".feedback-detail")).toContainText("The text I was writing should still be ready to edit.");await expect(app.locator(".feedback-detail img")).toHaveCount(0);
  await app.getByRole("button",{name:"Copy feedback",exact:true}).click();expect(await app.evaluate(()=>window.__copied)).toBe("日本語\n".repeat(4000)+"<img onerror=alert(1)>");
  await app.getByRole("button",{name:"Back to feedback",exact:true}).click();await expect(app.getByRole("button",{name:"Keep my place when reopening a project",exact:true})).toBeFocused();
});
test("stale, removed and unavailable feedback remain distinct",async({app})=>{
  await open(app);await app.getByRole("button",{name:"Keep my place when reopening a project",exact:true}).click();
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.signals={state:"stale",data:{last:f.workflow.signals.data,error:{kind:"io",message:"Cannot read feedback."}}};f.publish("release",{workflow_revision:1});});
  await expect(app.locator("#work-browser-state")).toContainText("Showing the last saved records.");await expect(app.locator(".feedback-detail")).toBeVisible();
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.signals={state:"current",data:{source:{kind:"memory"},value:[]}};f.publish("release",{workflow_revision:2});});
  await expect(app.locator("#work-browser-content")).toContainText("This feedback is no longer available.");
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.signals={state:"unavailable",data:{kind:"io",message:"Cannot read feedback."}};f.publish("release",{workflow_revision:3});});
  await expect(app.locator("#work-browser-content")).toContainText("Feedback is unavailable.");
});
