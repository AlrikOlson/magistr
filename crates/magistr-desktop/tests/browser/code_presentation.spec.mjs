import { test, expect } from "./fixture.mjs";

test("source surfaces wrap long lines without changing copied bytes", async ({app}) => {
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const source='const message = "'+"a long exact source string ".repeat(20)+'";\n';
  await app.evaluate(source=>{
    Object.defineProperty(navigator,"clipboard",{value:{writeText:async text=>{window.__fixture.copied=text;}}});
    window.__fixture.publish("release",{events:[{id:750,revision:1,role:"Agent",text:source,document:{nodes:[{tag:"pre",start:0,language:"javascript",text:source}]}}],retained:[750]});
  },source);
  const figure=app.locator(".code-document");
  await app.getByRole("button",{name:"Wrap lines",exact:true}).click();
  await expect(figure).toHaveAttribute("data-wrapped","true");
  await expect(figure.locator(".code-gutter")).toBeHidden();
  expect(await figure.locator("pre").evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBe(true);
  await app.getByRole("button",{name:"Copy code",exact:true}).click();
  expect(await app.evaluate(()=>window.__fixture.copied)).toBe(source);
  await app.getByRole("button",{name:"Wrap lines",exact:true}).click();
  await expect(figure.locator(".code-gutter")).toBeVisible();
  expect(await figure.locator("code").textContent()).toBe(source);
});
