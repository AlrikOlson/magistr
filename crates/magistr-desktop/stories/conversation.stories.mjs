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
