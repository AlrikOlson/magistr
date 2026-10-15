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

const attachmentSeed=seed=>{seed.frames[0].attachments=[{id:"notes",name:"release-notes.md",size:820,path:"docs/release-notes.md",content_hash:"notes",excerpt:"# Release checklist\n\nVerify reopening and unsent messages."},{id:"screen",name:"app-icon.png",size:32768,path:"ui/icon.png",content_hash:"screen",preview:"/desktop-assets/icon.png"}];};
const attachmentState={caption:"Release conversation with a text file and image staged beside its draft",task:"Check attached filenames, sizes and previews before sending.",seed:attachmentSeed,arrange:openRelease,
 verify:context=>{conversationVisible(context);context.expect(context.doc.querySelectorAll("#composer .attachment").length).toBe(2);context.expect(context.visible(context.doc,'[aria-label="Remove release-notes.md"]')).toBe(true);}};
export const StagedFiles={parameters:{desktop:attachmentState}};
export const FilePreviewOpen={parameters:{desktop:{...attachmentState,caption:"Release conversation with the staged text file preview expanded",task:"Read an attached text file without losing the draft or removal control.",
 arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByLabelText("Preview release-notes.md",{exact:true}));},
 verify:context=>{attachmentState.verify(context);context.expect(context.doc.querySelector("#composer .attachment details").open).toBe(true);},
}}};

async function showAgent(context){await openRelease(context);await context.userEvent.click(context.doc.querySelector("#toggle-details"));await context.waitFor(()=>context.expect(context.doc.querySelector("#session-detail").getClientRects().length).toBeGreaterThan(0));await new Promise(resolve=>context.window.requestAnimationFrame(()=>context.window.requestAnimationFrame(resolve)));context.doc.querySelector('#session-detail section[aria-label="Agent settings"]').scrollIntoView({block:"start"});}
const agentState={caption:"Release conversation details showing agent configuration and reported usage",task:"Check the chosen agent, model, context use and spending.",arrange:showAgent,
 verify:({doc,expect,visible})=>{expect(visible(doc,"#session-detail")).toBe(true);expect(doc.querySelector("#agent-provider").textContent).toBe("GitHub Copilot");expect(doc.querySelector("#context-value").textContent).toBe("9%");expect(doc.querySelector("#spending-value").getBoundingClientRect().bottom).toBeLessThanOrEqual(doc.defaultView.innerHeight);}};
export const AgentDetails={parameters:{desktop:agentState}};
export const UsageUnavailable={parameters:{desktop:{...agentState,caption:"Conversation details when the agent has not reported usage",task:"Distinguish unavailable usage from zero usage.",seed:seed=>{seed.frames[0].usage=null;},
 verify:({doc,expect,visible})=>{expect(visible(doc,"#session-detail")).toBe(true);expect(doc.querySelector("#context-value").textContent).toBe("Not reported");expect(doc.querySelector("#context-meter").hidden).toBe(true);expect(doc.querySelector("#spending-value").getBoundingClientRect().bottom).toBeLessThanOrEqual(doc.defaultView.innerHeight);},
}}};

export const Settings={parameters:{desktop:{
 caption:"Settings dialog showing appearance choices and reduced motion",
 task:"Choose an appearance or adjust motion without a long introduction.",
 arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Settings",exact:true}));},
 verify:({doc,expect,visible})=>{expect(doc.querySelector("#settings").open).toBe(true);expect(doc.querySelectorAll(".appearance-option").length).toBe(3);expect(visible(doc,"#settings-close")).toBe(true);},
}}};

