import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Feedback",exact:true}).click();await app.getByRole("button",{name:"Keep my place when reopening a project",exact:true}).click();}
test("feedback links open exact roadmap and reasoning records and restore research reading",async({app})=>{
  await open(app);await app.getByRole("region",{name:"Related work",exact:true}).getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await expect(app.locator(".roadmap-item-detail")).toBeVisible();await app.getByRole("button",{name:"Return to feedback",exact:true}).click();
  await expect(app.getByRole("region",{name:"Related work",exact:true}).getByRole("button",{name:"Restore the project conversation and draft",exact:true})).toBeFocused();
  const note=app.locator(".feedback-research details").first();await note.locator("summary").click();await note.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();
  await expect(app.locator(".reasoning-step")).toContainText("Step 41");await app.getByRole("button",{name:"Return to feedback",exact:true}).click();
  await expect(note).toHaveAttribute("open","");await expect(note.getByRole("button",{name:"Keep each conversation with its saved message",exact:true})).toBeFocused();
});
test("missing, unreadable and ambiguous references never guess another record",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture;f.workflow.signals.data.value[0].cross_refs=["chunk:missing","task:build","think:41"];f.workflow.think={state:"unavailable",data:{kind:"io",message:"Cannot read reasoning."}};f.publish("release",{workflow_revision:1});});
  const related=app.getByRole("region",{name:"Related work",exact:true});await expect(related).toContainText("The related record is no longer available.");await expect(related).toContainText("does not identify a saved record");await expect(related).toContainText("The related records cannot be read.");
  await expect(related.getByRole("button")).toHaveCount(3);
});
