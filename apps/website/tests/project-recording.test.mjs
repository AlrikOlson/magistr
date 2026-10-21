import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createHash} from "node:crypto";
import {filesAt,milestonesAt,checkOf,eventStatus} from "../src/data/project-recording.mjs";
const recording=JSON.parse(readFileSync(new URL("../public/demo/inbox/recording.json",import.meta.url)));
const manifest=JSON.parse(readFileSync(new URL("../public/demo/inbox/manifest.json",import.meta.url)));
const hash=bytes=>createHash("sha256").update(bytes).digest("hex");

test("the captured writes and edits reconstruct every delivered source file",()=>{
 const files=filesAt(recording,recording.events.length-1);
 assert.equal(Object.keys(files).length,9);
 for(const [path,content]of Object.entries(files)){
  const actual=readFileSync(new URL("../../../"+path,import.meta.url));
  assert.equal(content,actual.toString(),path);
  assert.equal(hash(actual),manifest.source[path.replace("examples/support-inbox/","")],path);
 }
 for(const event of recording.events.filter(event=>event.kind==="write"||event.kind==="edit")){
  const path=event.input.path||event.input.file;
  assert.equal(hash(filesAt(recording,event.id)[path]),event.output.results[0].contentHash,"Captured receipt "+event.id);
 }
});

test("playback preserves actual failures, check receipts, task completion and chronology",()=>{
 assert.equal(recording.events.length,47);
 for(const [index,event]of recording.events.entries()){
  assert.equal(event.id,index);
  assert(Date.parse(event.endedAt)>=Date.parse(event.at));
  if(index)assert(Date.parse(event.at)>=Date.parse(recording.events[index-1].endedAt));
  const check=checkOf(event);
  if(check?.verified)assert.equal(check.passed,check.exit_code===0);
 }
 assert.equal(eventStatus(recording.events[14]),"Failed");
 assert.match(checkOf(recording.events[14]).details,/No module named 'store'/);
 assert.equal(eventStatus(recording.events[16]),"Passed");
 assert.equal(milestonesAt(recording,14).find(x=>x.id==="store").status,"In progress");
 assert.equal(milestonesAt(recording,17).find(x=>x.id==="store").status,"Completed");
 assert(milestonesAt(recording,46).every(x=>x.status==="Completed"));
 assert.equal(Object.keys(filesAt(recording,0)).length,0);
 assert.equal(checkOf(recording.events[44]).receipt_id,"a1e68d0c-8fb1-484f-8d2f-5dc9f832cba5");
});

