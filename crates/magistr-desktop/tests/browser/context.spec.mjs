import { test, expect } from "./fixture.mjs";

test("rename stays pending until acknowledged, keeps failed text and uses the captured conversation",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const draft=await app.getByLabel("Message the agent",{exact:true}).inputValue();
  await app.getByLabel("Conversation actions",{exact:true}).click();
  await app.getByRole("button",{name:"Rename conversation",exact:true}).click();
  const name=app.getByLabel("Conversation name",{exact:true});
  await name.fill("Prepare the desktop release");
  await app.evaluate(()=>window.__fixture.handlers.session_action=()=>new Promise((resolve,reject)=>window.finishRename={resolve,reject}));
  await app.getByRole("button",{name:"Save name",exact:true}).click();
  await expect(app.getByText("Saving name…",{exact:true})).toBeVisible();
  await expect(app.getByRole("button",{name:"Save name",exact:true})).toBeDisabled();
  await app.keyboard.press("Escape");
  await expect(app.getByRole("dialog",{name:"Rename conversation",exact:true})).toBeVisible();
  await app.evaluate(()=>window.finishRename.reject(new Error("The name could not be saved. Try again.")));
  await expect(name).toHaveValue("Prepare the desktop release");
  await expect(name).toBeFocused();
  await app.evaluate(()=>window.__fixture.handlers.session_action=args=>{
    const f=window.__fixture.frames.get("release");
    window.__fixture.publish("release",{conversation:{...f.conversation,name:args.action.name}});
  });
  await app.getByRole("button",{name:"Save name",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Prepare the desktop release",exact:true})).toBeVisible();
  await expect(app.getByLabel("Conversation actions",{exact:true})).toBeFocused();
  await expect(app.getByLabel("Message the agent",{exact:true})).toHaveValue(draft);
  const calls=await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="session_action"));
  expect(calls.map(c=>c.args.target.conversation)).toEqual(["release-conversation","release-conversation"]);
});

test("work state and waiting requests open their exact Details section",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.evaluate(()=>window.__fixture.publish("release",{
    work:{id:{project:"magistr",local:"run"},phase:{kind:"running"},iteration:2,limit:10,requested:"pause"}
  }));
  const summary=app.getByRole("button",{name:"Finishing this item, then pausing",exact:true});
  await expect(summary).toBeVisible();
  await summary.click();
  await expect(app.getByRole("heading",{name:"Planned work",exact:true})).toBeFocused();
  await expect(app.getByRole("button",{name:"Pause after this item",exact:true})).toBeDisabled();
  await app.getByRole("button",{name:"Close Details",exact:true}).click();
  await app.evaluate(()=>window.__fixture.publish("release",{work:{id:{project:"magistr",local:"run"},phase:{kind:"paused"},iteration:2,limit:10,requested:null}}));
  await expect(app.getByRole("button",{name:"Work paused",exact:true})).toBeVisible();
  await app.getByRole("button",{name:"Investigate sync failure",exact:true}).click();
  await app.getByRole("button",{name:"1 request needs your answer →",exact:true}).click();
  await expect(app.getByLabel("Request: Run the focused sync checks",{exact:true})).toBeFocused();
  await expect(app.getByRole("button",{name:"Allow once",exact:true})).toBeVisible();
});

test("a new conversation observation cannot redirect an open rename",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByLabel("Conversation actions",{exact:true}).click();
  await app.getByRole("button",{name:"Rename conversation",exact:true}).click();
  await app.getByLabel("Conversation name",{exact:true}).fill("My pending name");
  await app.evaluate(()=>{
    const f=window.__fixture.frames.get("release");
    window.__fixture.publish("release",{target:{...f.target,conversation:"replacement"},conversation:{...f.conversation,id:"replacement",name:"Another conversation"}});
  });
  await expect(app.getByRole("button",{name:"Save name",exact:true})).toBeDisabled();
  await expect(app.getByLabel("Conversation name",{exact:true})).toHaveValue("My pending name");
  await expect(app.getByText("This conversation changed. Close this dialog and try again.",{exact:true})).toBeVisible();
});
