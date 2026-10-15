import {test,expect,root} from "./fixture.mjs";
import {mkdir} from "node:fs/promises";
import {resolve} from "node:path";
export async function openWork(app,tab){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  if(tab)await app.getByRole("tab",{name:tab,exact:true}).click();
}
export async function capture(app,name){
  const dir=resolve(root,".magistr/desktop-ux");await mkdir(dir,{recursive:true});
  await app.screenshot({path:resolve(dir,name+".png"),animations:"disabled"});
}
test("work header leaves room for records in both appearances",async({app})=>{
  await openWork(app,"Roadmap");
  for(const theme of ["dark","light"]){
    await app.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    for(const width of [1280,760]){
      await app.setViewportSize({width,height:width===760?550:820});
      expect((await app.locator(".work-browser-heading").boundingBox()).height).toBeLessThan(90);
      await expect(app.getByRole("button",{name:"Refresh",exact:true})).toBeInViewport();
      expect(await app.locator("#work-browser").evaluate(node=>node.scrollWidth-node.clientWidth)).toBeLessThanOrEqual(1);
      await capture(app,"01-header-"+width+"-"+theme);
    }
  }
});

test("work navigation fits and follows arrow-key focus",async({app})=>{
  await openWork(app);
  await app.setViewportSize({width:760,height:550});
  const tabs=app.getByRole("tablist",{name:"Work view"});
  expect(await tabs.evaluate(node=>node.scrollWidth-node.clientWidth)).toBeLessThanOrEqual(1);
  expect((await tabs.boundingBox()).height).toBeLessThanOrEqual(45);
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).focus();
  await app.keyboard.press("End");
  await expect(app.getByRole("tab",{name:"Record status",exact:true})).toBeFocused();
  await expect(app.getByRole("tab",{name:"Record status",exact:true})).toHaveAttribute("aria-selected","true");
  await app.keyboard.press("Home");await app.keyboard.press("ArrowRight");
  await expect(app.getByRole("tab",{name:"Roadmap",exact:true})).toBeFocused();
  await capture(app,"02-navigation");
});

test("work filters group related fields and retain content space",async({app})=>{
  await openWork(app,"Roadmap");await app.setViewportSize({width:760,height:550});
  expect((await app.locator(".work-browser-filters").boundingBox()).height).toBeLessThan(85);
  await app.getByLabel("Search all roadmap",{exact:true}).fill("installer");
  await expect(app.getByRole("button",{name:"Build the desktop installer",exact:true})).toBeVisible();
  await app.getByLabel("Search all roadmap",{exact:true}).fill("");
  await capture(app,"03-filters");
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
  expect(await app.locator("#work-browser").evaluate(node=>node.scrollWidth-node.clientWidth)).toBeLessThanOrEqual(1);
  expect((await app.locator("#work-browser-content").boundingBox()).height).toBeGreaterThan(220);
});

test("roadmap totals leave the first items visible",async({app})=>{
  await openWork(app,"Roadmap");await app.setViewportSize({width:760,height:550});
  for(const theme of ["dark","light"]){
    await app.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    expect((await app.locator(".roadmap-facts").boundingBox()).height).toBeLessThan(55);
    await expect(app.locator(".roadmap-facts dd")).toHaveText(["3","1","1","1","0","0"]);
    await expect(app.getByRole("button",{name:"Build the desktop installer",exact:true})).toBeInViewport();
    await capture(app,"04-totals-"+theme);
  }
});

test("roadmap rows expose readiness and blockers without oversized cards",async({app})=>{
  await openWork(app,"Roadmap");await app.setViewportSize({width:760,height:550});
  const rows=app.locator(".roadmap-items>li");await expect(rows).toHaveCount(3);
  expect((await rows.first().boundingBox()).height).toBeLessThan(80);
  await expect(rows.last()).toBeInViewport();
  await capture(app,"05-roadmap-rows");
});

test("roadmap detail keeps metadata beside the title",async({app})=>{
  await openWork(app,"Roadmap");await app.setViewportSize({width:760,height:550});
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  expect((await app.locator(".roadmap-item-heading").boundingBox()).height).toBeLessThan(65);
  await expect(app.locator(".roadmap-item-heading")).toContainText("Desktop · Planned");
  await capture(app,"06-roadmap-detail");
});

test("requirements remain complete in compact numbered rows",async({app})=>{
  await openWork(app,"Roadmap");await app.setViewportSize({width:760,height:550});
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  const requirements=app.locator(".roadmap-requirements");await requirements.scrollIntoViewIfNeeded();
  await expect(requirements).toContainText("The same conversation and unsent message return after reopening.");
  expect((await requirements.locator("li").first().boundingBox()).height).toBeLessThan(80);
  await capture(app,"07-requirements");
});

test("dependency groups share a compact comparison row",async({app})=>{
  await openWork(app,"Roadmap");await app.setViewportSize({width:760,height:550});
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  const required=await app.getByRole("region",{name:"Required work",exact:true}).boundingBox();
  const dependent=await app.getByRole("region",{name:"Work that depends on this",exact:true}).boundingBox();
  expect(Math.abs(required.y-dependent.y)).toBeLessThan(2);
  expect((await app.locator(".roadmap-relations").boundingBox()).height).toBeLessThan(165);
  await capture(app,"08-dependencies");
});

test("current work reveals complete requirements on demand",async({app})=>{
  await openWork(app);await app.setViewportSize({width:760,height:550});
  const disclosure=app.locator(".work-objective-details").first();
  await expect(disclosure).not.toHaveAttribute("open","");
  await disclosure.locator("summary").focus();await app.keyboard.press("Enter");
  await expect(disclosure.locator("li").last()).toBeVisible();
  await expect(disclosure).toContainText("the previous build remains available");
  await app.evaluate(()=>{window.__fixture.publish("release",{workflow_revision:1});});
  await expect(disclosure).toHaveAttribute("open","");
  await disclosure.locator("summary").click();await capture(app,"09-current-work");
});

test("task rows align title and status at the minimum width",async({app})=>{
  await openWork(app);await app.setViewportSize({width:760,height:550});
  await app.locator(".work-tasks").scrollIntoViewIfNeeded();
  const heading=app.locator(".work-task-heading").first();
  const title=await heading.locator("strong").boundingBox(),state=await heading.locator("span").boundingBox();
  expect(Math.abs(title.y-state.y)).toBeLessThan(6);
  expect((await app.locator(".work-tasks>li").first().boundingBox()).height).toBeLessThan(85);
  await capture(app,"10-tasks");
});

test("task metadata uses labeled facts and complete times",async({app})=>{
  await openWork(app);await app.evaluate(()=>{const f=window.__fixture;f.workflow.ship.data.value.current.tasks[1].started_at="2026-10-15T08:12:00Z";f.publish("release",{workflow_revision:1});});
  const row=app.locator(".work-tasks>li").nth(1);await row.locator(".work-task-heading").scrollIntoViewIfNeeded();
  await expect(row.locator(".work-inline-facts dt")).toHaveText(["Type","Estimate","Started"]);
  await expect(row.locator(".work-inline-facts time")).toHaveAttribute("datetime","2026-10-15T08:12:00Z");
  await capture(app,"11-task-facts");
});
