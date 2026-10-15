import { test, expect } from "./fixture.mjs";

test("project activity follows catalog state without counting closed conversations",async({app})=>{
  const overview=app.locator("#project-activity");
  await expect(overview).toHaveAttribute("aria-label","3 open conversations across your projects");
  await expect(overview.locator('[data-kind="waiting"] dd')).toHaveText("1");
  await app.evaluate(()=>window.__fixture.publish("release",{activity:{active:true}}));
  await expect(overview.locator('[data-kind="working"] dd')).toHaveText("1");
  await app.evaluate(()=>window.__fixture.publish("release",{closed:true}));
  await expect(overview).toHaveAttribute("aria-label","2 open conversations across your projects");
  await expect(overview.locator('[data-kind="working"] dd')).toHaveText("0");
  await app.evaluate(()=>window.__fixture.publish("sync",{status:"Connection lost"}));
  await expect(overview.locator('[data-kind="waiting"] dd')).toHaveText("0");
  await expect(overview.locator('[data-kind="disconnected"] dd')).toHaveText("1");
});

test("activity material keeps the semantic counts through graphics context loss",async({app})=>{
  const canvas=app.locator(".activity-material");
  await expect(canvas).toHaveCount(1);
  const available=await canvas.evaluate(node=>!!node.getContext("webgl"));
  if(available){
    await expect(app.locator(".activity-wheel")).toHaveAttribute("data-material","rendered");
    expect(await canvas.evaluate(node=>node.width)).toBeLessThanOrEqual(384);
    expect(await canvas.evaluate(node=>node.width===node.height)).toBe(true);
    await canvas.evaluate(node=>node.getContext("webgl").getExtension("WEBGL_lose_context")?.loseContext());
    await expect(app.locator(".activity-wheel")).not.toHaveAttribute("data-material","rendered");
  }
  await expect(app.locator('#project-activity [data-kind="waiting"] dd')).toHaveText("1");
  await expect(app.locator(".activity-wheel strong")).toHaveText("3");
});

test("dimensional material redraws only when its displayed data changes",async({app})=>{
  const canvas=app.locator(".activity-material");
  const available=await canvas.evaluate(node=>!!node.getContext("webgl"));
  test.skip(!available,"WebGL unavailable in this renderer");
  await expect(app.locator(".activity-wheel")).toHaveAttribute("data-material","rendered");
  await canvas.evaluate(node=>{
    const gl=node.getContext("webgl"),draw=gl.drawArrays.bind(gl);
    window.__draws=0;gl.drawArrays=(...args)=>{window.__draws++;return draw(...args);};
  });
  await app.evaluate(()=>window.__fixture.publish("release",{status:"Ready"}));
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  expect(await app.evaluate(()=>window.__draws)).toBe(0);
  await app.evaluate(()=>window.__fixture.publish("release",{activity:{active:true}}));
  await expect.poll(()=>app.evaluate(()=>window.__draws)).toBeGreaterThan(0);
  expect(await canvas.evaluate(node=>node.getContext("webgl").getError())).toBe(0);
});

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
test("the project directory reflects live conversations and opens the selected owner",async({app})=>{
  const directory=app.locator("#welcome-projects"),project=directory.locator(".directory-project").first();
  await expect(project.locator(".directory-project-facts dd")).toHaveText(["2","1"]);
  const target=app.getByRole("button",{name:"Continue Investigate sync failure in Magistr",exact:true});
  await target.focus();
  await app.evaluate(()=>window.__fixture.publish("sync",{pending:[],status:"Ready"}));
  await expect(project.locator(".directory-project-facts dd")).toHaveText(["2","0"]);
  await expect(target).toBeFocused();await expect(target).toContainText("Ready");
  await app.evaluate(()=>window.__fixture.publish("release",{closed:true}));
  await expect(project.locator(".directory-project-facts dd")).toHaveText(["1","0"]);
  await expect(project.locator(".directory-conversation")).toHaveCount(1);
  await target.click();
  await expect(app.getByRole("heading",{name:"Investigate sync failure",exact:true})).toBeVisible();
  const call=await app.evaluate(()=>window.__fixture.calls.filter(call=>call.command==="subscribe_session").at(-1));
  expect(call.args.address.session).toBe("sync");
});

test("a failed save returns from compact projects to the retained draft and visible error",async({app})=>{
  await app.setViewportSize({width:760,height:550});
  await app.getByRole("button",{name:"Projects",exact:true}).click();
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.evaluate(()=>window.__fixture.handlers.save_draft=()=>{throw Error("Could not save this message.");});
  const editor=app.getByLabel("Message the agent",{exact:true});
  await editor.fill("Keep this exact draft.");
  await app.getByRole("button",{name:"Projects",exact:true}).click();
  await app.getByRole("button",{name:"Investigate sync failure",exact:true}).click();
  await expect(app.locator("#workspace-main")).toBeVisible();
  await expect(app.locator("#error")).toBeFocused();await expect(app.locator("#error")).toContainText("Could not save");
  await expect(editor).toHaveValue("Keep this exact draft.");
  await expect(app.getByRole("heading",{name:"Review the release plan",exact:true})).toBeVisible();
  await app.evaluate(()=>delete window.__fixture.handlers.save_draft);
});

test("appearance preview choices use the same saved preference and preserve motion",async({app})=>{
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Reduce motion",{exact:true}).check();
  await app.getByRole("button",{name:"Use light appearance",exact:true}).click();
  await expect(app.getByLabel("Appearance",{exact:true})).toHaveValue("light");
  await expect(app.getByRole("button",{name:"Use light appearance",exact:true})).toHaveAttribute("aria-pressed","true");
  await expect(app.getByLabel("Reduce motion",{exact:true})).toBeChecked();
  await app.getByRole("button",{name:"Use system appearance",exact:true}).click();
  await expect(app.getByLabel("Appearance",{exact:true})).toHaveValue("system");
  await expect(app.locator("html")).not.toHaveAttribute("data-theme");
});