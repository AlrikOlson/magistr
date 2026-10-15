import {test,expect} from "./fixture.mjs";
test("surface entrances settle, honor effective reduced motion and cancel on preference changes",async({app})=>{
  await app.emulateMedia({reducedMotion:"no-preference"});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Reduce motion",{exact:true}).uncheck();
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await expect(app.locator("html")).toHaveAttribute("data-reduced-motion","false");
  await app.evaluate(()=>{
    window.__motionCalls=[];const animate=Element.prototype.animate;
    Element.prototype.animate=function(frames,options){window.__motionCalls.push({id:this.id,...options});return animate.call(this,frames,options);};
  });
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  expect(await app.evaluate(()=>window.__motionCalls)).toEqual([{id:"settings",duration:160,easing:"cubic-bezier(.2,.7,.2,1)",iterations:1}]);
  await expect.poll(()=>app.evaluate(()=>document.getAnimations().length)).toBe(0);
  await app.getByRole("button",{name:"Done",exact:true}).click();
  const cancelled=await app.evaluate(async()=>{
    document.getElementById("settings").showModal();
    await new Promise(resolve=>requestAnimationFrame(resolve));
    const before=document.getAnimations().length;
    document.documentElement.dataset.reducedMotion="true";
    await new Promise(resolve=>requestAnimationFrame(resolve));
    return {before,after:document.getAnimations().length};
  });
  expect(cancelled).toEqual({before:1,after:0});
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.emulateMedia({reducedMotion:"reduce"});
  await expect(app.locator("html")).toHaveAttribute("data-reduced-motion","true");
  const before=await app.evaluate(()=>window.__motionCalls.length);
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  expect(await app.evaluate(()=>window.__motionCalls.length)).toBe(before);
  await app.getByLabel("Reduce motion",{exact:true}).check();
  await app.emulateMedia({reducedMotion:"no-preference"});
  await expect(app.locator("html")).toHaveAttribute("data-reduced-motion","true");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  expect(await app.evaluate(()=>window.__motionCalls.length)).toBe(before);
});
