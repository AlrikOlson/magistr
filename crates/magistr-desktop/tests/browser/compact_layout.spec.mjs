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