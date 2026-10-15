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

test("paired patches align real hunk positions and copy the original patch",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const source="--- a/a.rs\n+++ b/a.rs\n@@ -2,2 +2,3 @@\n-old();\n+new();\n+extra();\n done();\n";
  await app.evaluate(source=>{
    Object.defineProperty(navigator,"clipboard",{value:{writeText:async text=>{window.__fixture.copied=text;}}});
    window.__fixture.publish("release",{events:[{id:751,revision:1,role:"Agent",text:source,document:{nodes:[{tag:"pre",start:0,language:"diff",text:source}]}}],retained:[751]});
  },source);
  await app.getByRole("button",{name:"Side by side",exact:true}).click();
  await expect(app.getByRole("region",{name:"Before and after",exact:true})).toBeVisible();
  await expect(app.locator(".diff-cell-removed code")).toHaveText("old();\n");
  await expect(app.locator(".diff-cell-added code")).toHaveText(["new();\n","extra();\n"]);
  await expect(app.locator(".diff-cell-added .diff-line-number")).toHaveText(["2","3"]);
  await expect(app.locator(".diff-cell-empty")).toHaveCount(1);
  await app.getByRole("button",{name:"Copy code",exact:true}).click();
  expect(await app.evaluate(()=>window.__fixture.copied)).toBe(source);
  await app.getByRole("button",{name:"Side by side",exact:true}).click();
  expect(await app.locator(".code-document code").first().textContent()).toBe(source);
});

test("homogeneous result records form a bounded table and keep original source",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const source=JSON.stringify({checks:[{name:"format",passed:true,seconds:1.2},{name:"<script>literal</script>",passed:false,seconds:0}]});
  await app.evaluate(source=>{
    window.__fixture.publish("release",{events:[{id:752,revision:1,role:"Tool",text:source,tool:{title:"Check results",status:"Finished",details:[{label:"Result",source}]}}],retained:[752]});
  },source);
  await app.locator('article[data-tool]>details>summary').click();
  await app.locator(".result-fields summary").click();
  await expect(app.getByRole("region",{name:"Records",exact:true})).toBeVisible();
  await expect(app.locator(".result-records th")).toHaveText(["name","passed","seconds"]);
  await expect(app.locator(".result-records script")).toHaveCount(0);
  await expect(app.locator(".result-records .result-number")).toHaveText(["1.2","0"]);
  await app.getByText("Original text",{exact:true}).click();
  expect(await app.locator(".result-original pre").textContent()).toBe(source);
});
