import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();}
test("execution search pins exact current run while new work starts",async({app})=>{
  await open(app);await app.getByLabel("Search work history",{exact:true}).fill("expected saved");
  await app.getByRole("button",{name:"Prepare the desktop release",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Prepare the desktop release",exact:true})).toBeFocused();
  await app.evaluate(()=>{const f=window.__fixture,data=f.workflow.ship.data.value;data.retained.push(data.current);data.current={...structuredClone(data.current),run_id:"new-run",objective:{...data.current.objective,description:"Different work"}};f.publish("release",{workflow_revision:1});});
  await expect(app.getByRole("heading",{name:"Prepare the desktop release",exact:true})).toBeVisible();await expect(app.getByLabel("Work to inspect",{exact:true})).toHaveValue("run:release-run");
  await app.getByRole("button",{name:"Back to work results",exact:true}).click();await expect(app.getByRole("button",{name:"Prepare the desktop release",exact:true})).toBeFocused();
});
test("complete historical checks remain searchable beyond the first result page",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,data=f.workflow.ship.data.value,base=data.retained[0];data.retained=Array.from({length:101},(_,n)=>({...structuredClone(base),run_id:"saved-"+n,objective:{...base.objective,description:"Saved work "+n}}));data.retained.forEach(run=>run.tasks[1].checks[0].details="Long output ".repeat(500)+"日本語 ＡＢＣ");f.publish("release",{workflow_revision:1});});
  await app.getByLabel("Search work history",{exact:true}).fill("日本語 abc");await expect(app.locator(".roadmap-items>li")).toHaveCount(50);
  await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();
  await app.getByRole("button",{name:"Saved work 0",exact:true}).click();await expect(app.getByLabel("Work to inspect",{exact:true})).toHaveValue("run:saved-0");await app.getByRole("button",{name:"Back to work results",exact:true}).click();await expect(app.getByRole("button",{name:"Saved work 0",exact:true})).toBeFocused();
});
