import { test, expect, showProjects } from "./fixture.mjs";

async function longConversation(app) {
  await app.evaluate(()=>{
    const paragraphs=Array.from({length:45},(_,index)=>({tag:"p",start:index*100,text:"Section "+(index+1)+": Keep the project and conversation clear while work continues. Read the proposed change, keep the draft, and return to the same paragraph after checking another conversation. ".repeat(2),children:[]}));
    const events=[
      {id:1,revision:1,role:"Tool",text:"Review the project checklist",tool:{title:"Read the release checklist",status:"Completed",details:[{label:"Result",source:"The project checklist is available for review."}],duration_ms:null,truncated_from:null,image_count:0}},
      {id:2,revision:1,role:"Agent",text:paragraphs.map(p=>p.text).join("\n\n"),document:{nodes:paragraphs},attachments:[]}
    ];
    window.__fixture.publish("release",{events,retained:[1,2]});
  });
  await showProjects(app);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByText("Read the release checklist · Completed",{exact:true}).click();
  await app.locator('[data-reading="2:1500"]').evaluate(node=>node.scrollIntoView({block:"start"}));
  await expect(app.locator("#latest")).toBeVisible();
}
const offset=app=>app.locator('[data-reading="2:1500"]').evaluate(node=>node.getBoundingClientRect().top-document.getElementById("transcript").getBoundingClientRect().top);
test("code documents preserve exact source, stable selection and copy bytes",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const code="const value = '<script>literal</script>';\n\t日本語();\n";
  await app.evaluate(code=>{
    Object.defineProperty(navigator,"clipboard",{value:{writeText:async text=>{window.__fixture.copied=text;}}});
    window.__fixture.publish("release",{events:[{id:500,revision:1,role:"Agent",text:code,document:{nodes:[{tag:"pre",start:0,children:[{tag:"code",text:code,start:0}]}]}}],retained:[500]});
  },code);
  await expect(app.locator(".code-count")).toHaveText("2 lines");
  expect(await app.locator(".code-document pre code").textContent()).toBe(code);
  await app.getByRole("button",{name:"Copy code",exact:true}).click();
  expect(await app.evaluate(()=>window.__fixture.copied)).toBe(code);
  await app.locator(".code-document pre code").evaluate(node=>{
    window.__fixture.codeNode=node;
    const range=document.createRange();range.setStart(node.firstChild,6);range.setEnd(node.firstChild,11);
    document.getSelection().removeAllRanges();document.getSelection().addRange(range);
  });
  await app.evaluate(()=>window.__fixture.publish("release",{events:[{id:500,revision:2,role:"Agent",text:"updated",document:{nodes:[{tag:"pre",text:"updated",start:0}]}}],retained:[500]}));
  expect(await app.locator(".code-document pre code").textContent()).toBe(code);
  expect(await app.evaluate(()=>document.getSelection().toString())).toBe("value");
  await app.evaluate(()=>document.getSelection().removeAllRanges());
  await expect(app.locator(".code-document pre code")).toHaveText("updated");
  expect(await app.locator(".code-document pre code").evaluate(node=>node===window.__fixture.codeNode)).toBe(true);
});

