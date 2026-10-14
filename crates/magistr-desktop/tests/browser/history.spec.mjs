import { test, expect } from "./fixture.mjs";

test("search keeps the selected draft and fixed actions reachable in a short window",async({app})=>{
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Release notes and rollback",exact:true})).toBeVisible();
  const draft=app.getByLabel("Saved message",{exact:true});
  await draft.fill("Keep the rollback instructions with this release.");
  const search=app.getByLabel("Search saved conversations",{exact:true});
  await search.fill("website");
  await expect(app.locator("#history-list button")).toHaveCount(4);
  await expect(draft).toHaveValue("Keep the rollback instructions with this release.");
  await search.fill("not a matching project");
  await expect(app.getByText("No conversations match your search.",{exact:true})).toBeVisible();
  await expect(draft).toHaveValue("Keep the rollback instructions with this release.");
  await search.fill("");
  await app.setViewportSize({width:800,height:560});
  await app.locator("#history-list").evaluate(node=>node.scrollTop=node.scrollHeight);
  await app.locator("#history-reading").evaluate(node=>node.scrollTop=node.scrollHeight);
  for(const name of ["Close saved conversations","Resume conversation"]){
    const box=await app.getByRole("button",{name,exact:true}).boundingBox();
    expect(box.y).toBeGreaterThanOrEqual(0);expect(box.y+box.height).toBeLessThanOrEqual(560);
  }
  await app.getByRole("button",{name:"Close saved conversations",exact:true}).click();
  await expect(app.getByRole("button",{name:"Saved conversations",exact:true})).toBeFocused();
  expect(await app.evaluate(()=>window.__fixture.saved.get("release-notes").text)).toBe("Keep the rollback instructions with this release.");
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="open_session"))).toHaveLength(0);
});

test("failed save blocks navigation and closing without losing text",async({app})=>{
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByLabel("Saved message",{exact:true})).toBeEnabled();
  await app.evaluate(()=>window.__fixture.handlers.save_saved_draft=()=>{throw new Error("The disk is full.");});
  await app.getByLabel("Saved message",{exact:true}).fill("This edit must stay available.");
  await expect(app.locator("#saved-draft-status")).toHaveText("The disk is full.");
  await app.getByRole("button",{name:"Review the sync retry",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Release notes and rollback",exact:true})).toBeVisible();
  await expect(app.getByLabel("Saved message",{exact:true})).toHaveValue("This edit must stay available.");
  await app.getByRole("button",{name:"Close saved conversations",exact:true}).click();
  await expect(app.getByRole("dialog",{name:"Saved conversations",exact:true})).toBeVisible();
  await app.evaluate(()=>delete window.__fixture.handlers.save_saved_draft);
  await app.getByRole("button",{name:"Retry save",exact:true}).click();
  await expect(app.locator("#saved-draft-status")).toHaveText("Draft saved");
  await app.getByRole("button",{name:"Review the sync retry",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Review the sync retry",exact:true})).toBeVisible();
});

test("loading can close safely, and load failure is distinct from empty history",async({app})=>{
  await app.evaluate(()=>window.__fixture.handlers.saved_conversations=()=>new Promise(resolve=>window.finishHistory=resolve));
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByText("Reading saved conversations…",{exact:true})).toBeVisible();
  await app.getByRole("button",{name:"Close saved conversations",exact:true}).click();
  await app.evaluate(()=>{window.finishHistory(window.__fixture.history);window.__fixture.handlers.saved_conversations=()=>{throw new Error("Saved conversations could not be read.");};});
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByText("Could not read saved conversations.",{exact:true})).toBeVisible();
  await expect(app.getByText("No saved conversations yet.",{exact:true})).toHaveCount(0);
  await app.evaluate(()=>window.__fixture.handlers.saved_conversations=()=>[]);
  await app.getByRole("button",{name:"Refresh",exact:true}).click();
  await expect(app.getByText("No saved conversations yet.",{exact:true})).toBeVisible();
});

test("a refused resume stays in the browser with its saved message and retry",async({app})=>{
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByRole("button",{name:"Resume conversation",exact:true})).toBeEnabled();
  const draft=await app.getByLabel("Saved message",{exact:true}).inputValue();
  await app.evaluate(()=>window.__fixture.handlers.open_session=()=>new Promise((resolve,reject)=>window.finishResume={resolve,reject}));
  await app.getByRole("button",{name:"Resume conversation",exact:true}).click();
  await expect(app.getByRole("button",{name:"Opening conversation…",exact:true})).toBeDisabled();
  await expect(app.getByRole("button",{name:"Close saved conversations",exact:true})).toBeDisabled();
  await app.evaluate(()=>window.finishResume.reject(new Error("The agent could not reopen this conversation.")));
  await expect(app.locator("#history-error")).toHaveText("The agent could not reopen this conversation.");
  await expect(app.locator("#history-error")).toBeFocused();
  await expect(app.getByLabel("Saved message",{exact:true})).toHaveValue(draft);
  await expect(app.getByRole("button",{name:"Resume conversation",exact:true})).toBeEnabled();
});

test("resuming an already open conversation returns to its owner without starting an agent",async({app})=>{
  await app.evaluate(()=>{
    const fixture=window.__fixture,f=fixture.frames.get("release");
    fixture.history[0]={...fixture.history[0],id:f.conversation.id,name:f.conversation.name};
    fixture.saved.set(f.conversation.id,{text:f.draft,revision:f.draft_revision});
  });
  await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
  await expect(app.getByLabel("Saved message",{exact:true})).toBeDisabled();
  await expect(app.getByRole("button",{name:"Resume conversation",exact:true})).toBeEnabled();
  await app.getByRole("button",{name:"Resume conversation",exact:true}).click();
  await expect(app.getByRole("dialog",{name:"Saved conversations",exact:true})).toBeHidden();
  await expect(app.getByRole("heading",{name:"Review the release plan",exact:true})).toBeVisible();
  await expect(app.getByLabel("Message the agent",{exact:true})).toBeFocused();
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="open_session"))).toHaveLength(0);
});
