// Editorial explanations point at captured records; they never change recorded state.
export const roadmapNotes = [
  {id:"workflow-active-workstreams", label:"Choose the work", position:"tracks", detail:"Keep current work easy to find while completed work stays available.", outcome:"An agent can choose an active area without wading through closed work."},
  {id:"workflow-reasoning-scope-ownership", label:"Read saved reasoning", position:"reasoning", detail:"Read the decisions and investigations from the session you selected.", outcome:"A new session can find the reasoning that belongs to this work."},
  {id:"workflow-agent-context-ownership", label:"Give agents the plan", position:"context", detail:"Give each running agent the objective and tasks for its own work.", outcome:"The agent starts with the right plan, including work still in progress."},
  {id:"rust-gui-current-plan", label:"Show the current plan", position:"plan", detail:"Read the current objective, requirements and ordered tasks in the desktop.", outcome:"The current work plan is available in the desktop preview."},
  {id:"rust-gui-planning-evidence", label:"Plan and inspect work", position:"next", detail:"Bring the full roadmap, reasoning and checks into the desktop planning flow.", outcome:"This broader desktop workflow was still planned when these records were captured."}
];
export const reasoningNotes = [
  {step:1740,label:"Lead with a recording",position:"recording"},
  {step:1741,label:"Let people explore the records",position:"records"},
  {step:1742,label:"Make room for both",position:"decision"}
];
export const chunkNotes = [
  {task:"demo-plan",label:"Agree on the scope",scene:0,result:"A small support inbox with a clear boundary: local mail intake, threaded conversations, search, assignment and resolution."},
  {task:"demo-store",label:"Make incoming mail reliable",scene:1,result:"Mail is saved in SQLite. Replies stay in the conversation, and delivering a message twice doesn't create a duplicate."},
  {task:"demo-api",label:"Connect the team's actions",scene:2,result:"Search, assignment and resolution use the real API. A stale save can't silently overwrite a teammate's change."},
  {task:"demo-ui",label:"Build and review the inbox",scene:4,result:"The browser uses saved data. A screen review caught small menus and confusing recovery text; both were fixed and checked again."},
  {task:"demo-verify",label:"Check the finished project",scene:5,result:"The storage and HTTP tests pass, and the browser recording shows the completed app being used."}
];
export function taskRecord(recording,task) {
  const events=recording.events.filter(event=>event.input.task_id===task && event.output?.ok!==false);
  const finish=events.findLast(event=>event.kind==="finish");
  return {events,finish,checks:events.filter(event=>event.output?.check?.value).map(event=>({event:event.id,...event.output.check.value}))};
}
export const roadmapRequirements = {
 "workflow-active-workstreams":["Show unfinished work and what is ready to start.","Keep closed work available as history.","Tell the agent when its saved focus no longer applies."],
 "workflow-reasoning-scope-ownership":["Read the complete reasoning from the selected session.","Keep missing records distinct from records that cannot be read.","Keep references to earlier decisions stable."],
 "workflow-agent-context-ownership":["Give the agent the objective, requirements and tasks for its work.","Keep separate sessions from picking up each other's plans.","Read saved work again after the conversation is shortened."],
 "rust-gui-current-plan":["Read the current objective and ordered tasks from the selected session.","Refresh when the work changes while preserving drafts.","Explain missing, unreadable or older records."],
 "rust-gui-planning-evidence":["Browse work, dependencies, reasoning and checks together.","Keep planning separate from starting implementation.","Make the original requirements and check results readable."]
};
export const dependencyNames = {
 "rust-gui-agent-supervision":"Agent permissions and stopping",
 "rust-gui-rich-conversation":"Readable conversations",
 "rust-gui-workflow-read-integrity":"Reliable project reads",
 "rust-gui-conversation-context-ownership":"Conversation context",
 "rust-gui-work-run-ownership":"Work kept with its run",
 "rust-gui-execution-history":"Work history",
 "workflow-durable-mutation-acknowledgement":"Confirmed saved changes"
};
export const roadmapEdges = [
 {from:"workflow-active-workstreams",to:"rust-gui-planning-evidence",path:"M192 60 H540 V136"},
 {from:"workflow-reasoning-scope-ownership",to:"rust-gui-current-plan",path:"M192 180 H236"},
 {from:"rust-gui-current-plan",to:"rust-gui-planning-evidence",path:"M412 180 H452"},
 {from:"workflow-agent-context-ownership",to:"rust-gui-planning-evidence",path:"M192 300 H540 V224"}
];
