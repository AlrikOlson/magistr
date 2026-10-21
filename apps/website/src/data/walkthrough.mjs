// Editorial chapters point into the immutable recording; they are not new events.
export const walkthrough = [
  {id:"brief", label:"The brief", event:11, title:"Start with a plan for the whole project.", description:"The agent records the boundaries, who owns the data, and the work to do next. Magistr keeps that plan with the project.", kind:"plan"},
  {id:"mail", label:"Mail arrives", event:17, title:"Handle repeated deliveries safely.", description:"The agent builds storage first. Replies stay together, and a repeated delivery cannot create another message. The first failed check and the passing rerun both stay in the record.", kind:"source", path:"examples/support-inbox/store.py", first:"# A writer lock", last:'return self._get(db, previous["conversation_id"]), False', check:16},
  {id:"team", label:"Work together", event:22, title:"Keep a teammate’s changes intact.", description:"Search, assignment and resolution now go through the real API. This check proves that saving an old view cannot overwrite someone else’s assignment.", kind:"source", path:"examples/support-inbox/tests/test_store.py", first:"def test_stale_assignment_cannot_overwrite_a_teammate", last:'self.assertEqual(self.store.list(assignee="Leo"), [])', check:21},
  {id:"inbox", label:"Use the inbox", event:33, title:"Open the app and do the work.", description:"The agent runs the inbox against its real database: receive mail, find a conversation, assign it, resolve it, and handle a new reply. These are captures from that run.", kind:"capture"},
  {id:"review", label:"Review & fix", event:41, title:"Passing tests still left things to fix.", description:"The screen review catches small menus and a misleading recovery message. The agent fixes both, runs the workflow again, and saves a new review in Magistr.", kind:"review"},
  {id:"finished", label:"The result", event:46, title:"A working support inbox.", description:"Follow the build from its brief to the finished app. The plan, code changes, failed checks and fixes all come from one recorded Codex session using Magistr.", kind:"video", check:44}
];

export function excerptAt(recording, scene, filesAt) {
  const source=filesAt(recording,scene.event)[scene.path];
  const lines=source.split("\n");
  const start=lines.findIndex(line=>line.includes(scene.first));
  const end=lines.findIndex((line,index)=>index>=start&&line.includes(scene.last));
  if(start<0||end<start)throw new Error("Walkthrough excerpt does not match its recorded source");
  const selected=lines.slice(start,end+1);
  const indent=Math.min(...selected.filter(line=>line.trim()).map(line=>line.length-line.trimStart().length));
  return selected.map((text,index)=>({number:start+index+1,text:text.slice(indent)}));
}
