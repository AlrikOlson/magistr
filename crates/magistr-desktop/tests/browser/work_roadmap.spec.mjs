import { test,expect } from "./fixture.mjs";
async function open(app){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
  await app.getByRole("tab",{name:"Roadmap",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Desktop",exact:true})).toBeVisible();
}
test("roadmap uses owner counts, full titles, closed tracks and ungrouped work",async({app})=>{
  await open(app);
  await expect(app.locator("#work-browser .surface-readiness span")).toHaveText(["1 ready","1 waiting","1 blocked"]);
  await expect(app.locator(".work-piece")).toHaveCount(5);
  await expect(app.locator('.work-piece[data-next="true"]')).toContainText("Restore the project conversation and draft");
  await expect(app.locator(".work-piece[data-next=true]")).toContainText("Next ready");
  await expect(app.locator("#work-browser .work-surface")).toContainText("The release date needs approval.");
  await app.getByLabel("Track",{exact:true}).selectOption('"Platform"');
  await expect(app.locator("#work-browser .surface-readiness span")).toHaveText(["0 ready","0 waiting","0 blocked"]);
  await expect(app.locator("#work-browser-content")).toContainText("This track has no unfinished items.");
  await app.getByLabel("Track",{exact:true}).selectOption("null");
  await expect(app.locator("#work-browser .work-surface")).toContainText("Improve the installation guide");
  await app.getByRole("tab",{name:"Roadmap",exact:true}).focus();await app.keyboard.press("Home");await app.keyboard.press("ArrowRight");
  await expect(app.getByRole("tab",{name:"Current and saved work",exact:true})).toBeFocused();
  await expect(app.locator("#work-run-selection")).toBeVisible();
  await expect(app.locator("#work-track-selection")).toBeHidden();
  await app.keyboard.press("ArrowRight");
  await expect(app.getByRole("tab",{name:"Roadmap",exact:true})).toBeFocused();
  await expect(app.getByLabel("Track",{exact:true})).toHaveValue("null");
});
test("roadmap refresh retains stale records and never silently changes a vanished track",async({app})=>{
  await open(app);
  await app.evaluate(()=>{
    const f=window.__fixture;f.roadmap={state:"stale",data:{last:f.roadmap.data,error:{kind:"io",message:"Cannot read the roadmap."}}};
    f.publish("release",{workflow_revision:1});
  });
  await expect(app.locator("#work-browser-state")).toContainText("Showing the last saved records.");
  await expect(app.locator("#work-browser .surface-readiness span")).toHaveText(["1 ready","1 waiting","1 blocked"]);
  await app.evaluate(()=>{
    const f=window.__fixture;f.roadmap={state:"current",data:f.roadmap.data.last};
    f.roadmap.data.value.frontiers=f.roadmap.data.value.frontiers.filter(track=>track.group!=="Desktop");
    delete f.roadmap.data.value.catalog.current.Desktop;f.publish("release",{workflow_revision:2});
  });
  await expect(app.locator("#work-browser-content")).toContainText("This track is no longer available.");
  await expect(app.getByLabel("Track",{exact:true})).toHaveValue('"Desktop"');
  await app.evaluate(()=>{window.__fixture.roadmap={state:"unavailable",data:{kind:"decode",message:"Cannot read the roadmap."}};window.__fixture.publish("release",{workflow_revision:3});});
  await expect(app.locator("#work-browser-content")).toHaveText("The roadmap is unavailable. Use Refresh to try again.");
});