import {test,expect,root} from "./fixture.mjs";
import {captureIteration} from "./iteration_evidence.mjs";
async function open(app,area){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("tab",{name:area,exact:true}).click();}
async function capture(app,name){await captureIteration(app,{root,name,scenario:test.info().file});}
async function inspectRoutes(app,expected){
  const geometry=await app.locator("#work-browser .surface-world").evaluate(world=>({
    boxes:[...world.querySelectorAll(".surface-node,.thread-heading")].map(n=>({id:n.dataset.node,x:n.offsetLeft,y:n.offsetTop,w:n.offsetWidth,h:n.offsetHeight})),
    routes:[...world.querySelectorAll(".surface-connection")].map(n=>({points:JSON.parse(n.dataset.route??"[]"),edge:n.dataset.edge,hidden:n.hidden})),
    labels:[...world.querySelectorAll(".surface-connection")].filter(n=>getComputedStyle(n).opacity==="1").map(n=>{const r=n.getBoundingClientRect();return {x:r.x,y:r.y,w:r.width,h:r.height};}),
    pieces:[...world.querySelectorAll(".surface-node,.thread-heading")].map(n=>{const r=n.getBoundingClientRect();return {x:r.x,y:r.y,w:r.width,h:r.height};})
  }));
  expect(geometry.routes).toHaveLength(expected);
  const overlaps=(a,b)=>a.x<b.x+b.w-.5&&a.x+a.w>b.x+.5&&a.y<b.y+b.h-.5&&a.y+a.h>b.y+.5;
  for(const route of geometry.routes){
    expect(route.hidden,route.edge+" has a route").toBe(false);expect(route.points.length).toBeGreaterThanOrEqual(2);
    for(let i=1;i<route.points.length;i++){
      const a=route.points[i-1],b=route.points[i];expect(a.x===b.x||a.y===b.y,route.edge+" is orthogonal").toBe(true);
      for(const box of geometry.boxes){
        const crosses=a.x===b.x?a.x>box.x+.1&&a.x<box.x+box.w-.1&&Math.max(a.y,b.y)>box.y+.1&&Math.min(a.y,b.y)<box.y+box.h-.1:
          a.y>box.y+.1&&a.y<box.y+box.h-.1&&Math.max(a.x,b.x)>box.x+.1&&Math.min(a.x,b.x)<box.x+box.w-.1;
        expect(crosses,route.edge+" crosses "+(box.id??"a heading")).toBe(false);
      }
    }
    for(const p of [route.points[0],route.points.at(-1)])expect(geometry.boxes.some(b=>b.id&&((p.x===b.x||p.x===b.x+b.w)&&p.y>b.y&&p.y<b.y+b.h||(p.y===b.y||p.y===b.y+b.h)&&p.x>b.x&&p.x<b.x+b.w)),"endpoint attaches to an item edge").toBe(true);
  }
  geometry.labels.forEach((label,i)=>{for(const piece of geometry.pieces)expect(overlaps(label,piece),"connection label overlaps content").toBe(false);for(const other of geometry.labels.slice(i+1))expect(overlaps(label,other),"connection labels overlap").toBe(false);});
}
test("default compact roadmap is readable and prerequisites align with their consumers",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.setViewportSize({width:760,height:550});await app.getByRole("tab",{name:"Roadmap",exact:true}).click();await expect(app.locator(".work-piece")).toHaveCount(5);
  const values=await app.locator("#work-browser .work-surface").evaluate(root=>{const zoom=Number(root.dataset.zoom),nodes=[...root.querySelectorAll(".surface-node")];return {zoom,font:parseFloat(getComputedStyle(root.querySelector(".work-piece-name strong")).fontSize)*zoom,rows:Object.fromEntries(nodes.map(n=>[n.dataset.node,n.offsetTop]))};});
  expect(values.font).toBeGreaterThanOrEqual(12);expect(values.rows.foundation).toBe(values.rows.reopen);expect(values.rows.signing).toBe(values.rows.packaging);
  await inspectRoutes(app,2);
  for(const theme of ["dark","light"]){await app.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);await capture(app,"04-default-roadmap-compact-"+theme);}
});
test("shared prerequisites and manual obstruction keep routes clear",async({app})=>{
  await app.evaluate(()=>{
    const f=window.__fixture,r=f.workflow.roadmap,base=r.data.value.chunks[0];
    const specs=[["source","Save project data",[],"done"],["draft","Restore the unsent message",["source"],"done"],["files","Verify attached files",[],"done"],["reopen","Reopen the conversation",["source","draft"],"pending"],["installer","Build the installer",["source","files"],"pending"],["verify","Check a clean installation",["reopen","installer"],"pending"],["release","Publish the release",["verify"],"blocked"]];
    r.data.value.chunks=specs.map(([id,title,deps,status])=>({...base,id,name:title,title,deps,status,group:"Desktop"}));
    const frontier=f.roadmap.data.value.frontiers[0];frontier.items=r.data.value.chunks.filter(n=>n.status!=="done").map(n=>({id:n.id,status:n.status,readiness:{state:n.id==="reopen"?"ready":"waiting"}}));frontier.activity.next_ready="reopen";
    f.publish("release",{workflow_revision:3});
  });
  await open(app,"Roadmap");
  await expect(app.locator(".work-piece")).toHaveCount(7);
  const canvas=await app.locator("#work-browser .surface-viewport").boundingBox(),next=await app.locator('.work-piece[data-next="true"]').boundingBox();expect(next.x).toBeGreaterThanOrEqual(canvas.x);expect(next.x+next.width).toBeLessThanOrEqual(canvas.x+canvas.width);await inspectRoutes(app,8);await capture(app,"04-dense-roadmap-default");
  await app.getByRole("button",{name:"Fit all",exact:true}).click();await capture(app,"04-dense-roadmap-overview");
  const grip=app.getByRole("button",{name:"Move Verify attached files",exact:true});await grip.click();
  for(let i=0;i<3;i++)await app.getByRole("button",{name:"Move piece right",exact:true}).click();
  await app.getByRole("button",{name:"Done arranging",exact:true}).click();await inspectRoutes(app,8);await capture(app,"04-dense-roadmap-arranged");
});
test("nested investigations and return paths stay separate from decision content",async({app})=>{
  await app.evaluate(()=>{
    const f=window.__fixture,h=f.workflow.think.data.value.sessions[0],base=h.steps[0];
    h.steps=[
      {...base,record_id:"s41",step_number:41,purpose:"Locate the saved conversation"},
      {...base,record_id:"s44",step_number:44,purpose:"Restore the existing conversation",dependencies:[{step:41,relation:"supports"}]},
      {...base,record_id:"s43",step_number:43,branch_id:"storage",purpose:"Confirm who saves the draft"},
      {...base,record_id:"s45",step_number:45,branch_id:"failure",purpose:"Reproduce an interrupted save"},
      {...base,record_id:"s46",step_number:46,branch_id:"failure",purpose:"Verify a safe retry"},
      {...base,record_id:"s47",step_number:47,branch_id:"detail",purpose:"Check the acknowledgement order"}
    ];
    h.branches=[{id:"storage",name:"Who owns the saved draft?",from_step:41,status:"merged",merged_into:44},{id:"failure",name:"What happens if saving stops?",from_step:41,status:"merged",merged_into:44},{id:"detail",name:"Can an old acknowledgement replace newer text?",from_step:43,status:"merged",merged_into:46}];
    f.publish("release",{workflow_revision:2});
  });
  await open(app,"Reasoning");
  await expect(app.locator(".decision-piece")).toHaveCount(6);await inspectRoutes(app,7);
  const sameColumn=JSON.parse(await app.locator('[data-edge="s44:s41"]').getAttribute("data-route"));expect(sameColumn).toHaveLength(2);expect(sameColumn[0].x).toBe(sameColumn[1].x);await capture(app,"04-nested-reasoning-default");
  await app.getByRole("button",{name:"Fit all",exact:true}).click();await inspectRoutes(app,7);
  for(const theme of ["dark","light"]){await app.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);await capture(app,"04-nested-reasoning-"+theme);}
  await app.setViewportSize({width:760,height:550});await app.getByRole("button",{name:"Fit all",exact:true}).click();await inspectRoutes(app,7);await capture(app,"04-nested-reasoning-compact");
  await app.getByRole("button",{name:"Restore the existing conversation",exact:true}).click();await expect(app.locator("#work-browser-inspector")).toBeVisible();await capture(app,"04-nested-reasoning-selected");
});
test("large prerequisite sets expose an explicit bundle and keep complete details reachable",async({app})=>{
  await app.evaluate(()=>{
    const f=window.__fixture,r=f.workflow.roadmap,base=r.data.value.chunks[0],ids=Array.from({length:500},(_,i)=>"required-"+i);
    r.data.value.chunks=[{...base,id:"many",title:"Validate the full delivery",deps:ids},...ids.map((id,i)=>({...base,id,title:"Required check "+(i+1),deps:[],status:"done"}))];
    f.roadmap.data.value.frontiers[0].items=[{id:"many",status:"pending",readiness:{state:"ready"}}];f.roadmap.data.value.frontiers[0].activity.next_ready="many";
  });
  await open(app,"Roadmap");await expect(app.locator("#work-browser .surface-node")).toHaveCount(2);await expect(app.locator(".dependency-bundle")).toContainText("500");
  await app.getByRole("button",{name:"Fit all",exact:true}).click();await inspectRoutes(app,1);await capture(app,"04-prerequisite-bundle");
  await app.getByRole("button",{name:"View 500 prerequisites for Validate the full delivery",exact:true}).click();
  await expect(app.getByRole("region",{name:"Required work",exact:true})).toContainText("Required check 500");
});
test("pointer arrangement cancels cleanly and preserves the camera across hidden views",async({app})=>{
  await open(app,"Roadmap");const view=app.locator("#work-browser .surface-viewport"),world=app.locator("#work-browser .surface-world");
  const grip=app.getByRole("button",{name:"Move Restore the project conversation and draft",exact:true}),box=await grip.boundingBox(),node=app.locator('[data-node="reopen"]'),initial=await node.evaluate(n=>n.style.left);
  await app.mouse.move(box.x+box.width/2,box.y+box.height/2);await app.mouse.down();await app.mouse.move(box.x+70,box.y+20,{steps:4});
  expect(await node.evaluate(n=>n.style.left)).not.toBe(initial);await app.keyboard.press("Escape");await app.mouse.up();expect(await node.evaluate(n=>n.style.left)).toBe(initial);await expect(app.locator("#work-browser")).toBeVisible();
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();const before=await world.evaluate(n=>n.style.transform);
  await app.getByRole("tab",{name:"Conversation",exact:true}).click();await app.getByRole("tab",{name:"Roadmap",exact:true}).click();expect(await world.evaluate(n=>n.style.transform)).toBe(before);
  await expect(view).toBeVisible();
});