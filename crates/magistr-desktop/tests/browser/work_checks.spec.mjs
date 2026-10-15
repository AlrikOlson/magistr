import { test,expect } from "./fixture.mjs";
async function open(app){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await expect(app.locator(".work-check")).toHaveCount(3);
}
test("checks keep attempts, distinguish claims and copy complete output",async({app})=>{
  await open(app);
  const checks=app.locator(".work-check");
  await expect(checks.nth(0).locator("summary")).toContainText("Executed · Failed");
  await expect(checks.nth(1).locator("summary")).toContainText("Executed · Passed");
  await expect(checks.nth(2).locator("summary")).toContainText("Reported · Passed");
  await checks.nth(0).locator("summary").click();
  await expect(checks.nth(0)).toContainText("Exit code 101");
  await expect(checks.nth(0)).toContainText("1 passed · 1 failed · 0 skipped");
  await app.evaluate(()=>Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async text=>{window.__copied=text;}}}));
  await checks.nth(0).getByRole("button",{name:"Copy check output",exact:true}).click();
  expect(await app.evaluate(()=>window.__copied)).toBe(await app.evaluate(()=>window.__fixture.workflow.ship.data.value.current.tasks[1].checks[0].details));
  await checks.nth(2).locator("summary").click();
  await expect(checks.nth(2)).toContainText("This result was reported without running a command.");
  await expect(checks.nth(2)).toHaveAttribute("data-verified","false");
});
test("check disclosure and focus survive changed results while output stays literal",async({app})=>{
  await open(app);
  const summary=app.locator(".work-check").nth(1).locator("summary");
  await summary.click();await summary.focus();
  await app.evaluate(()=>{
    const fixture=window.__fixture,check=fixture.workflow.ship.data.value.current.tasks[1].checks[1];
    check.details+="\n日本語\r\n<img src=x onerror=alert(1)>\nEnd of complete output";
    fixture.publish("release",{workflow_revision:1});
  });
  await expect(app.locator(".work-check").nth(1)).toHaveAttribute("open","");
  await expect(summary).toBeFocused();
  await expect(app.locator(".work-check").nth(1).getByLabel("Check output",{exact:true})).toContainText("<img src=x onerror=alert(1)>");
  await expect(app.locator(".work-check img")).toHaveCount(0);
  await expect(app.locator(".work-check").nth(1)).toContainText("End of complete output");
});
