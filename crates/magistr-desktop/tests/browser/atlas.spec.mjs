import { test, expect, root } from "./fixture.mjs";
import {captureIteration} from "./iteration_evidence.mjs";
test("the project map preserves focused controls and selects the addressed conversation",async({app})=>{
  await app.getByRole("button",{name:"Map",exact:true}).click();
  const target=app.getByRole("button",{name:"Open Investigate sync failure from project map",exact:true});
  await target.focus();
  await app.evaluate(()=>window.__fixture.publish("sync",{pending:[],status:"Ready"}));
  await expect(target).toBeFocused();await expect(target).toContainText("Ready");
  await app.keyboard.press("Enter");
  await expect(app.getByRole("heading",{name:"Investigate sync failure",exact:true})).toBeVisible();
  expect(await app.evaluate(()=>window.__fixture.calls.filter(call=>call.command==="subscribe_session").at(-1).args.address.session)).toBe("sync");
});
test("the project map bounds its geometry and removes closed conversations",async({app})=>{
  await app.getByRole("button",{name:"Map",exact:true}).click();
  await app.evaluate(()=>{
    const seed=window.__fixture.frames.get("release");
    for(let i=0;i<50;i++){const frame=structuredClone(seed);frame.address.session="extra-"+i;frame.conversation.name="Conversation "+i;window.__fixture.frames.set("extra-"+i,frame);}
    window.__fixture.publish("sync",{closed:true});
  });
  await expect(app.getByRole("button",{name:"Open Investigate sync failure from project map",exact:true})).toHaveCount(0);
  expect(await app.locator(".atlas-conversation").count()).toBeLessThanOrEqual(12);
  await app.getByRole("button",{name:"Fit all",exact:true}).click();
  await app.getByRole("button",{name:"View all 51 conversations in List",exact:true}).click();
  await expect(app.locator("#welcome-projects")).toBeVisible();
  await expect(app.getByRole("button",{name:"List",exact:true})).toBeFocused();
});
test("project membership stays readable and opens resident work without admission",async({app})=>{
  await app.getByRole("button",{name:"Map",exact:true}).click();
  const surface=app.getByRole("region",{name:"Projects canvas",exact:true});
  await expect(surface.locator(".atlas-project")).toHaveCount(2);await expect(surface.locator(".atlas-conversation")).toHaveCount(3);
  await expect(surface.locator(".surface-connection[data-kind=membership]")).toHaveCount(3);
  for(const size of [{width:1280,height:820},{width:760,height:550}]){
    await app.setViewportSize(size);
    if(size.width===760){await app.getByRole("button",{name:"Actual size",exact:true}).click();await captureIteration(app,{root,name:"17-projects-760-readable",scenario:test.info().file});}
    await app.getByRole("button",{name:"Fit all",exact:true}).click();
    for(const theme of ["dark","light"]){
      await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);
      const boxes=await surface.locator(".atlas-piece").evaluateAll(nodes=>nodes.map(node=>{const b=node.getBoundingClientRect(),p=node.querySelector("strong");return{x:b.x,y:b.y,w:b.width,h:b.height,font:parseFloat(getComputedStyle(p).fontSize)*Number(node.closest(".work-surface").dataset.zoom)};}));
      expect(boxes.every(box=>box.font>=11.99)).toBe(true);
      for(let i=0;i<boxes.length;i++)for(let j=i+1;j<boxes.length;j++){const a=boxes[i],b=boxes[j];expect(a.x+a.w<=b.x||b.x+b.w<=a.x||a.y+a.h<=b.y||b.y+b.h<=a.y).toBe(true);}
      await captureIteration(app,{root,name:"17-projects-"+size.width+"-"+theme,scenario:test.info().file});
    }
  }
  await surface.getByRole("button",{name:"Open workspace for Magistr from map",exact:true}).click();
  await expect(app.getByRole("tab",{name:"Roadmap",exact:true})).toHaveAttribute("aria-selected","true");
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>["open_session","session_action"].includes(c.command)).length)).toBe(0);
});