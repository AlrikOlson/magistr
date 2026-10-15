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

function toolSeed(seed){
 seed.frames[0].events=[
  {id:12,revision:1,role:"Tool",text:"Formatting passed",tool:{title:"Check formatting",status:"Finished",duration_ms:1200,details:[{label:"Input",source:'{"command":"cargo fmt --check"}'},{label:"Result",source:"Formatting passed"}]}},
  {id:13,revision:1,role:"Tool",text:"Draft storage check failed",tool:{title:"Verify saved drafts",status:"Failed",duration_ms:2400,details:[{label:"Result",source:"The saved message did not match after reopening. Review the draft writer before retrying."}]}}
 ];seed.frames[0].retained=[12,13];
}
async function toolPosition(context){await new Promise(resolve=>context.window.requestAnimationFrame(()=>context.window.requestAnimationFrame(resolve)));context.doc.querySelector("#transcript").scrollTop=0;}
const toolState={caption:"Release conversation with a finished formatting check and failed draft check",task:"Scan tool outcomes and duration, then open the failed check.",seed:toolSeed,
 arrange:async context=>{await openRelease(context);await toolPosition(context);},
 verify:context=>{conversationVisible(context);context.expect(context.doc.querySelectorAll("article[data-tool]").length).toBe(2);context.expect(context.doc.querySelectorAll("article[data-tool] details[open]").length).toBe(0);}};
export const ToolActivity={parameters:{desktop:toolState}};
export const FailedToolDetails={parameters:{desktop:{...toolState,
 caption:"Release conversation showing the failed draft check's details",
 task:"Read why the draft check failed and reach its copy control.",
 arrange:async context=>{await openRelease(context);await context.userEvent.click(context.doc.querySelector('article[data-status="failed"] summary'));await toolPosition(context);const t=context.doc.querySelector("#transcript"),row=context.doc.querySelector('article[data-status="failed"]');if(context.window.innerWidth<900)t.scrollTop+=row.getBoundingClientRect().top-t.getBoundingClientRect().top;},
 verify:context=>{conversationVisible(context);context.expect(context.doc.querySelector('article[data-status="failed"] details').open).toBe(true);context.expect(context.doc.querySelector('article[data-status="failed"] .tool-body').textContent).toContain("saved message did not match");},
}}};

const outlineNames=["What is ready","What needs attention","Verification results","Next steps"];
function outlineSeed(seed){const nodes=outlineNames.flatMap((text,index)=>[{tag:"h2",start:index*100,text},{tag:"p",start:index*100+10,text:"Keep the current work visible. Each conversation retains its own draft, selected text and reading position. Review the evidence before continuing. ".repeat(3)}]);seed.frames[0].events=[{id:14,revision:1,role:"Agent",text:"Release review",attachments:[],document:{nodes}}];seed.frames[0].retained=[14];}
const outlineState={caption:"Long release reply with a collapsed section navigator",task:"Read the reply at full width and open its section list when needed.",seed:outlineSeed,
 arrange:async context=>{await openRelease(context);await toolPosition(context);},
 verify:context=>{conversationVisible(context);context.expect(context.doc.querySelector(".reply-outline-toggle").getAttribute("aria-expanded")).toBe("false");context.expect(context.doc.querySelectorAll(".reply-contents li").length).toBe(4);}};
export const ReplySections={parameters:{desktop:outlineState}};
export const ReplySectionsOpen={parameters:{desktop:{...outlineState,caption:"Long release reply with its four-section navigator expanded",task:"Choose a section from a compact list without a permanent side column.",
 arrange:async context=>{await openRelease(context);await toolPosition(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Reply sections",exact:true}));},
 verify:context=>{conversationVisible(context);context.expect(context.doc.querySelector(".reply-outline-toggle").getAttribute("aria-expanded")).toBe("true");context.expect(context.visible(context.doc,".reply-contents ol")).toBe(true);},
}}};

export const ReplySectionSelected={parameters:{desktop:{...outlineState,caption:"Long release reply after choosing Verification results",task:"Continue reading the selected section with keyboard focus on its heading.",
 arrange:async context=>{await openRelease(context);await toolPosition(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Reply sections",exact:true}));await context.userEvent.click(context.ui.getByRole("button",{name:"Verification results",exact:true}));},
 verify:context=>{outlineState.verify(context);const h=context.ui.getByRole("heading",{name:"Verification results",exact:true}),t=context.doc.querySelector("#transcript").getBoundingClientRect(),r=h.getBoundingClientRect();context.expect(context.doc.activeElement).toBe(h);context.expect(r.top).toBeGreaterThanOrEqual(t.top);context.expect(r.bottom).toBeLessThanOrEqual(t.bottom);},
}}};

