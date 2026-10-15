import {test,expect,root} from "./fixture.mjs";
import {mkdir} from "node:fs/promises";
import {resolve} from "node:path";
async function openWork(app,tab){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  if(tab)await app.getByRole("tab",{name:tab,exact:true}).click();
}
async function capture(app,name){
  const dir=resolve(root,".magistr/desktop-ia");await mkdir(dir,{recursive:true});
  await app.screenshot({path:resolve(dir,name+".png"),animations:"disabled"});
}
test("01 roadmap sections navigate complete content and retain focus",async({app})=>{
  await openWork(app,"Roadmap");
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  for(const theme of ["dark","light"]){
    await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);
    await app.getByLabel("Jump to section",{exact:true}).selectOption({label:"Requirements"});
    const heading=app.getByRole("heading",{name:"Requirements",exact:true});
    await expect(heading).toBeFocused();await expect(heading).toBeInViewport();
    await expect(app.locator(".roadmap-requirements")).toContainText("The same conversation and unsent message return after reopening.");
    await capture(app,"01-sections-"+theme);
  }
  await app.evaluate(()=>window.__fixture.publish("release",{workflow_revision:1}));
  await expect(app.getByRole("heading",{name:"Requirements",exact:true})).toBeFocused();
});
test("02 roadmap prose uses compact readable documents",async({app})=>{
  await openWork(app,"Roadmap");await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await app.setViewportSize({width:760,height:550});
  for(const theme of ["dark","light"]){
    await app.evaluate(value=>document.documentElement.dataset.theme=value,theme);
    await app.getByLabel("Jump to section",{exact:true}).selectOption({label:"Description"});
    const section=app.locator(".roadmap-document").first();
    expect((await section.boundingBox()).height).toBeLessThan(95);
    await expect(section).toContainText("Keep the last conversation available when opening a project.");
    await capture(app,"02-documents-"+theme);
  }
});