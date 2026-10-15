import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();await app.getByRole("radio",{name:"Activity and produced items",exact:true}).check();}
test("activity index searches mentions and copies exact produced references",async({app})=>{
  await open(app);await expect(app.locator(".work-activity-entry")).toHaveCount(3);
  const query=app.getByLabel("Find activity",{exact:true});await query.fill("reopen.rs");await expect(query).toBeFocused();await expect(app.locator(".work-activity-entry")).toHaveCount(1);
  await expect(app.locator(".work-activity-entry")).toContainText("Files mentioned");await expect(app.locator(".work-activity-entry")).toContainText("Build and inspect the installer");
  await query.fill("");await app.getByLabel("Entries",{exact:true}).selectOption("items");await expect(app.locator(".work-activity-entry")).toHaveCount(2);
  await app.evaluate(()=>{Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async text=>window.__copied=text}});window.__fixture.handlers.open_link=()=>{};});
  await app.locator(".work-artifacts").first().getByRole("button",{name:"Copy reference",exact:true}).click();expect(await app.evaluate(()=>window.__copied)).toBe("docs/release-checklist.md");
  await app.getByRole("button",{name:"Open link",exact:true}).click();expect(await app.evaluate(()=>window.__fixture.calls.findLast(call=>call.command==="open_link").args.destination)).toBe("https://example.com/review/42");
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.ship.data.value.current.tasks[1].artifacts.push({type:"file",ref:"javascript:alert(1)",description:"Plain reference"});f.publish("release",{workflow_revision:1});});
  await expect(app.locator(".work-activity-entry")).toHaveCount(3);await expect(app.locator(".work-activity-entry").last().getByRole("button",{name:"Open link",exact:true})).toHaveCount(0);
});
test("all recorded actions remain reachable and updates do not mix saved work",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,t=f.workflow.ship.data.value.current.tasks[1],base=t.actions[0];t.artifacts=[];t.actions=Array.from({length:123},(_,i)=>({...base,id:i+1,description:"Action "+i,result:i===122?"Full result 日本語 <script>plain text</script>":"Recorded "+i}));f.publish("release",{workflow_revision:1});});
  await expect(app.locator(".work-activity-entry")).toHaveCount(50);await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();await expect(app.locator(".work-activity-entry")).toHaveCount(23);
  await app.getByLabel("Find activity",{exact:true}).fill("日本語");await expect(app.locator(".work-activity-entry")).toHaveCount(1);await expect(app.locator(".work-activity-entry")).toContainText("Action 122");await expect(app.locator(".work-activity-index script")).toHaveCount(0);
  await app.evaluate(()=>Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async text=>window.__copied=text}}));await app.getByRole("button",{name:"Copy result",exact:true}).click();expect(await app.evaluate(()=>window.__copied)).toBe("Full result 日本語 <script>plain text</script>");
  await app.getByLabel("Work to inspect",{exact:true}).selectOption("run:previous-run");await expect(app.getByLabel("Find activity",{exact:true})).toHaveValue("");await expect(app.locator(".work-activity-entry")).toHaveCount(3);await expect(app.locator(".work-activity-index")).not.toContainText("Action 122");
});