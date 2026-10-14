import { test, expect } from "./fixture.mjs";

test("project groups preserve selection and addressed background actions",async({app})=>{
  const projects=app.getByRole("navigation",{name:"Projects",exact:true});
  await expect(projects.locator(".project-group")).toHaveCount(2);
  const magistr=projects.locator(".project-group").filter({has:app.locator(".project-title",{hasText:"Magistr"})});
  await expect(magistr.locator(".session-row")).toHaveCount(2);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const draft=app.getByLabel("Message the agent",{exact:true});
  await draft.fill("Keep this draft while stopping the other reply.");
  await app.evaluate(()=>{
    const frame=window.__fixture.frames.get("sync");
    window.__fixture.publish("sync",{status:"Working",pending:[],activity:{...frame.activity,active:true}});
  });
  const more=app.getByLabel("Actions for Investigate sync failure",{exact:true});
  await more.click();
  await expect(app.getByRole("button",{name:"Stop Investigate sync failure",exact:true})).toBeVisible();
  await app.evaluate(()=>window.__fixture.publish("sync",{status:"Writing"}));
  await expect(more).toBeFocused();
  await app.getByRole("button",{name:"Stop Investigate sync failure",exact:true}).click();
  const calls=await app.evaluate(()=>window.__fixture.calls.filter(call=>call.command==="session_action"));
  expect(calls.at(-1).args).toMatchObject({target:{address:{session:"sync"},conversation:"sync-conversation"},action:{action:"cancel"}});
  await expect(draft).toHaveValue("Keep this draft while stopping the other reply.");
  await expect(app.getByRole("button",{name:"Review the release plan",exact:true})).toHaveAttribute("aria-current","true");
  await more.click();await app.keyboard.press("Escape");await expect(more).toBeFocused();
  await expect(app.getByRole("button",{name:"Close Investigate sync failure",exact:true})).toBeHidden();
  await magistr.locator(".project-heading").click();
  await app.evaluate(()=>window.__fixture.publish("sync",{status:"Ready"}));
  await expect(magistr).not.toHaveAttribute("open","");
});

test("Settings returns focus and keeps the conversation draft",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const draft=app.getByLabel("Message the agent",{exact:true}),saved=await draft.inputValue();
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await expect(app.getByRole("dialog",{name:"Settings",exact:true})).toBeVisible();
  await app.keyboard.press("Control+o");
  await expect(app.locator("dialog[open]")).toHaveCount(1);
  await app.keyboard.press("Escape");
  await expect(app.getByRole("button",{name:"Settings",exact:true})).toBeFocused();
  await expect(draft).toHaveValue(saved);
  await app.getByRole("button",{name:"Agent settings",exact:true}).click();
  await expect(app.getByLabel("Model",{exact:true})).toBeFocused();
});
