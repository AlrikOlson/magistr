import {test,expect} from "./fixture.mjs";
async function open(app){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await app.getByRole("tab",{name:"Roadmap",exact:true}).click();
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Restore the project conversation and draft",exact:true})).toBeVisible();
}
test("complete roadmap body, copy, selection and return focus",async({app})=>{
  await open(app);
  await expect(app.locator("#work-browser-content")).toContainText("Return to the work you left.");
  await expect(app.locator("#work-browser-content")).toContainText("Project reopening");
  await expect(app.locator(".roadmap-checklist")).toContainText("Restore the saved conversation.");
  await expect(app.locator(".roadmap-requirements")).toContainText("The same conversation and unsent message return after reopening.");
  await app.evaluate(()=>{
    Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async value=>window.__copied=value}});
    const node=document.querySelector('[data-reading="reopen:description"] p').firstChild,selection=getSelection();
    selection.setBaseAndExtent(node,0,node,4);
  });
  await app.evaluate(()=>{window.__fixture.workflow.roadmap.data.value.chunks[0].notes+=" Another note.";window.__fixture.publish("release",{workflow_revision:1});});
  await expect(app.locator("#work-browser-content")).toContainText("Another note.");
  expect(await app.evaluate(()=>getSelection().toString())).toBe("Keep");
  await app.getByRole("button",{name:"Copy description",exact:true}).click();
  expect(await app.evaluate(()=>window.__copied)).toBe("Keep the last conversation available when opening a project.");
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await expect(app.getByRole("button",{name:"Restore the project conversation and draft",exact:true})).toBeFocused();
  await expect(app.getByLabel("Track",{exact:true})).toHaveValue('"Desktop"');
  await app.keyboard.press("Escape");
  await expect(app.locator("#draft")).toHaveValue("Keep the project list visible, and let me return to the same paragraph.");
});
test("removed and unreadable roadmap items stay distinct",async({app})=>{
  await open(app);
  await app.evaluate(()=>{
    const f=window.__fixture;f.workflow.roadmap={state:"stale",data:{last:f.workflow.roadmap.data,error:{kind:"io",message:"Cannot read the roadmap."}}};
    f.publish("release",{workflow_revision:1});
  });
  await expect(app.locator(".roadmap-item-state[role=status]")).toContainText("Showing the last saved records.");
  await expect(app.getByRole("heading",{name:"Restore the project conversation and draft",exact:true})).toBeVisible();
  await app.evaluate(()=>{
    const f=window.__fixture;f.workflow.roadmap={state:"current",data:f.workflow.roadmap.data.last};
    f.workflow.roadmap.data.value.chunks=[];f.publish("release",{workflow_revision:2});
  });
  await expect(app.getByRole("heading",{name:"This item is no longer available.",exact:true})).toBeVisible();
  await app.evaluate(()=>{window.__fixture.workflow.roadmap={state:"unavailable",data:{kind:"decode",message:"Cannot read the roadmap."}};window.__fixture.publish("release",{workflow_revision:3});});
  await expect(app.getByRole("heading",{name:"This item cannot be read.",exact:true})).toBeVisible();
});
test("late item reads cannot replace the originating track after Back",async({app})=>{
  await open(app);await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await app.evaluate(()=>window.__fixture.handlers.read_roadmap_item=args=>new Promise(resolve=>window.__fixture.finishItem=()=>resolve({address:args.address,id:args.id,revision:0,item:{state:"current",data:{source:{kind:"memory"},value:null}}})));
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await expect(app.locator(".roadmap-item-state[role=status]")).toHaveText("Reading this item…");
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await app.evaluate(()=>window.__fixture.finishItem());
  await expect(app.getByRole("heading",{name:"Desktop",exact:true})).toBeVisible();
  await expect(app.getByRole("heading",{name:"This item is no longer available.",exact:true})).toHaveCount(0);
});
