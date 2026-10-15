// Source-owned presentation fixtures used by the recorded WebKit composition journey.
export async function showDocument(app,nodes,id=900){
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  if(await app.locator("#close-details").isVisible())await app.locator("#close-details").click();
  await app.evaluate(({nodes,id})=>window.__fixture.publish("release",{
    events:[{id,revision:1,role:"Agent",text:"Retained document source",document:{nodes}}],retained:[id]
  }),{nodes,id});
}
export async function appearances(app,capture,name,focus=null){
  for(const theme of ["dark","light"]){
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await app.getByRole("button",{name:"Done",exact:true}).click();
    await app.locator("#transcript").evaluate(node=>node.scrollTop=0);
    if(focus)await app.locator(focus).evaluate(node=>node.scrollIntoView({block:"center"}));
    await capture(name+"-"+theme+".png");
  }
}
export async function tableScene(app,capture,expect){
  const cell=(tag,text,index)=>({tag,text,start:index,alignment:index===2?"right":null});
  const rows=[
    ["Draft storage","Saved before switching","42","Available offline"],
    ["Conversation history","Retained per project","128","Searchable"],
    ["Source documents","Original text preserved","16","Copy without formatting"],
    ["Attachments","Images and text files","8","Preview before sending"]
  ];
  const nodes=[
    {tag:"h2",start:0,text:"Release readiness"},
    {tag:"p",start:1,text:"Compare what is ready and where the evidence lives."},
    {tag:"table",start:2,children:[
      {tag:"thead",start:3,children:[{tag:"tr",start:4,children:["Area","What is ready","Checks","Notes"].map((text,i)=>cell("th",text,i))}]},
      ...rows.map((row,i)=>({tag:"tr",start:10+i,children:row.map((text,j)=>cell("td",text,j))}))
    ]}
  ];
  await showDocument(app,nodes);
  await expect(app.getByRole("columnheader")).toHaveCount(4);
  await expect(app.getByRole("cell",{name:"42",exact:true})).toHaveCSS("text-align","right");
  await appearances(app,capture,"tables",".table-document");
  await app.setViewportSize({width:760,height:550});
  await expect(app.getByRole("region",{name:"Table",exact:true})).toBeVisible();
  expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(760);
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await app.getByRole("region",{name:"Table",exact:true}).focus();
  await expect(app.getByRole("region",{name:"Table",exact:true})).toBeFocused();
  await app.locator(".table-document").evaluate(node=>node.scrollIntoView({block:"start"}));
  await capture("tables-compact.png");
}

export async function patchScene(app,capture,expect){
  const text="diff --git a/src/draft.rs b/src/draft.rs\n--- a/src/draft.rs\n+++ b/src/draft.rs\n@@ -12,4 +12,5 @@\n pub fn save_draft(text: &str) {\n-    cache.insert(text);\n+    writer.save(text)?;\n+    cache.insert(text.to_owned());\n     notify_saved();\n }\n";
  await showDocument(app,[{tag:"h2",start:0,text:"Keep drafts through a restart"},{tag:"pre",start:10,language:"diff",text}],901);
  await expect(app.locator(".patch-document code")).toHaveText(text,{useInnerText:false});
  expect(await app.locator(".patch-document code").textContent()).toBe(text);
  await expect(app.locator(".patch-added")).toHaveCount(2);
  await expect(app.locator(".patch-removed")).toHaveCount(1);
  await expect(app.locator(".code-count")).toHaveText("+2 added · −1 removed");
  await expect(app.locator(".patch-added").first()).toHaveAttribute("data-position","         13");
  await appearances(app,capture,"patch",".patch-document");
  await app.setViewportSize({width:760,height:550});
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await app.locator(".patch-document").evaluate(node=>node.scrollIntoView({block:"start"}));
  expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(760);
  await capture("patch-compact.png");
}
export async function toolScene(app,capture,expect){
  await showDocument(app,[],902);
  const source=JSON.stringify({file:"src/draft.rs",checks:42,passed:true,duration_ms:1840,changes:{added:2,removed:1},notes:"<script>literal source</script>"},null,2);
  await app.evaluate(source=>{
    Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async text=>{window.__fixture.copied=text;}}});
    window.__fixture.publish("release",{events:[{id:903,revision:1,role:"Tool",text:source,tool:{title:"Verify saved drafts",status:"Completed",duration_ms:1840,details:[{label:"Result",source}]}}],retained:[903]});
  },source);
  await app.locator('article[data-tool]>details>summary').click();
  await expect(app.locator(".result-fields").first()).toContainText("duration_ms");
  await expect(app.locator(".tool-result script")).toHaveCount(0);
  await app.getByRole("button",{name:"Copy result",exact:true}).click();
  expect(await app.evaluate(()=>window.__fixture.copied)).toBe(source);
  await app.locator(".result-fields summary").click();
  await expect(app.locator(".result-fields").first()).toContainText("removed");
  await app.locator(".result-fields summary").click();
  await appearances(app,capture,"result");
  await app.getByText("Original text",{exact:true}).click();
  expect(await app.locator(".result-original pre").textContent()).toBe(source);
  await app.setViewportSize({width:760,height:550});
  expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(760);
}
