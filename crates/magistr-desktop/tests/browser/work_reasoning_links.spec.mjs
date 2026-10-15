import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Reasoning",exact:true}).click();await app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();}
test("cross-session references and cycles retain local Back and origin focus",async({app})=>{
  await open(app);const related=app.getByRole("region",{name:"Related reasoning",exact:true});
  await related.getByRole("button",{name:"Inspect the installation result",exact:true}).click();
  await expect(app.getByLabel("Reasoning to inspect",{exact:true})).toHaveValue('"review"');
  await related.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();
  await app.getByRole("button",{name:"Back to previous step",exact:true}).click();
  await expect(related.getByRole("button",{name:"Keep each conversation with its saved message",exact:true})).toBeFocused();
  await app.getByRole("button",{name:"Back to previous step",exact:true}).click();
  await expect(related.getByRole("button",{name:"Inspect the installation result",exact:true})).toBeFocused();
  await app.getByRole("button",{name:"Back to reasoning",exact:true}).click();
  await expect(app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true})).toBeFocused();
});
test("revisions resolve across sessions and missing references never look readable",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,s=f.workflow.think.data.value.sessions[0].steps[0];s.revised_by=42;s.dependencies=[{step:999,relation:"refutes"}];f.publish("release",{workflow_revision:1});});
  const related=app.getByRole("region",{name:"Related reasoning",exact:true});await expect(related).toContainText("Revised by");await expect(related).toContainText("Referenced step unavailable");
  await expect(related.getByRole("button",{name:"Referenced step unavailable",exact:true})).toHaveCount(0);
  await related.getByRole("button",{name:"Inspect the installation result",exact:true}).first().click();
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.think.data.value.sessions[1].steps=[];f.publish("release",{workflow_revision:2});});
  await expect(app.locator("#work-browser-content")).toContainText("This step is no longer available.");
  await app.getByRole("button",{name:"Back to previous step",exact:true}).click();await expect(related).toContainText("Referenced step unavailable");
});
