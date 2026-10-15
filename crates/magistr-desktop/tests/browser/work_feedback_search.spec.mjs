import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Feedback",exact:true}).click();}
test("feedback filters search full research and preserve later pages on return",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,base=f.workflow.signals.data.value[0];f.workflow.signals.data.value=Array.from({length:121},(_,n)=>({...base,id:"feedback-"+n,content:{...base.content,summary:"Concern "+n},enrichment:[{...base.enrichment[0],summary:"Full detail ".repeat(500)+"日本語 ＡＢＣ"}]}));f.publish("release",{workflow_revision:1});});
  await app.getByLabel("Kind",{exact:true}).selectOption("concern");await app.getByLabel("Status",{exact:true}).selectOption("researched");
  const input=app.getByLabel("Search feedback",{exact:true});await input.fill("日本語 abc");await expect(input).toBeFocused();await expect(app.locator(".roadmap-items>li")).toHaveCount(50);
  await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();
  await app.getByRole("button",{name:"Concern 120",exact:true}).click();await app.getByRole("button",{name:"Back to feedback",exact:true}).click();await expect(app.getByRole("button",{name:"Concern 120",exact:true})).toBeFocused();
  await expect(app.getByRole("navigation",{name:"Result pages",exact:true})).toContainText("Page 3 of 3 · 121 results");
  await app.getByLabel("Status",{exact:true}).selectOption("new");await expect(app.locator("#work-browser-content")).toContainText("No feedback matches these filters.");
});
