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
test("guided chapters keep their source and screenshots tied to the original build",async()=>{
 const {walkthrough,excerptAt}=await import("../src/data/walkthrough.mjs");
 assert.deepEqual(walkthrough.map(scene=>scene.event),[11,17,22,33,41,46]);
 for(const scene of walkthrough.filter(scene=>scene.kind==="source")){
  const excerpt=excerptAt(recording,scene,filesAt);
  const lines=filesAt(recording,scene.event)[scene.path].split("\n");
  assert(excerpt.length>4);
  for(const line of excerpt)assert.equal(line.text.trim(),lines[line.number-1].trim());
  assert.equal(checkOf(recording.events[scene.check]).passed,true);
 }
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
 const {walkthrough}=await import("../src/data/walkthrough.mjs");
 for(const note of chunkNotes){
  const task=taskRecord(recording,note.task);
  assert.equal(task.finish.output.status,"completed");
  assert(walkthrough[note.scene].event>=task.events[0].id);
  assert(task.checks.length>0);
 }
 const storage=taskRecord(recording,"demo-store");
 assert.deepEqual(storage.checks.map(check=>check.passed),[false,true]);
});
