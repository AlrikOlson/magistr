import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();await app.getByRole("tab",{name:"Reasoning",exact:true}).click();}
test("search reaches complete cross-session fields and keeps later pages on return",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,h=f.workflow.think.data.value.sessions[1],base=h.steps[0];h.steps=Array.from({length:101},(_,n)=>({...base,record_id:"search-"+n,step_number:100+n,purpose:"Review "+n,uncertainty_notes:"Unknown result. ".repeat(500)+"日本語 ＡＢＣ"}));f.publish("release",{workflow_revision:1});});
  const input=app.getByLabel("Search all reasoning",{exact:true});await input.fill("日本語 abc");await expect(input).toBeFocused();await expect(app.locator(".roadmap-items>li")).toHaveCount(50);
  await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();
  await app.getByRole("button",{name:"Review 100",exact:true}).click();await expect(app.locator(".reasoning-step")).toContainText("日本語 ＡＢＣ");
  await app.getByRole("button",{name:"Back to reasoning",exact:true}).click();await expect(app.getByRole("button",{name:"Review 100",exact:true})).toBeFocused();
  await expect(app.getByRole("navigation",{name:"Result pages",exact:true})).toContainText("Page 3 of 3 · 101 results");
  await input.fill("unrecorded phrase");await expect(app.locator("#work-browser-content")).toContainText("No recorded reasoning matches this search.");
});