import {readFileSync} from "node:fs";
import {createHash} from "node:crypto";
import {pathToFileURL} from "node:url";
import assert from "node:assert/strict";

// Visible conversation appears once per response item. Hidden analysis, tool
// payloads, system instructions and compacted history are not conversation.
export function conversationMessages(jsonl,{start,end}){
 return jsonl.split("\n").filter(Boolean).map(line=>JSON.parse(line)).filter(entry=>{
  const p=entry.payload;
  return entry.timestamp>=start&&entry.timestamp<=end&&entry.type==="response_item"&&p?.type==="message"&&
   (p.role==="user"||(p.role==="assistant"&&["commentary","final","final_answer"].includes(p.phase??p.channel)));
 }).map(entry=>{
  const p=entry.payload,text=p.content.filter(c=>["input_text","output_text"].includes(c.type)).map(c=>c.text).join("\n");
  return {at:entry.timestamp,role:p.role,phase:p.phase??p.channel??"user",text,sha256:createHash("sha256").update(text).digest("hex")};
 });
}
export function auditConversation(recording,jsonl){
 const source=conversationMessages(jsonl,recording.provenance.conversation);
 const published=recording.events.filter(e=>e.kind==="message");
 assert.equal(source.length,recording.provenance.conversation.messages);
 assert.equal(published.length,source.length,"Every visible message in the build must be published");
 for(let i=0;i<source.length;i++){
  const s=source[i],p=published[i];
  assert.deepEqual([p.at,p.role,p.source.sha256,p.source.phase],[s.at,s.role,s.sha256,s.phase]);
  assert(p.output.trim(),"Messages cannot disappear into empty placeholders");
  if(s.role==="user")assert.notEqual(p.output,s.text,"Personal prompts must be rewritten before publication");
  else assert.equal(p.output,s.text.replace(/\/Users\/[^/]+\/Code\/magistr\/docs\//g,"https://github.com/AlrikOlson/magistr/blob/723b7794/docs/").trim());
 }
 return {messages:source.length,users:source.filter(m=>m.role==="user").length,assistant:source.filter(m=>m.role==="assistant").length};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const recording=JSON.parse(readFileSync(new URL("../public/demo/tic-tac-toe/recording.json",import.meta.url),"utf8"));
 assert(process.argv[2],"Pass the original local Codex session file; it is never copied or published.");
 console.log(JSON.stringify(auditConversation(recording,readFileSync(process.argv[2],"utf8"))));
}
