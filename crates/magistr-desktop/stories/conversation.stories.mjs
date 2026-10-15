import {desktopMeta,openRelease,conversationVisible} from "./desktop.mjs";
export default {...desktopMeta,title:"Conversation"};
export const ReleasePlan={parameters:{desktop:{
  caption:"Release plan conversation with an unsent message",
  task:"Read the agent reply and continue the conversation.",
  arrange:openRelease,verify:conversationVisible,
}}};
const longTitle="Review reopening, unsent messages, working copies, installation and the final desktop release checklist";
export const LongTitle={parameters:{desktop:{
  caption:"Release conversation with a long title and an unsent message",
  task:"Read the complete conversation title and reach its actions.",
  arrange:async context=>{
    await openRelease(context);
    const frame=context.window.__fixture.frames.get("release");
    context.window.__fixture.publish("release",{conversation:{...frame.conversation,name:longTitle}});
  },
  verify:({doc,expect,visible})=>{
    expect(visible(doc,"#workspace")).toBe(true);
    expect(doc.querySelector("#view-title").textContent).toBe(longTitle);
    expect(visible(doc,"#toggle-details")).toBe(true);
    expect(doc.querySelector(".work-header").scrollWidth).toBeLessThanOrEqual(doc.querySelector(".work-header").clientWidth);
  },
}}};
export const ExpandedEditor={parameters:{desktop:{
  caption:"Release conversation with its message editor expanded",
  task:"Write a longer message while retaining save state, length and send controls.",
  arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Expand editor",exact:true}));},
  verify:({doc,expect,visible})=>{
    expect(doc.querySelector("#composer").dataset.expanded).toBe("true");
    expect(visible(doc,"#draft-measure")).toBe(true);
    expect(doc.activeElement).toBe(doc.querySelector("#draft"));
    expect(doc.querySelector("#draft").value).toContain("Keep the project list visible");
  },
}}};
export const TranscriptStart={parameters:{desktop:{
  caption:"Release conversation at the start of the transcript, showing the sent message and reply opening",
  task:"Read the sent request and the beginning of the agent reply.",
  arrange:async context=>{await openRelease(context);await new Promise(resolve=>context.window.requestAnimationFrame(()=>context.window.requestAnimationFrame(resolve)));context.doc.querySelector("#transcript").scrollTop=0;},
  verify:context=>{conversationVisible(context);context.expect(context.doc.querySelector("#transcript").scrollTop).toBe(0);},
}}};
export const ReplyEnd={parameters:{desktop:{
  caption:"Release conversation scrolled to the end of the agent reply",
  task:"Read the final recommendation and continue with an unsent message.",
  arrange:async context=>{await openRelease(context);await new Promise(resolve=>context.window.requestAnimationFrame(()=>context.window.requestAnimationFrame(resolve)));const transcript=context.doc.querySelector("#transcript");transcript.scrollTop=transcript.scrollHeight;},
  verify:context=>{conversationVisible(context);const t=context.doc.querySelector("#transcript");context.expect(Math.abs(t.scrollHeight-t.clientHeight-t.scrollTop)).toBeLessThanOrEqual(1);},
}}};
const codeText='pub fn save_draft(text: &str) -> Result<()> {\n    writer.save(text)?;\n    cache.insert(text.to_owned());\n    notify_saved();\n    Ok(())\n}\n';
function sourceSeed(seed){seed.frames[0].events=[{id:10,revision:1,role:"Agent",text:codeText,attachments:[],document:{nodes:[{tag:"pre",start:0,language:"rust",text:codeText}]}}];seed.frames[0].retained=[10];}
async function positionSource({doc,window}){await new Promise(resolve=>window.requestAnimationFrame(()=>window.requestAnimationFrame(resolve)));const t=doc.querySelector("#transcript"),code=doc.querySelector(".code-document");if(window.innerWidth<900)t.scrollTop+=code.getBoundingClientRect().top-t.getBoundingClientRect().top;}
const codeState={
  caption:"Release conversation displaying a Rust source block",
  task:"Read source with aligned line numbers and reach wrapping and copy controls.",
  seed:sourceSeed,arrange:async context=>{await openRelease(context);await positionSource(context);},
  verify:context=>{conversationVisible(context);context.expect(context.doc.querySelector(".code-document code").textContent).toBe(codeText);context.expect(context.visible(context.doc,".code-copy")).toBe(true);},
};
export const SourceCode={parameters:{desktop:codeState}};
export const WrappedSource={parameters:{desktop:{...codeState,
  caption:"Release conversation displaying wrapped Rust source",
  task:"Read source with wrapping enabled and copy its original bytes.",
  arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Wrap lines",exact:true}));await positionSource(context);},
  verify:context=>{codeState.verify(context);context.expect(context.doc.querySelector(".code-document").dataset.wrapped).toBe("true");},
}}};

const tableRows=[["Draft storage","Saved before switching","42","Available offline"],["Conversation history","Retained per project","128","Searchable"],["Source documents","Original text preserved","16","Copy without formatting"],["Attachments","Images and text files","8","Preview before sending"]];
const tableSeed=seed=>{
 const cell=(tag,text,index)=>({tag,text,start:index,alignment:index===2?"right":null});
 const nodes=[{tag:"table",start:0,children:[{tag:"thead",start:1,children:[{tag:"tr",start:2,children:["Area","What is ready","Checks","Notes"].map((text,i)=>cell("th",text,i))}]},...tableRows.map((row,i)=>({tag:"tr",start:10+i,children:row.map((text,j)=>cell("td",text,j))}))]}];
 seed.frames[0].events=[{id:11,revision:1,role:"Agent",text:"Release readiness",attachments:[],document:{nodes}}];seed.frames[0].retained=[11];
};
async function positionTable(context){await openRelease(context);await new Promise(resolve=>context.window.requestAnimationFrame(()=>context.window.requestAnimationFrame(resolve)));const t=context.doc.querySelector("#transcript"),table=context.doc.querySelector(".table-document");if(context.window.innerWidth<900)t.scrollTop+=table.getBoundingClientRect().top-t.getBoundingClientRect().top;}
const tableState={caption:"Release conversation showing a four-column readiness table",task:"Compare readiness and check counts across four areas.",seed:tableSeed,arrange:positionTable,
 verify:context=>{conversationVisible(context);context.expect(context.doc.querySelectorAll(".table-document td").length).toBe(16);context.expect(context.doc.querySelector(".table-note").textContent).toBe("4 rows · 4 columns");}};
export const ReadinessTable={parameters:{desktop:tableState}};
export const TableLastRows={parameters:{desktop:{...tableState,
 caption:"Release readiness table scrolled to its last rows",
 task:"Read the last rows while retaining table column headings.",
 arrange:async context=>{await positionTable(context);const table=context.doc.querySelector(".table-document");table.scrollTop=table.scrollHeight;},
 verify:context=>{tableState.verify(context);const t=context.doc.querySelector(".table-document");context.expect(t.scrollHeight-t.clientHeight-t.scrollTop).toBeLessThanOrEqual(1);},
}}};

