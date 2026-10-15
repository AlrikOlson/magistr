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

test("check outcomes stay aligned with long names",async({app})=>{
  await openWork(app);await app.getByLabel("Show",{exact:true}).selectOption("checks");
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.ship.data.value.current.tasks[1].checks[0].name="Verify installation and complete restoration of the conversation, file attachments and unsent message";f.publish("release",{workflow_revision:1});});
  await app.setViewportSize({width:760,height:550});
  const summary=app.locator(".work-check summary").first();await summary.scrollIntoViewIfNeeded();
  const title=await summary.locator("strong").boundingBox(),result=await summary.locator("span").boundingBox();
  expect(title.x+title.width).toBeLessThan(result.x);
  await summary.focus();await app.keyboard.press("Enter");await expect(app.locator(".work-check").first()).toHaveAttribute("open","");
  await capture(app,"12-check-results");
});

test("activity aligns timestamps and compact references",async({app})=>{
  await openWork(app);await app.getByLabel("Show",{exact:true}).selectOption("activity");
  await app.getByLabel("Entries",{exact:true}).selectOption("activity");
  const row=app.locator(".work-action-list>li").first();await row.scrollIntoViewIfNeeded();
  await expect(row.locator(".work-action-references")).toHaveCount(2);
  await expect(row).toContainText("crates/desktop/src/reopen.rs");
  await capture(app,"13-activity");
});

test("produced items place actions alongside their reference",async({app})=>{
  await openWork(app);await app.getByLabel("Show",{exact:true}).selectOption("activity");await app.getByLabel("Entries",{exact:true}).selectOption("items");
  await app.setViewportSize({width:760,height:550});
  const row=app.locator(".work-artifacts article").first();await row.scrollIntoViewIfNeeded();
  const reference=await row.locator(".work-artifact-reference").boundingBox(),actions=await row.locator(".work-artifact-actions").boundingBox();
  expect(reference.x+reference.width).toBeLessThan(actions.x);
  expect((await row.boundingBox()).height).toBeLessThan(90);
  await capture(app,"14-produced-items");
});

test("small result sets show counts and larger sets retain paging",async({app})=>{
  await openWork(app);await app.getByLabel("Search work history",{exact:true}).fill("installer");
  await expect(app.getByRole("button",{name:"Next page",exact:true})).toHaveCount(0);
  await expect(app.locator(".work-pages")).toContainText("2 results");
  await capture(app,"15-work-results");
  await app.evaluate(()=>{const f=window.__fixture,ship=f.workflow.ship.data.value,run=ship.retained[0];ship.retained=Array.from({length:70},(_,n)=>({...run,run_id:"saved-"+n}));f.publish("release",{workflow_revision:1});});
  await expect(app.getByRole("button",{name:"Next page",exact:true})).toBeEnabled();
  await app.getByRole("button",{name:"Next page",exact:true}).click();
  await expect(app.locator(".roadmap-items>li")).toHaveCount(21);
});

test("reasoning rows align sequence beside the decision",async({app})=>{
  await openWork(app,"Reasoning");await app.setViewportSize({width:760,height:550});
  const row=app.locator(".reasoning-row").first(),number=await row.locator(".reasoning-step-number").boundingBox(),button=await row.locator("button").boundingBox();
  expect(number.x+number.width).toBeLessThan(button.x);
  await capture(app,"16-reasoning-rows");
});

test("reasoning presents the outcome before supporting details",async({app})=>{
  await openWork(app,"Reasoning");await app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  await expect(app.locator(".reasoning-step .reasoning-prose").first().locator("h4")).toHaveText("Outcome");
  await expect(app.locator(".reasoning-step .reasoning-prose").first()).toBeInViewport();
  await capture(app,"17-reasoning-detail");
});

test("investigation summary keeps its origin and conclusion together",async({app})=>{
  await openWork(app,"Reasoning");await app.getByLabel("Investigation",{exact:true}).selectOption("branch:storage");
  await app.setViewportSize({width:760,height:550});
  const summary=app.locator(".reasoning-investigation");expect((await summary.boundingBox()).height).toBeLessThan(130);
  await expect(summary.locator("dd").first()).toHaveText("Step 41");await expect(summary.locator("dd").nth(1)).toHaveText("Step 41");
  await capture(app,"18-investigation");
});

test("feedback rows align kind title and status",async({app})=>{
  await openWork(app,"Feedback");await app.setViewportSize({width:760,height:550});
  const row=app.locator(".feedback-row").first();const kind=await row.locator(".feedback-kind").boundingBox(),title=await row.locator("button").boundingBox();
  expect(kind.x+kind.width).toBeLessThan(title.x);
  await expect(row.locator(".work-record-status")).toBeVisible();await capture(app,"19-feedback-rows");
});

