import { test, expect } from "./fixture.mjs";
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
  expect(await app.locator(".atlas-node").count()).toBeLessThanOrEqual(12);
  await app.getByRole("button",{name:"View all 51 conversations in List",exact:true}).click();
  await expect(app.locator("#welcome-projects")).toBeVisible();
  await expect(app.getByRole("button",{name:"List",exact:true})).toBeFocused();
});
