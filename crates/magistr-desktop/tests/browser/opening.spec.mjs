import { test, expect, showProjects } from "./fixture.mjs";

async function show(app) {
  await showProjects(app);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"New conversation",exact:true}).click();
}
test("destination and optional model are explicit and admission cannot duplicate or dismiss",async({app})=>{
  await app.setViewportSize({width:800,height:560});
  await show(app);
  await expect(app.locator("#session-destination-name")).toHaveText("Magistr");
  await expect(app.locator("#session-destination-path")).toHaveText("/Projects/magistr");
  await expect(app.getByRole("radio",{name:"GitHub Copilot",exact:true})).toBeChecked();
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>["open_session","model_choices"].includes(c.command)))).toHaveLength(0);
  await app.getByRole("radio",{name:"API Agent",exact:true}).check();
  await app.getByRole("button",{name:"Choose a model…",exact:true}).click();
  await app.getByRole("radio",{name:"GPT",exact:true}).check();
  const action=await app.getByRole("button",{name:"Start conversation",exact:true}).boundingBox();expect(action.y+action.height).toBeLessThan(560);
  await app.evaluate(()=>window.__fixture.handlers.open_session=()=>new Promise(resolve=>window.finishOpening=resolve));
  await app.getByRole("button",{name:"Start conversation",exact:true}).click();
  await expect(app.getByRole("radio",{name:"API Agent",exact:true})).toBeDisabled();
  await expect(app.getByRole("button",{name:"Cancel",exact:true})).toBeDisabled();
  await app.keyboard.press("Escape");
  await expect(app.locator("#session-picker")).toBeVisible();
  const calls=await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="open_session"));
  expect(calls).toHaveLength(1);expect(calls[0].args).toEqual({directory:"/Projects/magistr",choice:{kind:"new",provider:"api",model:"gpt"}});
  await app.evaluate(()=>window.finishOpening(window.__fixture.frames.get("release").address));
  await expect(app.locator("#session-picker")).toBeHidden();
  await expect(app.getByLabel("Message the agent",{exact:true})).toBeFocused();
});
test("unavailable detection is distinct from no agents and both can retry",async({app})=>{
  await app.evaluate(()=>window.__fixture.handlers.agent_choices=()=>{throw Error("Agent discovery is unavailable.");});
  await show(app);
  await expect(app.locator("#session-picker-error")).toHaveText("Error: Agent discovery is unavailable.");
  await expect(app.locator("#agent-choice-empty")).toBeHidden();
  await expect(app.getByRole("button",{name:"Start conversation",exact:true})).toBeDisabled();
  await app.evaluate(()=>window.__fixture.handlers.agent_choices=()=>[]);
  await app.getByRole("button",{name:"Check again",exact:true}).click();
  await expect(app.locator("#agent-choice-empty")).toBeVisible();
  await expect(app.locator("#session-picker-error")).toBeHidden();
  await app.evaluate(()=>delete window.__fixture.handlers.agent_choices);
  await app.getByRole("button",{name:"Check again",exact:true}).click();
  await expect(app.getByRole("radio",{name:"GitHub Copilot",exact:true})).toBeFocused();
  await app.keyboard.press("Escape");
  await expect(app.getByRole("button",{name:"New conversation",exact:true})).toBeFocused();
});
test("late model failure cannot replace a changed agent or a closed dialog",async({app})=>{
  await show(app);
  await app.evaluate(()=>window.__fixture.handlers.model_choices=()=>new Promise((resolve,reject)=>window.failModels=reject));
  await app.getByRole("button",{name:"Choose a model…",exact:true}).click();
  await expect(app.locator("#session-picker-status")).toHaveText("Checking available models…");
  await app.getByRole("radio",{name:"API Agent",exact:true}).check();
  await app.evaluate(()=>window.failModels(Error("Old provider failed.")));
  await expect(app.locator("#session-picker-error")).toBeHidden();
  await expect(app.getByRole("button",{name:"Start conversation",exact:true})).toBeEnabled();
  await app.getByRole("button",{name:"Choose a model…",exact:true}).click();
  await app.keyboard.press("Escape");
  await app.evaluate(()=>window.failModels(Error("Closed provider failed.")));
  await expect(app.locator("#session-picker")).toBeHidden();
  await expect(app.getByLabel("Message the agent",{exact:true})).toHaveValue("Keep the project list visible, and let me return to the same paragraph.");
});
test("admission refusal preserves choices and admitted navigation retry never starts a second agent",async({app})=>{
  await show(app);
  await app.getByRole("button",{name:"Choose a model…",exact:true}).click();
  await app.getByRole("radio",{name:"Claude Sonnet",exact:true}).check();
  await app.evaluate(()=>window.__fixture.handlers.open_session=()=>{throw Error("The agent needs sign-in.");});
  await app.getByRole("button",{name:"Start conversation",exact:true}).click();
  await expect(app.locator("#session-picker-error")).toContainText("needs sign-in");
  await expect(app.getByRole("radio",{name:"Claude Sonnet",exact:true})).toBeChecked();
  await app.evaluate(()=>{
    window.__fixture.handlers.open_session=()=>window.__fixture.frames.get("release").address;
    window.__fixture.handlers.overview=()=>{delete window.__fixture.handlers.overview;throw Error("Project list could not be read.");};
  });
  await app.getByRole("button",{name:"Start conversation",exact:true}).click();
  await expect(app.locator("#session-picker-error")).toContainText("conversation opened, but could not be shown");
  await app.getByRole("button",{name:"Go to conversation",exact:true}).click();
  await expect(app.locator("#session-picker")).toBeHidden();
  expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="open_session"))).toHaveLength(2);
});
