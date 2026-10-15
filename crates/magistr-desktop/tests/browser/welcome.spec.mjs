import {test,expect} from "./fixture.mjs";

const frames=app=>app.locator("#welcome-art").evaluate(node=>Number(node.dataset.frames));
test("welcome shader stays still for reduced motion and stops when the project opens",async({app})=>{
  await expect(app.locator("#welcome-art")).toHaveAttribute("data-renderer","webgl");
  // Await browser frames, not arbitrary wall time, to observe a stable still rendering.
  const still=await frames(app);
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  expect(await frames(app)).toBe(still);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const hidden=await frames(app);
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  expect(await frames(app)).toBe(hidden);
  await expect(app.getByLabel("Message the agent",{exact:true})).toBeVisible();
});

test("the welcome entrance settles and a lost graphics context keeps project entry available",async({app})=>{
  test.setTimeout(15000);
  await app.emulateMedia({reducedMotion:"no-preference"});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Reduce motion",{exact:true}).uncheck();
  await app.getByRole("button",{name:"Done",exact:true}).click();
  const first=await frames(app);
  await expect.poll(()=>frames(app)).toBeGreaterThan(first+3);
  await expect.poll(async()=>{
    const before=await frames(app);
    await app.evaluate(()=>new Promise(resolve=>{let n=0;const tick=()=>++n===12?resolve():requestAnimationFrame(tick);requestAnimationFrame(tick);}));
    return (await frames(app))===before;
  },{timeout:6500}).toBe(true);
  expect(await frames(app)).toBeLessThan(160);
  await app.evaluate(()=>document.getElementById("welcome-art").getContext("webgl").getExtension("WEBGL_lose_context").loseContext());
  await expect(app.locator("#welcome-art")).toHaveAttribute("data-renderer","static");
  await app.getByRole("button",{name:"Choose a folder…",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Open a project",exact:true})).toBeVisible();
});