export const KeyboardShortcuts={parameters:{desktop:{...Settings.parameters.desktop,
 caption:"Settings dialog scrolled to keyboard shortcuts",
 task:"Look up the commands for finding conversations and moving between areas.",
 arrange:async context=>{await Settings.parameters.desktop.arrange(context);context.doc.querySelector(".settings-shortcuts").scrollIntoView({block:"start"});},
 verify:context=>{Settings.parameters.desktop.verify(context);const r=context.doc.querySelector(".settings-shortcuts").getBoundingClientRect();context.expect(r.top).toBeGreaterThanOrEqual(0);context.expect(r.bottom).toBeLessThan(context.doc.querySelector("#settings-close").getBoundingClientRect().top);},
}}};
export const SavedHistory={parameters:{desktop:{
 caption:"Saved conversations with the release conversation selected and its unsent message",
 task:"Choose a saved conversation and review its unsent message and details.",
 arrange:async context=>{await context.userEvent.click(context.ui.getByRole("button",{name:"Saved conversations",exact:true}));await context.waitFor(()=>context.expect(context.doc.querySelector("#history-summary").hidden).toBe(false));},
 verify:({doc,expect,visible})=>{expect(doc.querySelector("#history-picker").open).toBe(true);expect(visible(doc,"#history-name")).toBe(true);expect(doc.querySelector("#saved-draft").open).toBe(true);},
}}};
export const SavedReply={parameters:{desktop:{...SavedHistory.parameters.desktop,
 caption:"Saved conversations with the sync review selected and its last reply",
 task:"Read the last reply from a previous conversation before resuming.",
 arrange:async context=>{await SavedHistory.parameters.desktop.arrange(context);await context.userEvent.click(context.doc.querySelectorAll("#history-list>button")[1]);await context.waitFor(()=>context.expect(context.doc.querySelector("#history-name").textContent).toBe("Review the sync retry"));context.doc.querySelector("#history-text").scrollIntoView({block:"nearest"});},
 verify:({doc,expect})=>{expect(doc.querySelector("#history-name").textContent).toBe("Review the sync retry");expect(doc.querySelector("#saved-draft").open).toBe(false);expect(doc.querySelector("#history-text").getBoundingClientRect().bottom).toBeLessThanOrEqual(doc.querySelector(".history-footer").getBoundingClientRect().top);},
}}};
export const Finder={parameters:{desktop:{
 caption:"Conversation finder grouped by Magistr and Website",
 task:"Choose an open conversation using its project and current state.",
 arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Find a conversation",exact:true}));},
 verify:({doc,expect})=>{expect(doc.querySelector("#switch-picker").open).toBe(true);expect(doc.querySelectorAll(".switch-group").length).toBe(2);expect(doc.querySelector("#switch-results").textContent).toContain("Investigate sync failure");},
}}};
export const FinderMatch={parameters:{desktop:{...Finder.parameters.desktop,
 caption:"Conversation finder filtered to the sync conversation",
 task:"Find a named conversation with a highlighted match.",
 arrange:async context=>{await Finder.parameters.desktop.arrange(context);await context.userEvent.type(context.doc.querySelector("#switch-search"),"sync");},
 verify:({doc,expect})=>{expect(doc.querySelectorAll("#switch-results button").length).toBe(1);expect(doc.querySelector("#switch-results mark").textContent).toBe("sync");},
}}};
export const OpenFolder={parameters:{desktop:{
 caption:"Open project dialog with its folder field focused",
 task:"Enter or browse to a project folder.",
 arrange:async context=>{await context.userEvent.click(context.ui.getByRole("button",{name:"Open project",exact:true}));},
 verify:({doc,expect})=>{expect(doc.querySelector("#project-picker").open).toBe(true);expect(doc.activeElement).toBe(doc.querySelector("#project-folder"));expect(doc.querySelector("#picker-intro").hidden).toBe(true);},
}}};
export const SetupReview={parameters:{desktop:{
 caption:"Project setup review with AGENTS.md changes selected for reading",
 task:"Read a proposed file change before applying selected files.",
 arrange:async context=>{await OpenFolder.parameters.desktop.arrange(context);await context.userEvent.type(context.doc.querySelector("#project-folder"),"/Projects/new-project");await context.userEvent.click(context.doc.querySelector("#picker-submit"));await context.waitFor(()=>context.expect(context.doc.querySelector("#setup-preview").hidden).toBe(false));await context.userEvent.click(context.ui.getByRole("button",{name:"Read AGENTS.md",exact:true}));},
 verify:({doc,expect})=>{expect(doc.querySelector("#picker-title").textContent).toBe("Review project setup");expect(doc.querySelector("#setup-change h3").textContent).toBe("AGENTS.md");expect(doc.querySelector("#setup-count").textContent).toBe("12 of 12 changes selected");},
}}};
export const FolderFailure={parameters:{desktop:{
 caption:"Open project dialog after a folder could not be read",
 task:"Read a folder error and retry with the selected path preserved.",
 arrange:async context=>{context.window.__fixture.handlers.preview_project=()=>{throw "This folder is not readable. Check its permissions and try again.";};await OpenFolder.parameters.desktop.arrange(context);await context.userEvent.type(context.doc.querySelector("#project-folder"),"/Projects/new-project");await context.userEvent.click(context.doc.querySelector("#picker-submit"));},
 verify:({doc,expect})=>{expect(doc.querySelector("#picker-failure").hidden).toBe(false);expect(doc.querySelector("#picker-submit").textContent).toBe("Try again");expect(doc.querySelector("#project-folder").value).toBe("/Projects/new-project");},
}}};
export const SetupFileChanges={parameters:{desktop:{...SetupReview.parameters.desktop,
 caption:"Project setup review scrolled to the complete AGENTS.md changes",
 task:"Read the source lines before applying selected files.",
 arrange:async context=>{await SetupReview.parameters.desktop.arrange(context);context.doc.querySelector(".setup-code-sheet").scrollIntoView({block:"nearest"});},
 verify:context=>{SetupReview.parameters.desktop.verify(context);context.expect(context.doc.querySelector(".setup-code-sheet").getBoundingClientRect().bottom).toBeLessThanOrEqual(context.doc.querySelector(".picker-footer").getBoundingClientRect().top);},
}}};