test("the downloadable recording, source archive, video and screenshots match the manifest",()=>{
 for(const [file,expected]of Object.entries(manifest.artifacts)){
  assert.equal(hash(readFileSync(new URL("../public/demo/inbox/"+file,import.meta.url))),expected,file);
 }
 assert.equal(manifest.artifacts["inbox-workflow.webm"],"baee474062f23fab399affe511afde9fd8431caadabf910a51324dfbec5c5d67");
 assert(!("projectState" in recording),"No simulated state is included");
});
test("original app captures retain their recorded bytes",()=>{
 const captures=JSON.parse(readFileSync(new URL("../public/demo/inbox/walkthrough/captures.json",import.meta.url)));
 assert.equal(captures.images.length,8);
 for(const capture of captures.images){
  const bytes=readFileSync(new URL("../public/demo/inbox/walkthrough/"+capture.file,import.meta.url));
  assert.equal(hash(bytes),capture.sha256);
  assert.equal(hash(readFileSync(new URL("../../../"+capture.source,import.meta.url))),capture.sha256);
 }
});
test("the product tour uses real dependencies, reasoning links and recorded task evidence",async()=>{
 const {roadmapNotes,roadmapEdges,reasoningNotes,chunkNotes,taskRecord}=await import("../src/data/product-tour.mjs");
 const snapshot=JSON.parse(readFileSync(new URL("../public/examples/project-records.json",import.meta.url)));
 for(const note of roadmapNotes)assert(snapshot.roadmap.some(record=>record.id===note.id));
 for(const edge of roadmapEdges){
  assert(snapshot.roadmap.find(record=>record.id===edge.to).deps.includes(edge.from),"Displayed dependency exists in the captured roadmap");
 }
 const conclusion=snapshot.reasoning.find(step=>step.step_number===1742);
 assert(conclusion.pinned);
 for(const note of reasoningNotes)assert(snapshot.reasoning.some(step=>step.step_number===note.step));
 for(const branch of snapshot.branches){
  assert.equal(branch.status,"merged");
  assert.equal(branch.merged_into,conclusion.step_number);
  const step=snapshot.reasoning.find(step=>step.branch_id===branch.id);
  assert.equal(step.branch_from,branch.from_step);
  assert(conclusion.dependencies.includes(step.step_number));
 }

 for(const note of chunkNotes){
  const task=taskRecord(recording,note.task);
  assert.equal(task.finish.output.status,"completed");
  assert.equal(recording.events[note.event].input.task_id,note.task);
  assert(task.checks.length>0);
 }
 const storage=taskRecord(recording,"demo-store");
 assert.deepEqual(storage.checks.map(check=>check.passed),[false,true]);
});
test("one chronological session cursor never exposes future source or completion",async()=>{
 const {sessionEntries,projectAt,moments,momentIndex}=await import("../src/data/session-replay.mjs");
 const entries=sessionEntries(recording);
 assert.equal(entries.length,53);
 assert.deepEqual(entries.filter(e=>e.kind==="action").map(e=>e.event.id),recording.events.map(e=>e.id));
 assert.deepEqual(entries.filter(e=>e.kind==="message").map(e=>e.text),recording.messages.map(e=>e.text));
 for(const [i,entry]of entries.entries()){
  if(i)assert(Date.parse(entry.at)>=Date.parse(entries[i-1].at));
  const state=projectAt(recording,entry);
  for(const check of state.checks)assert(check.id<=entry.eventIndex);
  for(const task of state.tasks)if(task.status==="Completed")assert(recording.events.slice(0,entry.eventIndex+1).some(e=>e.kind==="finish"&&e.input.task_id===task.task));
 }
 const beforeStore=entries.find(e=>e.event?.id===14),afterStore=entries.find(e=>e.event?.id===15);
 assert(!("examples/support-inbox/store.py" in filesAt(recording,beforeStore.eventIndex)));
 assert("examples/support-inbox/store.py" in filesAt(recording,afterStore.eventIndex));
 assert.equal(projectAt(recording,entries[0]).tasks.length,0);
 assert(moments.every(moment=>momentIndex(entries,moment)>=0));
});

test("publication excludes private metadata and preserves the reviewed conversation",async()=>{
 const {publishRecording,scrubText}=await import("../scripts/publish-recording.mjs");
 assert.deepEqual(publishRecording(recording,recording.messages),recording,"The publication allowlist is idempotent");
 const sensitive=structuredClone(recording);
 sensitive.private_session="must not ship";
 sensitive.events[0].input.private_account="must not ship";
 sensitive.events[0].output.source={inode:123,home:"/Users/private-person"};
 const extra={at:recording.messages[0].at,role:"assistant",channel:"analysis",text:"must not ship"};
 const result=publishRecording(sensitive,[...recording.messages,extra,{at:"2026-10-22T00:00:00Z",role:"user",text:"unrelated message"}]);
 assert.equal(JSON.stringify(result).includes("must not ship"),false);
 assert.equal(result.messages.length,6);
 assert.equal(result.messages[0].text,"Record a real project being built from start to finish.");
 assert.equal(result.messages.filter(message=>message.presentation==="paraphrased").length,2);
 assert.equal(result.messages[2].role,"assistant");
 assert.equal(scrubText("/Users/example/Code/magistr/example.py /home/someone/file"),"[project]/example.py [home]/file");
 assert(!JSON.stringify(recording).match(/\/Users\/|\/home\/|session_id|call_id|inode|source_revision/));
 assert.equal(scrubText("Authorization: Bearer fake-secret-token"),"Authorization: Bearer [redacted credential]");
});