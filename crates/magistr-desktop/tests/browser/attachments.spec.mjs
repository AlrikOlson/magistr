import { test, expect } from "./fixture.mjs";

test("image inspection retains dimensions, exact target and source focus", async ({ app }) => {
  await app.getByRole("button", { name: "Review the release plan", exact: true }).click();
  await app.evaluate(() => window.__fixture.publish("release", {attachments:[{id:"image",name:"icon.png",size:32768,path:"ui/icon.png",content_hash:"image",preview:"http://magistr.test/icon.png"}],draft_revision:2,draft_saved_revision:2}));
  await app.getByRole("button", { name: "View icon.png", exact: true }).click();
  await expect(app.getByRole("dialog", { name: "icon.png", exact: true })).toBeVisible();
  await expect(app.locator("#attachment-metadata")).toContainText("×");
  await app.getByRole("button", { name: "Actual size", exact: true }).click();
  await expect(app.locator("#attachment-viewer")).toHaveAttribute("data-actual","true");
  await app.keyboard.press("Escape");
  await expect(app.getByRole("button", { name: "View icon.png", exact: true })).toBeFocused();
  await app.getByRole("button", { name: "View icon.png", exact: true }).click();
  await app.evaluate(() => window.__fixture.publish("release", {attachments:[],draft_revision:3,draft_saved_revision:3}));
  await expect(app.locator("#attachment-viewer")).toBeHidden();
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
