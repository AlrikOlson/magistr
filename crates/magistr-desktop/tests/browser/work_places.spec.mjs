import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();}
test("work disclosures and scroll return across keyboard tabs",async({app})=>{
  await open(app);const check=app.locator(".work-check").nth(1);await check.locator("summary").click();await check.evaluate(node=>node.scrollIntoView({block:"start"}));
  const position=await app.locator("#work-browser-content").evaluate(node=>node.scrollTop);expect(position).toBeGreaterThan(100);
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).focus();await app.keyboard.press("End");await expect(app.getByRole("tab",{name:"Record status",exact:true})).toBeFocused();await app.keyboard.press("Home");
  await expect(app.getByRole("tab",{name:"Current and saved work",exact:true})).toBeFocused();await expect(check).toHaveAttribute("open","");
  expect(await app.locator("#work-browser-content").evaluate(node=>node.scrollTop)).toBe(position);
});
test("roadmap selection and dependency trail survive tabs and ignore late suspended reads",async({app})=>{
  await open(app);await app.getByRole("tab",{name:"Roadmap",exact:true}).click();await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();await app.getByRole("button",{name:"Store conversations and drafts",exact:true}).click();
  await app.getByRole("tab",{name:"Reasoning",exact:true}).click();await app.getByRole("tab",{name:"Roadmap",exact:true}).click();await expect(app.getByRole("heading",{name:"Store conversations and drafts",exact:true})).toBeVisible();
  await app.getByRole("button",{name:"Back to previous item",exact:true}).click();await expect(app.getByRole("heading",{name:"Restore the project conversation and draft",exact:true})).toBeVisible();
  await app.evaluate(()=>window.__fixture.handlers.read_roadmap_item=args=>new Promise(resolve=>window.__fixture.finishItem=()=>resolve({address:args.address,id:args.id,revision:0,item:{state:"current",data:{source:{kind:"memory"},value:null}}})));
  await app.getByRole("button",{name:"Refresh",exact:true}).click();await expect(app.locator(".roadmap-item-state[role=status]")).toHaveText("Reading this item…");
  await app.getByRole("tab",{name:"Feedback",exact:true}).click();await app.evaluate(()=>window.__fixture.finishItem());await expect(app.locator("#work-browser-content")).toContainText("Keep my place when reopening a project");await expect(app.locator(".roadmap-item-detail")).toHaveCount(0);
  await app.evaluate(()=>{delete window.__fixture.handlers.read_roadmap_item;window.__fixture.workflow.roadmap.data.value.chunks=[];});
  await app.getByRole("tab",{name:"Roadmap",exact:true}).click();await expect(app.getByRole("heading",{name:"This item is no longer available.",exact:true})).toBeVisible();
});
test("unchanged text selection returns but changed text is never substituted",async({app})=>{
  await open(app);await app.getByRole("tab",{name:"Reasoning",exact:true}).click();await app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();
  const selection=await app.evaluate(()=>{const block=document.querySelector(".reasoning-step [data-reading]"),node=block.firstChild;getSelection().setBaseAndExtent(node,0,node,8);return getSelection().toString();});
  await app.getByRole("tab",{name:"Feedback",exact:true}).click();await app.getByRole("tab",{name:"Reasoning",exact:true}).click();expect(await app.evaluate(()=>getSelection().toString())).toBe(selection);
  await app.getByRole("tab",{name:"Feedback",exact:true}).click();await app.evaluate(()=>{const f=window.__fixture;f.workflow.think.data.value.sessions[0].steps[0].context="Changed context.";f.publish("release",{workflow_revision:1});});
  await app.getByRole("tab",{name:"Reasoning",exact:true}).click();expect(await app.evaluate(()=>getSelection().toString())).toBe("");await expect(app.locator(".reasoning-step")).toContainText("Changed context.");
  await app.keyboard.press("Escape");await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Roadmap",exact:true}).click();await expect(app.getByRole("heading",{name:"Desktop",exact:true})).toBeVisible();
});
test("refresh failures remain explicit across tabs until a successful read",async({app})=>{
  await open(app);await app.evaluate(()=>window.__fixture.handlers.read_workflow=()=>{throw Error("Connection interrupted");});
  await app.getByRole("button",{name:"Refresh",exact:true}).click();await expect(app.locator("#work-browser-state")).toContainText("Showing the previous records.");
  await app.getByRole("tab",{name:"Record status",exact:true}).click();await expect(app.locator("#work-browser-state")).toContainText("Connection interrupted");await expect(app.locator("#work-browser-state")).toHaveAttribute("data-current","false");
  await app.evaluate(()=>delete window.__fixture.handlers.read_workflow);await app.getByRole("button",{name:"Refresh",exact:true}).click();await expect(app.locator("#work-browser-state")).toHaveText("All four record groups were read successfully.");await expect(app.locator("#work-browser-state")).toHaveAttribute("data-current","true");
});
