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
  await app.getByRole("button",{name:"Side by side",exact:true}).click();
  await appearances(app,capture,"patch-comparison",".patch-document");
  await app.getByRole("button",{name:"Side by side",exact:true}).click();
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
    window.__fixture.publish("release",{events:[{id:903,revision:1,role:"Tool",text:source,tool:{title:"Verify saved drafts",status:"Finished",duration_ms:1840,details:[{label:"Result",source}]}}],retained:[903]});
  },source);
  await app.locator('article[data-tool]>details>summary').click();
  await expect(app.locator(".result-fields").first()).toContainText("duration_ms");
  await expect(app.locator(".source-result script")).toHaveCount(0);
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
export async function outlineScene(app,capture,expect){
  const names=["What is ready","What needs attention","Verification results","Next steps"];
  await showDocument(app,names.flatMap((text,index)=>[
    {tag:"h2",start:index*100,text},
    {tag:"p",start:index*100+10,text:"Keep the current work visible. Each conversation retains its own draft, selected text and reading position. Review the evidence before continuing. ".repeat(3)}
  ]),904);
  await expect(app.getByRole("navigation",{name:"Reply contents",exact:true})).toBeVisible();
  await expect(app.locator(".reply-contents button")).toHaveCount(4);
  await appearances(app,capture,"contents");
  await app.getByRole("button",{name:"Verification results",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Verification results",exact:true})).toBeFocused();
  const position=await app.getByRole("heading",{name:"Verification results",exact:true}).boundingBox();
  const reading=await app.locator("#transcript").boundingBox();
  expect(position.y).toBeGreaterThanOrEqual(reading.y);
  expect(position.y).toBeLessThan(reading.y+70);
  await app.setViewportSize({width:760,height:550});
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await app.locator("#transcript").evaluate(node=>node.scrollTop=0);
  await capture("contents-compact.png");
}
export async function editorialScene(app,capture,expect){
  await showDocument(app,[
    {tag:"h2",start:0,text:"A dependable place to work"},
    {tag:"blockquote",start:1,children:[{tag:"p",start:2,text:"The draft should still be there when I return. I should never have to wonder whether my message was saved."}]},
    {tag:"ul",start:3,children:[
      {tag:"li",start:4,children:[{tag:"task",checked:true,start:4},{tag:"text",text:"Preserve the draft before switching",start:5}]},
      {tag:"li",start:6,children:[{tag:"task",checked:false,start:6},{tag:"text",text:"Review the release evidence",start:7},
        {tag:"ul",start:8,children:[{tag:"li",start:9,text:"Check the desktop at its smallest supported window."},{tag:"li",start:10,text:"Confirm both light and dark appearances."}]}]}
    ]}
  ],905);
  await expect(app.getByRole("img",{name:"Completed",exact:true})).toHaveCount(1);
  await expect(app.getByRole("img",{name:"Not completed",exact:true})).toHaveCount(1);
  await expect(app.locator(".message-body input")).toHaveCount(0);
  await appearances(app,capture,"editorial");
  await app.setViewportSize({width:760,height:550});
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await app.locator("blockquote").evaluate(node=>node.scrollIntoView({block:"start"}));
  expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(760);
  await capture("editorial-compact.png");
}
export async function galleryScene(app,capture,expect){
  await showDocument(app,[],906);
  await app.evaluate(()=>window.__fixture.publish("release",{events:[{id:907,revision:1,role:"You",text:"Here are the files for the release review.",attachments:[
    {id:"sent-notes",name:"release-notes.md",size:2460,path:"docs/release-notes.md",content_hash:"notes",excerpt:"# Release notes\n\nKeep drafts when changing conversations.\nReview every change before applying it.\n\n## Verification\nDesktop checks pass."},
    {id:"sent-image",name:"icon.png",size:32768,path:"ui/icon.png",content_hash:"image",preview:"http://magistr.test/icon.png"}
  ]}],retained:[907]}));
  await expect(app.locator(".transcript .attachment")).toHaveCount(2);
  await expect(app.locator(".transcript .attachment-excerpt")).toContainText("Release notes");
  const thumbnail=await app.locator(".transcript .attachment img").boundingBox();
  const stage=await app.locator(".transcript [data-kind=image] .attachment-visual").boundingBox();
  expect(thumbnail.height).toBeLessThanOrEqual(stage.height);
  await appearances(app,capture,"gallery");
  await app.getByRole("button",{name:"View icon.png",exact:true}).click();
  await expect(app.locator("#attachment-metadata")).toContainText("×");
  await app.getByRole("button",{name:"Close image",exact:true}).click();
  await expect(app.getByRole("button",{name:"View icon.png",exact:true})).toBeFocused();
  await app.setViewportSize({width:760,height:550});
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await app.locator(".transcript .attachments").evaluate(node=>node.scrollIntoView({block:"start"}));
  expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(760);
  await capture("gallery-compact.png");
}
export async function viewerScene(app,capture,expect){
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"View icon.png",exact:true}).click();
  await expect(app.locator("#attachment-metadata")).toContainText("×");
  await app.getByRole("button",{name:"Dark background",exact:true}).click();
  await capture("image-stage-dark.png");
  await app.getByRole("button",{name:"Light background",exact:true}).click();
  await capture("image-stage-light.png");
  await app.getByRole("button",{name:"Actual size",exact:true}).click();
  await app.getByRole("button",{name:"Zoom in",exact:true}).click();
  await expect(app.locator("#attachment-scale")).toHaveText("200%");
  await app.setViewportSize({width:760,height:550});
  const zoom=await app.getByRole("button",{name:"Zoom in",exact:true}).boundingBox();
  expect(zoom.y+zoom.height).toBeLessThan(550);
  await capture("image-stage-compact.png");
  await app.getByRole("button",{name:"Close image",exact:true}).click();
}
export async function writingScene(app,capture,expect){
  await app.setViewportSize({width:1280,height:820});
  const draft=app.getByLabel("Message the agent",{exact:true});
  const text="Review the release plan\n\nFocus on the changes that help someone work for a full afternoon:\n\n• Keep every draft through navigation and restarts.\n• Make the review evidence readable.\n• Give images enough room for close inspection.\n\nStart with the draft writer and record the focused checks.";
  await draft.fill(text);
  await draft.evaluate(node=>{window.__originalEditor=node;node.setSelectionRange(7,18,"backward");});
  await app.getByRole("button",{name:"Expand editor",exact:true}).click();
  await expect(draft).toBeFocused();
  expect(await draft.evaluate(node=>node===window.__originalEditor&&node.selectionStart===7&&node.selectionEnd===18)).toBe(true);
  const bounds=await draft.boundingBox();expect(bounds.height).toBeGreaterThan(350);
  await appearances(app,capture,"writing");
  await app.setViewportSize({width:760,height:550});
  const send=await app.getByRole("button",{name:"Send",exact:true}).boundingBox();
  expect(send.y+send.height).toBeLessThan(550);
  await capture("writing-compact.png");
  await draft.focus();await app.keyboard.press("Escape");
  await expect(app.getByRole("button",{name:"Expand editor",exact:true})).toBeFocused();
  await expect(draft).toHaveValue(text);
}
export async function ledgerScene(app,capture,expect){
  await showDocument(app,[],910);
  await app.evaluate(()=>window.__fixture.publish("release",{events:[
    {id:911,revision:1,role:"Tool",text:"Checked the desktop",tool:{title:"Check desktop compilation",status:"Finished",duration_ms:1840,details:[{label:"Input",source:JSON.stringify({command:"cargo check --manifest-path crates/magistr-desktop/Cargo.toml"})},{label:"Result",source:"Compilation finished without warnings."}]}},
    {id:912,revision:1,role:"Tool",text:"Could not read log",tool:{title:"Read the session log",status:"Failed",duration_ms:46,details:[{label:"Location",source:"logs/session.log"},{label:"Result",source:"Permission was denied."}]}},
    {id:913,revision:1,role:"Tool",text:"Capturing screenshots",tool:{title:"Capture the desktop views",status:"Running",duration_ms:null,details:[{label:"Input",source:"npm run test:ui"}]}}
  ],retained:[911,912,913]}));
  await expect(app.locator(".tool-status")).toHaveText(["Finished","Failed","Running"]);
  await expect(app.locator(".tool-preview").first()).toHaveText("cargo check --manifest-path crates/magistr-desktop/Cargo.toml");
  await expect(app.locator('[data-message="913"] .tool-duration')).toBeHidden();
  await appearances(app,capture,"execution");
  await app.locator('[data-message="912"]>details>summary').click();
  await expect(app.locator('[data-message="912"] .tool-body')).toContainText("Permission was denied.");
  await app.setViewportSize({width:760,height:550});
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await app.locator('[data-message="912"]').evaluate(node=>node.scrollIntoView({block:"start"}));
  expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(760);
  await capture("execution-compact.png");
}
export async function permissionScene(app,capture,expect){
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Investigate sync failure",exact:true}).click();
  const source=JSON.stringify({command:"cargo test -p magistr-application sync",directory:"/Projects/magistr",network:false},null,2);
  await app.evaluate(source=>{
    Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async value=>{window.__fixture.copied=value;}}});
    window.__fixture.publish("sync",{pending:[{id:811,title:"Run the focused sync checks",detail:source,choices:[
      {option_id:"once",label:"Allow once"},{option_id:"always",label:"Always allow"},{option_id:"deny",label:"Deny"},{option_id:"never",label:"Always deny"}
    ]}]});
  },source);
  await app.locator("#review-requests").click();
  await expect(app.locator(".request .result-fields")).toContainText("network");
  await app.getByRole("button",{name:"Copy request details",exact:true}).click();
  expect(await app.evaluate(()=>window.__fixture.copied)).toBe(source);
  await appearances(app,capture,"permission",".request");
  await app.setViewportSize({width:760,height:550});
  await app.locator("#review-requests").click();
  await app.locator(".request-actions").evaluate(node=>node.scrollIntoView({block:"center"}));
  const buttons=await app.locator(".request-actions button").count();expect(buttons).toBe(4);
  expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(760);
  await capture("permission-compact.png");
  await app.getByRole("button",{name:"Deny",exact:true}).click();
  const action=await app.evaluate(()=>window.__fixture.calls.filter(call=>call.command==="session_action").at(-1));
  expect(action.args).toMatchObject({target:{address:{session:"sync"}},action:{action:"answer",id:811,option_id:"deny"}});
}
export async function historyScene(app,capture,expect){
  await app.setViewportSize({width:1280,height:820});
  const source="## Ready to review\n\nThe draft stays with its conversation.\n\n- Check the installer.\n- Keep the previous build available.";
  await app.evaluate(source=>{const record=window.__fixture.history[0];record.summary=source;record.document={nodes:[
    {tag:"h2",start:0,text:"Ready to review"},
    {tag:"p",start:20,text:"The draft stays with its conversation."},
    {tag:"ul",start:60,children:[{tag:"li",start:60,text:"Check the installer."},{tag:"li",start:84,text:"Keep the previous build available."}]}
  ]};},source);
  for(const theme of ["dark","light"]){
    await app.getByRole("button",{name:"Settings",exact:true}).click();
    await app.getByLabel("Appearance",{exact:true}).selectOption(theme);
    await app.getByRole("button",{name:"Done",exact:true}).click();
    await app.getByRole("button",{name:"Saved conversations",exact:true}).click();
    await expect(app.locator("#history-text h2")).toHaveText("Ready to review");
    await expect(app.locator("#history-text li")).toHaveCount(2);
    await app.locator(".saved-reply").evaluate(node=>node.scrollIntoView({block:"start"}));
    await app.getByRole("button",{name:"Copy saved reply",exact:true}).click();
    expect(await app.evaluate(()=>window.__fixture.copied)).toBe(source);
    await capture("saved-document-"+theme+".png");
    if(theme==="light"){
      await app.setViewportSize({width:760,height:550});
      await app.locator(".saved-reply").evaluate(node=>node.scrollIntoView({block:"start"}));
      await capture("saved-document-compact.png");
    }
    await app.getByRole("button",{name:"Close saved conversations",exact:true}).click();
  }
}
export async function workspaceScaleScene(app,capture,expect){
  await showDocument(app,[
    {tag:"h2",start:0,text:"Ready for a focused review"},
    {tag:"p",start:30,text:"The conversation keeps its draft, selected text and reading position. Project activity stays nearby, and the details are available when you need them."},
    {tag:"blockquote",start:200,children:[{tag:"p",start:202,text:"Keep the work easy to read, even when the window is small."}]},
    {tag:"h3",start:300,text:"What changed"},
    {tag:"ul",start:320,children:[{tag:"li",start:322,text:"Source documents keep exact copy and readable line positions."},{tag:"li",start:390,text:"Saved conversations use the same document presentation."},{tag:"li",start:460,text:"Setup review separates current files from proposed changes."}]}
  ],920);
  await app.getByLabel("Message the agent",{exact:true}).fill("Keep the evidence with this release.");
  await app.setViewportSize({width:1920,height:1080});
  await appearances(app,capture,"workspace-wide");
  await app.getByRole("button",{name:"Details",exact:true}).click();
  await capture("workspace-wide-details-light.png");
  expect((await app.locator("#transcript").boundingBox()).width).toBeGreaterThan(1000);
  await app.setViewportSize({width:760,height:550});
  if(await app.locator("#close-details").isVisible())await app.locator("#close-details").click();
  await appearances(app,capture,"workspace-compact-final");
  expect((await app.locator("#transcript").boundingBox()).height).toBeGreaterThanOrEqual(200);
  expect(await app.evaluate(()=>document.documentElement.scrollWidth)).toBe(760);
  const title="Review the desktop release, preserve every saved draft, and verify the installer before sharing the next build";
  await app.evaluate(name=>{const f=window.__fixture.frames.get("release");window.__fixture.publish("release",{conversation:{...f.conversation,name}});},title);
  await expect(app.locator("#view-title")).toHaveAttribute("title",title);
  expect((await app.locator("#view-title").boundingBox()).height).toBeLessThanOrEqual(49);
  await capture("workspace-compact-long-title.png");
}
