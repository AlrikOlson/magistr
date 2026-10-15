import { test, expect } from "./fixture.mjs";

test("image inspection retains dimensions, exact target and source focus", async ({ app }) => {
  await app.getByRole("button", { name: "Review the release plan", exact: true }).click();
  await app.evaluate(() => window.__fixture.publish("release", {attachments:[{id:"image",name:"icon.png",size:32768,path:"ui/icon.png",content_hash:"image",preview:"http://magistr.test/icon.png"}],draft_revision:2,draft_saved_revision:2}));
  await app.getByRole("button", { name: "View icon.png", exact: true }).click();
  await expect(app.getByRole("dialog", { name: "icon.png", exact: true })).toBeVisible();
  await expect(app.locator("#attachment-metadata")).toContainText("×");
  await app.getByRole("button", { name: "Actual size", exact: true }).click();
  await expect(app.locator("#attachment-viewer")).toHaveAttribute("data-actual","true");
  await app.getByRole("button",{name:"Zoom in",exact:true}).click();
  await expect(app.locator("#attachment-scale")).toHaveText("200%");
  await app.getByRole("button",{name:"Light background",exact:true}).click();
  await expect(app.locator("#attachment-viewer")).toHaveAttribute("data-background","light");
  await app.getByRole("button",{name:"Fit image",exact:true}).click();
  await expect(app.locator("#attachment-scale")).toHaveText("Fit");
  await app.keyboard.press("Escape");
  await expect(app.getByRole("button", { name: "View icon.png", exact: true })).toBeFocused();
  await app.getByRole("button", { name: "View icon.png", exact: true }).click();
  await app.evaluate(() => window.__fixture.publish("release", {attachments:[],draft_revision:3,draft_saved_revision:3}));
  await expect(app.locator("#attachment-viewer")).toBeHidden();
});

test("image strip follows the source gallery and closes when its selected image is removed",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.evaluate(()=>window.__fixture.publish("release",{attachments:["first","second"].map(id=>({id,name:id+".png",size:32768,path:id+".png",content_hash:id,preview:"http://magistr.test/icon.png"})),draft_revision:2,draft_saved_revision:2}));
  await app.getByRole("button",{name:"View first.png",exact:true}).click();
  await expect(app.locator("#attachment-position")).toHaveText("1 / 2");
  await expect(app.getByRole("button",{name:"Previous image",exact:true})).toBeDisabled();
  await app.keyboard.press("ArrowRight");
  await expect(app.locator("#attachment-title")).toHaveText("second.png");
  await expect(app.getByRole("button",{name:"Inspect second.png",exact:true})).toHaveAttribute("aria-pressed","true");
  await app.evaluate(()=>{const f=window.__fixture.frames.get("release");window.__fixture.publish("release",{attachments:f.attachments.filter(file=>file.id==="first"),draft_revision:3,draft_saved_revision:3});});
  await expect(app.locator("#attachment-viewer")).toBeHidden();
  await expect(app.getByRole("button",{name:"View first.png",exact:true})).toBeFocused();
});

test("an unavailable image remains dismissible", async ({ app }) => {
  await app.getByRole("button", { name: "Review the release plan", exact: true }).click();
  await app.evaluate(() => window.__fixture.publish("release", {attachments:[{id:"image",name:"missing.png",size:100,path:"missing.png",content_hash:"missing",preview:"data:image/png;base64,invalid"}],draft_revision:2,draft_saved_revision:2}));
  await app.getByRole("button", { name: "View missing.png", exact: true }).click();
  await expect(app.locator("#attachment-image-status")).toHaveText("This image preview is unavailable.");
  await expect(app.getByRole("button", { name: "Actual size", exact: true })).toBeDisabled();
  await app.getByRole("button", { name: "Close image", exact: true }).click();
  await expect(app.getByRole("button", { name: "View missing.png", exact: true })).toBeFocused();
});
