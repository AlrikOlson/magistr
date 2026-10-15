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
