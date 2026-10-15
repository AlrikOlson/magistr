import {test,expect,root} from "./fixture.mjs";
import {captureIteration} from "./iteration_evidence.mjs";
async function open(app,area="Roadmap"){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("tab",{name:area,exact:true}).click();
}
async function capture(app,name){await captureIteration(app,{root,name,scenario:test.info().file});}
test("work canvas keeps dependency context while inspecting and supports zoom, pan and arrangement",async({app})=>{
  await open(app);const surface=app.getByRole("region",{name:"Roadmap canvas",exact:true}),piece=surface.getByRole("button",{name:"Restore the project conversation and draft",exact:true});
  await expect(piece).toBeVisible();await expect(surface.locator(".surface-connection")).toHaveCount(2);
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await capture(app,"01-roadmap-"+theme);}
  const before=Number(await app.locator("#work-browser .work-surface").getAttribute("data-zoom"));
  await app.getByRole("button",{name:"Zoom in",exact:true}).click();expect(Number(await app.locator("#work-browser .work-surface").getAttribute("data-zoom"))).toBeGreaterThan(before);
  await app.getByRole("button",{name:"Fit all",exact:true}).click();await piece.click();
  const inspector=app.locator("#work-browser-inspector");await expect(inspector).toBeVisible();await expect(inspector.getByRole("heading",{name:"Restore the project conversation and draft",exact:true})).toBeFocused();
  await expect(surface).toBeVisible();await capture(app,"01-inspector-light");
  await inspector.getByRole("button",{name:"Store conversations and drafts",exact:true}).click();await expect(inspector.getByRole("heading",{name:"Store conversations and drafts",exact:true})).toBeFocused();
  await inspector.getByRole("button",{name:"Back to previous item",exact:true}).click();await inspector.getByRole("button",{name:"Back to track",exact:true}).click();await expect(piece).toBeFocused();
  const world=surface.locator(".surface-world"),position=await world.evaluate(n=>n.style.transform);
  await surface.focus();await surface.press("ArrowRight");expect(await world.evaluate(n=>n.style.transform)).not.toBe(position);
  await app.getByRole("button",{name:"Fit all",exact:true}).click();
  await surface.getByRole("button",{name:"Move Restore the project conversation and draft",exact:true}).click();
  const node=surface.locator('[data-node="reopen"]'),left=await node.evaluate(n=>parseFloat(n.style.left));
  await app.getByRole("button",{name:"Move piece right",exact:true}).click();expect(await node.evaluate(n=>parseFloat(n.style.left))).toBe(left+48);
  await app.getByRole("button",{name:"Done arranging",exact:true}).click();
  await app.evaluate(()=>window.__fixture.publish("release",{workflow_revision:1}));await expect(node).toHaveCSS("left",(left+48)+"px");
  await app.setViewportSize({width:760,height:550});await app.getByRole("button",{name:"Fit all",exact:true}).click();
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await capture(app,"01-roadmap-compact-"+theme);}
  await piece.click();await expect(inspector).toBeVisible();await capture(app,"01-inspector-compact");await inspector.getByRole("button",{name:"Back to track",exact:true}).click();await expect(piece).toBeFocused();
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>["session_action","control_work","open_session"].includes(c.command)).length)).toBe(0);
});
test("reasoning threads keep the investigation visible beside complete evidence",async({app})=>{
  await open(app,"Reasoning");const surface=app.getByRole("region",{name:"Reasoning canvas",exact:true}),inspector=app.locator("#work-browser-inspector");
  await expect(surface.locator(".decision-piece")).toHaveCount(2);await expect(surface.locator(".surface-connection")).toHaveCount(2);
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await capture(app,"02-reasoning-"+theme);}
  const decision=surface.getByRole("button",{name:"Keep each conversation with its saved message",exact:true});await decision.click();
  await expect(inspector.getByRole("heading",{name:"Keep each conversation with its saved message",exact:true})).toBeFocused();await expect(surface).toBeVisible();
  await app.evaluate(()=>Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async text=>window.__copied=text}}));
  await inspector.getByRole("button",{name:"Copy reasoning",exact:true}).click();expect(await app.evaluate(()=>window.__copied)).toBe("The conversation already owns its saved message. Read that record before the editor opens.");
  await inspector.evaluate(node=>node.scrollTop=0);
  await expect.poll(async()=>{const view=await surface.boundingBox(),pieces=await surface.locator(".decision-piece").all();return Promise.all(pieces.map(async piece=>{const box=await piece.boundingBox();return box.x>=view.x&&box.x+box.width<=view.x+view.width;}));}).toEqual([true,true]);
  await capture(app,"02-evidence-light");
  await inspector.getByRole("region",{name:"Related reasoning",exact:true}).getByRole("button",{name:"Inspect the installation result",exact:true}).click();
  await expect(inspector.getByRole("heading",{name:"Inspect the installation result",exact:true})).toBeFocused();
  await inspector.getByRole("button",{name:"Back to previous step",exact:true}).click();await inspector.getByRole("button",{name:"Back to reasoning",exact:true}).click();await expect(decision).toBeFocused();
  await app.setViewportSize({width:760,height:550});await app.getByRole("button",{name:"Fit all",exact:true}).click();
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await capture(app,"02-reasoning-compact-"+theme);}
  await app.getByLabel("Investigation",{exact:true}).selectOption("branch:storage");await expect(surface.locator(".surface-connection")).toHaveCount(0);
  await surface.getByRole("button",{name:"Origin · 41",exact:true}).click();await expect(inspector.getByRole("heading",{name:"Keep each conversation with its saved message",exact:true})).toBeFocused();
});
test("reasoning pagination never connects a partial investigation to its conclusion",async({app})=>{
  await open(app,"Reasoning");
  await app.evaluate(()=>{const f=window.__fixture,h=f.workflow.think.data.value.sessions[0],base=h.steps[0];h.steps=Array.from({length:62},(_,i)=>({...base,record_id:"paged-"+i,step_number:i+1,purpose:"Finding "+i,pinned:false,branch_id:i>1?"large":null}));h.branches=[{id:"large",name:"Follow the complete investigation",from_step:1,merged_into:2,status:"merged"}];f.publish("release",{workflow_revision:1});});
  const surface=app.getByRole("region",{name:"Reasoning canvas",exact:true});await expect(surface.locator(".decision-piece")).toHaveCount(50);
  await expect(surface.getByRole("button",{name:"Follow investigation: Follow the complete investigation",exact:true})).toHaveCount(1);
  await expect(surface.getByRole("button",{name:"Follow conclusion: Follow the complete investigation",exact:true})).toHaveCount(0);
  await app.getByRole("button",{name:"Next page",exact:true}).click();await expect(surface.locator(".decision-piece")).toHaveCount(12);await expect(surface.locator(".surface-connection")).toHaveCount(0);
});
test("project destinations keep the draft and camera in place across the shell",async({app})=>{
  await app.locator(".directory-project").first().getByRole("button",{name:"Open workspace",exact:true}).click();
  const roadmap=app.getByRole("tab",{name:"Roadmap",exact:true});await expect(roadmap).toHaveAttribute("aria-selected","true");
  const canvas=app.getByRole("region",{name:"Roadmap canvas",exact:true});await expect(canvas).toBeVisible();
  await app.getByRole("button",{name:"Actual size",exact:true}).click();const camera=await canvas.locator(".surface-world").evaluate(n=>n.style.transform);
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await capture(app,"03-shell-roadmap-"+theme);}
  await app.getByRole("tab",{name:"Conversation",exact:true}).click();await expect(app.locator("#draft")).toHaveValue("Keep the project list visible, and let me return to the same paragraph.");
  await expect(app.locator("#project-name")).toBeVisible();await expect(app.locator("#project-name")).toHaveText("Magistr");await capture(app,"03-shell-conversation");await roadmap.click();expect(await canvas.locator(".surface-world").evaluate(n=>n.style.transform)).toBe(camera);
  await app.setViewportSize({width:760,height:550});await app.getByRole("button",{name:"Fit all",exact:true}).click();
  expect((await app.getByRole("region",{name:"Roadmap",exact:true}).boundingBox()).height).toBeGreaterThan(300);
  expect((await canvas.boundingBox()).height).toBeGreaterThan(240);
  for(const theme of ["dark","light"]){await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);await capture(app,"03-shell-compact-"+theme);}
  await app.getByRole("tab",{name:"Reasoning",exact:true}).click();await capture(app,"03-reasoning-compact");
  await app.getByRole("tab",{name:"Conversation",exact:true}).click();await expect(app.locator("#draft")).toHaveValue("Keep the project list visible, and let me return to the same paragraph.");
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>["open_session","session_action"].includes(c.command)).length)).toBe(0);
});