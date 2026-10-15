import { test,expect } from "./fixture.mjs";
async function open(app){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Prepare the desktop release",exact:true})).toBeVisible();
}
test("current work preserves its complete requirements and conversation draft",async({app})=>{
  await open(app);
  await expect(app.locator(".work-tasks>li")).toHaveCount(3);
  await expect(app.locator("#work-browser-content")).toContainText("the previous build remains available");
  await app.keyboard.press("Escape");
  await expect(app.getByRole("button",{name:"Work plan",exact:true})).toBeFocused();
  await expect(app.locator("#draft")).toHaveValue("Keep the project list visible, and let me return to the same paragraph.");
});
test("work refresh follows owner revision and keeps stale records explicitly stale",async({app})=>{
  await open(app);
  const calls=()=>app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="read_workflow").length);
  expect(await calls()).toBe(1);
  await app.evaluate(()=>window.__fixture.publish("release",{status:"Working"}));
  expect(await calls()).toBe(1);
  await app.evaluate(()=>{
    const fixture=window.__fixture;
    fixture.workflow.ship={state:"stale",data:{last:fixture.workflow.ship.data,error:{kind:"io",message:"The work file cannot be read."}}};
    fixture.publish("release",{workflow_revision:1});
  });
  await expect(app.locator("#work-browser-state")).toContainText("Showing the last saved records.");
  await expect(app.locator(".work-tasks>li")).toHaveCount(3);
  expect(await calls()).toBe(2);
  await app.evaluate(()=>{
    const fixture=window.__fixture;fixture.workflow.ship={state:"current",data:fixture.workflow.ship.data.last};
    fixture.workflow.ship.data.value.current.tasks[1].status="completed";
    fixture.publish("release",{workflow_revision:2});
  });
  await expect(app.locator(".work-tasks>li").nth(1)).toHaveAttribute("data-status","completed");
  await expect(app.locator("#work-browser-state")).toHaveText("Up to date.");
});
test("missing, unreadable and failed refresh remain distinct",async({app})=>{
  await open(app);
  await app.evaluate(()=>{
    window.__fixture.workflow.ship={state:"current",data:{source:{kind:"missing"},value:{current:null,retained:[]}}};
    window.__fixture.publish("release",{workflow_revision:1});
  });
  await expect(app.locator("#work-browser-state")).toHaveText("No saved work yet.");
  await expect(app.getByRole("heading",{name:"No current work",exact:true})).toBeVisible();
  await app.evaluate(()=>{
    window.__fixture.workflow.ship={state:"unavailable",data:{kind:"decode",message:"The work file cannot be read."}};
    window.__fixture.publish("release",{workflow_revision:2});
  });
  await expect(app.locator("#work-browser-content")).toHaveText("Work records are unavailable. Use Refresh to try again.");
  await app.evaluate(()=>window.__fixture.handlers.read_workflow=()=>{throw new Error("Connection closed.");});
  await app.getByRole("button",{name:"Refresh",exact:true}).click();
  await expect(app.locator("#work-browser-state")).toContainText("Could not read work records.");
  await expect(app.getByRole("button",{name:"Refresh",exact:true})).toBeEnabled();
});
test("closed readers ignore late responses",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.evaluate(()=>window.__fixture.handlers.read_workflow=args=>new Promise(resolve=>window.__fixture.finishRead=()=>resolve({address:args.address,revision:0,snapshot:window.__fixture.workflow})));
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await expect(app.locator("#work-browser-state")).toHaveText("Reading work records…");
  await app.keyboard.press("Escape");
  await app.evaluate(()=>window.__fixture.finishRead());
  await expect(app.locator("#work-browser")).not.toBeVisible();
});
