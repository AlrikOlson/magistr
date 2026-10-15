import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByLabel("Show",{exact:true}).selectOption("checks");}
test("check index preserves attempts, task context and complete copied output",async({app})=>{
  await open(app);await expect(app.locator(".work-check")).toHaveCount(3);await expect(app.locator(".work-check-index")).toContainText("3 recorded check attempts");
  await app.getByLabel("Check results",{exact:true}).selectOption("failed");await expect(app.locator(".work-check")).toHaveCount(1);
  await expect(app.locator(".work-check-attempt")).toContainText("Build and inspect the installer");
  await app.locator(".work-check summary").click();await app.evaluate(()=>Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async value=>window.__copied=value}}));
  await app.getByRole("button",{name:"Copy check output",exact:true}).click();expect(await app.evaluate(()=>window.__copied)).toContain("Expected the saved message; found an empty draft.");
  await app.getByLabel("Check results",{exact:true}).selectOption("reported");await expect(app.locator(".work-check summary")).toContainText("Reported · Passed");
  const identity=await app.locator(".work-check").getAttribute("data-reading-key");await app.locator(".work-check summary").click();
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.ship.data.value.current.tasks[1].checks[2].details+=" More detail.";f.publish("release",{workflow_revision:1});});
  await expect(app.locator(".work-check")).toHaveAttribute("data-reading-key",identity);await expect(app.locator(".work-check")).toHaveAttribute("open","");
  await expect(app.locator(".work-check-index")).toContainText("They do not confirm that the files are unchanged today.");
});
test("later check pages and full failure text remain searchable with literal matching",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,t=f.workflow.ship.data.value.current.tasks[1],base=t.checks[0];t.checks=Array.from({length:123},(_,i)=>({...base,receipt_id:"attempt-"+i,name:"Attempt "+i,details:"Output "+i,results:{...base.results,failures:[{name:"Case "+i,message:i===122?"日本語 final diagnosis":"Earlier"}]}}));f.publish("release",{workflow_revision:1});});
  await expect(app.locator(".work-check")).toHaveCount(50);await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();await expect(app.locator(".work-check")).toHaveCount(23);
  const query=app.getByLabel("Find a check",{exact:true});await query.fill("日本語 final");await expect(query).toBeFocused();await expect(app.locator(".work-check summary")).toContainText("Attempt 122");
  await app.getByLabel("Work to inspect",{exact:true}).selectOption("run:previous-run");await expect(query).toHaveValue("");await expect(app.locator(".work-check")).toHaveCount(3);
});