test("feedback metadata and research sources use compact labeled rows",async({app})=>{
  await openWork(app,"Feedback");await app.getByRole("button",{name:"Keep my place when reopening a project",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  await expect(app.locator(".feedback-detail>.work-inline-facts")).toContainText("Project owner");
  await capture(app,"20-feedback-detail");
  const note=app.locator(".feedback-research details").first();await note.locator("summary").click();
  await note.locator(".feedback-sources").scrollIntoViewIfNeeded();await capture(app,"20-feedback-sources");
});

test("record availability places each action beside its status",async({app})=>{
  await openWork(app,"Record status");
  await expect(app.locator(".work-status-family")).toHaveCount(4);
  const family=app.locator(".work-status-family").first();await expect(family.locator("header button")).toHaveText("Open work");
  expect((await family.boundingBox()).height).toBeLessThan(140);
  await capture(app,"21-record-status");
});

test("application navigation leaves room for project lists",async({app})=>{
  expect((await app.locator(".brand").boundingBox()).height).toBeLessThan(42);
  expect((await app.locator("#find-conversation").boundingBox()).y).toBeLessThan(175);
  await expect(app.getByRole("button",{name:"Settings",exact:true})).toBeInViewport();
  await capture(app,"22-navigation");
});

test("project conversation rows retain selection in a compact list",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const selected=app.locator(".session-select[aria-current=true]");
  expect((await selected.boundingBox()).height).toBeLessThan(52);
  await expect(app.locator('.session-row[data-state=waiting] .session-state')).toContainText("Needs your attention");
  await expect(app.getByRole("button",{name:"Update the home page",exact:true})).toBeInViewport();
  await capture(app,"23-project-conversations");
});

test("conversation heading preserves controls with less vertical space",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  for(const width of [1280,760]){
    await app.setViewportSize({width,height:width===760?550:820});
    expect((await app.locator(".work-header").boundingBox()).height).toBeLessThan(110);
    await expect(app.getByRole("heading",{name:"Review the release plan",exact:true})).toBeInViewport();
    await expect(app.getByRole("button",{name:"Work plan",exact:true})).toBeInViewport();
    expect(await app.locator(".work-header").evaluate(node=>node.scrollWidth-node.clientWidth)).toBeLessThanOrEqual(1);
    await capture(app,"24-conversation-"+width);
  }
});

test("project directory is visible below a compact activity summary",async({app})=>{
  for(const width of [1280,760]){
    await app.setViewportSize({width,height:width===760?550:820});
    expect((await app.locator(".welcome-stage").boundingBox()).height).toBeLessThan(180);
    await expect(app.getByRole("button",{name:"Continue Review the release plan in Magistr",exact:true})).toBeInViewport();
    expect((await app.locator(".directory-project").first().boundingBox()).y).toBeLessThan(360);
    await capture(app,"25-project-directory-"+width);
  }
});

test("agent usage shows a compact native meter and honest unknown values",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Details",exact:true}).click();
  await app.getByRole("region",{name:"Agent settings",exact:true}).scrollIntoViewIfNeeded();
  await expect(app.getByRole("meter",{name:"Context used",exact:true})).toBeVisible();
  await expect(app.getByRole("meter",{name:"Context used",exact:true})).toHaveAttribute("aria-valuetext","18,400 of 200,000 context tokens");
  expect((await app.locator(".usage-summary").boundingBox()).height).toBeLessThan(150);
  await capture(app,"26-agent-details");
  await app.evaluate(()=>window.__fixture.publish("release",{usage:null}));
  await expect(app.getByRole("meter",{name:"Context used",exact:true})).toBeHidden();
  await expect(app.locator("#context-value")).toHaveText("Not reported");
  await app.evaluate(()=>window.__fixture.publish("release",{usage:{used:0,size:200000,cost_cents:0}}));
  await expect(app.getByRole("meter",{name:"Context used",exact:true})).toHaveAttribute("value","0");
  await expect(app.locator("#spending-value")).toHaveText("$0.00");
});

test("composer leaves reading room and expands the same draft",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const draft=app.getByLabel("Message the agent",{exact:true}),text=await draft.inputValue();
  expect((await app.locator(".composer-box").boundingBox()).height).toBeLessThan(150);
  await app.getByRole("button",{name:"Expand editor",exact:true}).click();
  await expect(app.locator("#composer")).toHaveAttribute("data-expanded","true");
  await expect(draft).toHaveValue(text);
  expect((await app.locator(".composer-heading").boundingBox()).height).toBeLessThan(70);
  expect((await draft.boundingBox()).height).toBeGreaterThan(400);
  await capture(app,"27-expanded-composer");
});

test("settings keeps compact previews and reachable controls",async({app})=>{
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  expect((await app.locator(".appearance-comparison").boundingBox()).height).toBeLessThan(155);
  await app.getByRole("button",{name:"Use light appearance",exact:true}).click();
  await expect(app.getByLabel("Appearance",{exact:true})).toHaveValue("light");
  await capture(app,"28-settings");
  await app.setViewportSize({width:760,height:550});
  await expect(app.getByRole("button",{name:"Done",exact:true})).toBeInViewport();
  await app.getByLabel("Reduce motion",{exact:true}).check();
  await expect(app.getByLabel("Reduce motion",{exact:true})).toBeChecked();
  await capture(app,"28-settings-compact");
});
