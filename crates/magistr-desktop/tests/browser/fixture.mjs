// Production UI in an isolated headless browser. This fixture does not verify Rust owners or native input.
import { test as base, expect } from "@playwright/test";
import { workspaceSeed, install } from "./observations.mjs";
import { readFile } from "node:fs/promises";
import { resolve, extname } from "node:path";
import { fileURLToPath } from "node:url";
export const root=fileURLToPath(new URL("../../../..",import.meta.url));
export const ui=resolve(root,"crates/magistr-desktop/ui");
export { workspaceSeed };
export const test=base.extend({
  app:async({page},use)=>{
    const errors=[];page.on("pageerror",error=>errors.push(error.message));
    await page.route("**/*",async route=>{
      const url=new URL(route.request().url());
      if(url.origin!=="http://magistr.test"){await route.abort();return;}
      const file=resolve(ui,url.pathname==="/"?"index.html":"."+url.pathname);
      if(!file.startsWith(ui+"/")){await route.abort();return;}
      const types={".html":"text/html",".css":"text/css",".mjs":"text/javascript",".png":"image/png",".woff2":"font/woff2"};
      await route.fulfill({body:await readFile(file),contentType:types[extname(file)]??"text/plain"});
    });
    await page.addInitScript(install,workspaceSeed());
    await page.goto("http://magistr.test");
    await expect(page.locator("html")).toHaveAttribute("data-theme","dark");
    await page.evaluate(()=>document.fonts.ready);
    await use(page);
    expect(errors).toEqual([]);
  },
});
export { expect };
export async function showProjects(app) {
  if(app.viewportSize().width<=900){
    const toggle=app.getByRole("button",{name:"Projects",exact:true});
    await expect(toggle).toBeVisible();
    if(await toggle.getAttribute("aria-expanded")==="false")await toggle.click();
  }
  await expect(app.locator("#project-navigation")).toBeVisible();
}