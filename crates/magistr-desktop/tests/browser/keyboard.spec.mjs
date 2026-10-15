import { test, expect } from "./fixture.mjs";
const modifier=app=>app.evaluate(()=>/Mac|iPhone|iPad/.test(navigator.platform)?"Meta":"Control");
test("find a conversation with the keyboard and recover the originating editor on cancel",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const editor=app.getByLabel("Message the agent",{exact:true}),mod=await modifier(app);
  await editor.focus();await app.keyboard.press(mod+"+k");
  await expect(app.getByLabel("Find an open conversation",{exact:true})).toBeFocused();
  await app.getByLabel("Find an open conversation",{exact:true}).fill("not-a-conversation");
  await expect(app.locator("#switch-empty")).toHaveText("No open conversations match your search.");
  await app.keyboard.press("Escape");await expect(editor).toBeFocused();
  await app.keyboard.press(mod+"+k");await app.getByLabel("Find an open conversation",{exact:true}).fill("sync magistr");
  await app.keyboard.press("ArrowDown");
  await expect(app.getByRole("button",{name:"Open Investigate sync failure in Magistr",exact:true})).toBeFocused();
  await app.keyboard.press("Enter");
  await expect(app.getByRole("heading",{name:"Investigate sync failure",exact:true})).toBeFocused();
  await expect(app.locator("#switch-picker")).toBeHidden();
});
test("F6 reaches every logical area and recovers the conversation in a narrow window",async({app})=>{
  await app.setViewportSize({width:800,height:680});
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Find a conversation",exact:true}).focus();
  await app.keyboard.press("F6");await expect(app.locator("#transcript")).toBeFocused();
  await app.keyboard.press("F6");await expect(app.getByLabel("Message the agent",{exact:true})).toBeFocused();
  await app.keyboard.press("F6");await expect(app.getByRole("heading",{name:"Conversation details",exact:true})).toBeFocused();
  await expect(app.locator("#transcript")).toBeHidden();
  await app.keyboard.press("Shift+F6");await expect(app.getByLabel("Message the agent",{exact:true})).toBeFocused();
  await expect(app.locator("#transcript")).toBeVisible();
  await app.keyboard.press("Shift+F6");await expect(app.locator("#transcript")).toBeFocused();
  await app.locator(".project-group").first().evaluate(node=>node.open=false);
  await app.keyboard.press("Shift+F6");await expect(app.getByRole("button",{name:"Find a conversation",exact:true})).toBeFocused();
});
test("shortcuts are documented, do not stack dialogs and ignore composition",async({app})=>{
  const mod=await modifier(app);
  await app.evaluate(()=>{
    const mac=/Mac|iPhone|iPad/.test(navigator.platform);
    document.dispatchEvent(new KeyboardEvent("keydown",{key:"k",bubbles:true,metaKey:mac,ctrlKey:!mac,isComposing:true}));
  });
  await expect(app.locator("dialog[open]")).toHaveCount(0);
  await app.keyboard.press(mod+"+,");
  await expect(app.getByRole("heading",{name:"Keyboard shortcuts",exact:true})).toBeVisible();
  await expect(app.locator("#open-project-shortcut")).toContainText(mod==="Meta"?"⌘":"Ctrl");
  await app.keyboard.press(mod+"+k");await app.keyboard.press(mod+"+o");
  await expect(app.locator("dialog[open]")).toHaveCount(1);await expect(app.locator("#settings")).toBeVisible();
  await app.keyboard.press("Escape");await expect(app.getByRole("button",{name:"Settings",exact:true})).toBeFocused();
  await app.keyboard.press(mod+"+o");await expect(app.getByLabel("Project folder",{exact:true})).toBeFocused();
});
test("a failed draft save keeps the switcher and current draft available for retry",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.evaluate(()=>window.__fixture.handlers.save_draft=()=>{throw Error("The draft could not be saved.");});
  await app.getByLabel("Message the agent",{exact:true}).fill("Keep this draft during recovery.");
  await app.getByRole("button",{name:"Find a conversation",exact:true}).click();
  await app.getByRole("button",{name:"Open Investigate sync failure in Magistr",exact:true}).click();
  await expect(app.locator("#switch-error")).toContainText("draft could not be saved");
  await expect(app.locator("#switch-error")).toBeFocused();
  await expect(app.getByRole("heading",{name:"Review the release plan",exact:true})).toBeVisible();
  await expect(app.getByLabel("Message the agent",{exact:true})).toHaveValue("Keep this draft during recovery.");
  await app.evaluate(()=>delete window.__fixture.handlers.save_draft);
  await app.getByRole("button",{name:"Open Investigate sync failure in Magistr",exact:true}).click();
  await expect(app.locator("#switch-picker")).toBeHidden();
  await expect(app.getByRole("heading",{name:"Investigate sync failure",exact:true})).toBeFocused();
});
test("search groups real projects and preserves keyboard focus through catalog updates",async({app})=>{
  await app.getByRole("button",{name:"Find a conversation",exact:true}).click();
  await expect(app.locator(".switch-group")).toHaveCount(2);
  await expect(app.getByRole("region",{name:"Magistr conversations",exact:true}).getByRole("button")).toHaveCount(2);
  await app.keyboard.press("ArrowDown");
  const first=app.getByRole("button",{name:"Open Review the release plan in Magistr",exact:true});
  await expect(first).toBeFocused();
  await app.evaluate(()=>window.__fixture.publish("release",{status:"Working"}));
  await expect(first).toBeFocused();await expect(first).toContainText("Working");
  await app.keyboard.press("End");
  await expect(app.getByRole("button",{name:"Open Update the home page in Website",exact:true})).toBeFocused();
  await app.keyboard.press("Home");await expect(first).toBeFocused();
  await app.getByLabel("Find an open conversation",{exact:true}).fill("magistr");
  await expect(app.locator(".switch-group")).toHaveCount(1);
  await expect(app.locator("#switch-count")).toHaveText("2 conversations");
});