test("return to the same paragraph, valid text selection and open tool with current background content",async({app})=>{
  await longConversation(app);
  const before=await offset(app);
  await app.locator('[data-reading="2:1500"]').evaluate(node=>{
    const range=document.createRange();range.setStart(node.firstChild,0);range.setEnd(node.firstChild,55);
    getSelection().removeAllRanges();getSelection().addRange(range);
  });
  const selected=await app.evaluate(()=>getSelection().toString());
  await expect.poll(()=>app.evaluate(()=>getSelection().toString())).toBe(selected);
  await showProjects(app);
  await app.getByRole("button",{name:"Investigate sync failure",exact:true}).click();
  await app.evaluate(()=>{
    const frame=window.__fixture.frames.get("release");
    window.__fixture.publish("release",{events:[...frame.events,{id:3,revision:1,role:"Agent",text:"The background review has finished.",attachments:[]}],retained:[1,2,3]});
  });
  await showProjects(app);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await expect(app.locator('article[data-message="1"] details')).toHaveAttribute("open","");
  await expect(app.locator('article[data-message="3"]')).toContainText("background review has finished");
  expect(Math.abs(await offset(app)-before)).toBeLessThan(3);
  expect(await app.evaluate(()=>getSelection().toString())).toBe(selected);
  await app.getByRole("button",{name:"Latest reply",exact:true}).click();
  expect(await app.evaluate(()=>getSelection().toString())).toBe("");
  await expect(app.locator("#latest")).toBeHidden();
});
test("editor caret, direction and scroll restore only against the same current draft",async({app})=>{
  await showProjects(app);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  const editor=app.getByLabel("Message the agent",{exact:true}),draft=Array.from({length:35},(_,i)=>"Draft line "+i+": preserve this work.").join("\n");
  await editor.fill(draft);await expect(app.locator("#draft-status")).toHaveText("Draft saved");
  await editor.evaluate(node=>{node.setSelectionRange(250,310,"backward");node.scrollTop=170;});
  const before=await editor.evaluate(node=>({start:node.selectionStart,end:node.selectionEnd,direction:node.selectionDirection,top:node.scrollTop}));
  await showProjects(app);
  await app.getByRole("button",{name:"Investigate sync failure",exact:true}).click();
  await showProjects(app);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await expect(editor).toHaveValue(draft);
  expect(await editor.evaluate(node=>({start:node.selectionStart,end:node.selectionEnd,direction:node.selectionDirection,top:node.scrollTop}))).toEqual(before);
  await showProjects(app);
  await app.getByRole("button",{name:"Investigate sync failure",exact:true}).click();
  await app.evaluate(()=>window.__fixture.publish("release",{draft:"A newer owner draft.",draft_revision:50,draft_saved_revision:50}));
  await showProjects(app);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await expect(editor).toHaveValue("A newer owner draft.");
  expect(await editor.evaluate(node=>node.selectionStart)).not.toBe(before.start);
});
test("Details, resize and Latest preserve deliberate reading while pruned selection is discarded",async({app})=>{
  await longConversation(app);
  const before=await offset(app);
  await app.getByRole("button",{name:"Details",exact:true}).click();
  await expect.poll(async()=>Math.abs(await offset(app)-before)).toBeLessThan(3);
  await app.setViewportSize({width:800,height:680});
  await expect(app.locator("#transcript")).toBeHidden();
  await app.evaluate(()=>{const frame=window.__fixture.frames.get("release");window.__fixture.publish("release",{events:[...frame.events,{id:3,revision:1,role:"Agent",text:"Work continued while you read Details.",attachments:[]}],retained:[1,2,3]});});
  await app.keyboard.press("Escape");
  await expect(app.locator("#transcript")).toBeVisible();
  await expect.poll(async()=>Math.abs(await offset(app)-before)).toBeLessThan(3);
  await app.locator('[data-reading="2:1500"]').evaluate(node=>{
    const range=document.createRange();range.selectNodeContents(node);getSelection().removeAllRanges();getSelection().addRange(range);
  });
  await showProjects(app);
  await app.getByRole("button",{name:"Investigate sync failure",exact:true}).click();
  await app.evaluate(()=>window.__fixture.publish("release",{events:[{id:3,revision:1,role:"Agent",text:"Only the newest reply is available.",attachments:[]}],retained:[3],dropped:2}));
  await showProjects(app);
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await expect(app.locator("#transcript")).toContainText("Some earlier messages are unavailable.");
  expect(await app.evaluate(()=>getSelection().toString())).toBe("");
});
test("an observation before resize restoration preserves the last stable reading anchor",async({app})=>{
  await app.evaluate(async()=>{
    const { Transcript }=await import("/transcript.mjs");
    const render=Transcript.prototype.render;
    Transcript.prototype.render=function(view){window.__readingView=this;return render.call(this,view);};
  });
  await longConversation(app);
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const before=await offset(app);
  await app.evaluate(()=>{
    // Simulate a narrower pane and an owner update in the same rendering frame.
    const reader=window.__readingView;
    reader.root.style.width="592px";
    reader.render(reader.view);
  });
  await expect.poll(async()=>Math.abs(await offset(app)-before)).toBeLessThan(3);
});
test("compact project browsing preserves reading and draft selection through background updates",async({app})=>{
  await longConversation(app);
  await app.setViewportSize({width:760,height:550});
  await app.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const editor=app.getByLabel("Message the agent",{exact:true});
  await editor.fill("Keep this message while I browse projects.");
  await expect(app.locator("#draft-status")).toHaveText("Draft saved");
  await editor.evaluate(node=>node.setSelectionRange(5,17,"backward"));
  const before=await offset(app);
  await app.getByRole("button",{name:"Projects",exact:true}).click();
  await expect(app.locator("#workspace-main")).toBeHidden();
  await app.evaluate(()=>{
    const frame=window.__fixture.frames.get("release");
    window.__fixture.publish("release",{events:[...frame.events,{id:3,revision:1,role:"Agent",text:"Background work finished.",attachments:[]}],retained:[1,2,3]});
  });
  await app.keyboard.press("Escape");
  await expect(editor).toBeVisible();
  await expect.poll(async()=>Math.abs(await offset(app)-before)).toBeLessThan(3);
  await expect(editor).toHaveValue("Keep this message while I browse projects.");
  expect(await editor.evaluate(node=>[node.selectionStart,node.selectionEnd,node.selectionDirection])).toEqual([5,17,"backward"]);
});

