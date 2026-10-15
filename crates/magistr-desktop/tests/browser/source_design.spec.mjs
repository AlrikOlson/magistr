import { test, expect } from "./fixture.mjs";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { root } from "./fixture.mjs";
test("named source languages retain literal content, copy and selected text through updates", async ({ app }) => {
  await app.getByRole("button", {name:"Review the release plan",exact:true}).click();
  const source='// Keep an interrupted message.\nexport async function saveDraft(target, text) {\n  const saved = await writer.save(target, text);\n  return {saved, message: "<script>literal</script>", retries: 3};\n}\n';
  await app.evaluate(source => {
    Object.defineProperty(navigator,"clipboard",{value:{writeText:async text=>{window.__fixture.copied=text;}}});
    window.__fixture.publish("release",{events:[{id:610,revision:1,role:"Agent",text:source,document:{nodes:[{tag:"h2",start:0,text:"Keep every draft"},{tag:"p",start:20,text:"Save the message before changing conversations. The writer returns the saved revision."},{tag:"pre",start:100,language:"javascript",text:source}]}}],retained:[610]});
  },source);
  const code=app.locator(".code-document code");
  expect(await code.textContent()).toBe(source);
  await expect(code.locator(".source-keyword").first()).toHaveText("export");
  await expect(code.locator(".source-string")).toHaveText('"<script>literal</script>"');
  await expect(app.locator(".code-title")).toHaveText("javascript");
  await expect(code.locator("script")).toHaveCount(0);
  await app.getByRole("button",{name:"Copy code",exact:true}).click();
  expect(await app.evaluate(()=>window.__fixture.copied)).toBe(source);
  await mkdir(resolve(root,".magistr/desktop-design"),{recursive:true});
  for(const theme of ["dark","light"]){
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await app.getByRole("button",{name:"Done",exact:true}).click();
    await app.locator("#transcript").evaluate(node=>node.scrollTop=0);
    await app.screenshot({path:resolve(root,".magistr/desktop-design/source-"+theme+".png")});
  }
  await code.evaluate(node=>{
    const range=document.createRange();range.selectNodeContents(node);
    document.getSelection().removeAllRanges();document.getSelection().addRange(range);
  });
  await app.evaluate(()=>window.__fixture.publish("release",{events:[{id:610,revision:2,role:"Agent",text:"new",document:{nodes:[{tag:"pre",start:100,language:"rust",text:"let next = 42;"}]}}],retained:[610]}));
  expect(await app.evaluate(()=>document.getSelection().toString())).toBe(source);
  await app.evaluate(()=>document.getSelection().removeAllRanges());
  await expect(code).toHaveText("let next = 42;");
  const bounded=await app.evaluate(async()=>{
    const {colorSource}=await import("/source_color.mjs");
    const text="x".repeat(65537);return colorSource(text,"rust").length===1&&colorSource(text,"rust")[0].text===text;
  });
  expect(bounded).toBe(true);
  const lexical=await app.evaluate(async()=>{
    const {colorSource}=await import("/source_color.mjs");
    const samples=[
      ["javascript","const label = 'hello <img>';"],
      ["rust","let name: &'a str = \"hello\"; // 日本語"],
      ["python","value = 'café' # keep exactly"],
      ["unrecognized","<script>literal</script>"],
      ["javascript",'"' + '\\\\\\" let'.repeat(5000)]
    ];
    return samples.map(([language,text])=>({exact:colorSource(text,language).map(token=>token.text).join("")===text,
      strings:colorSource(text,language).filter(token=>token.kind==="string").map(token=>token.text)}));
  });
  expect(lexical.every(sample=>sample.exact)).toBe(true);
  expect(lexical[0].strings).toEqual(["'hello <img>'"]);
  expect(lexical[1].strings).toEqual(['"hello"']);
});
