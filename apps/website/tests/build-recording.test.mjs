import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createHash} from "node:crypto";
import {execFileSync} from "node:child_process";
import {stateAt,chapters,minutes,walkthroughEnd} from "../src/data/build-recording.mjs";
import {explanations,explanationFor,explanationHTML} from "../src/data/build-explanations.mjs";
const dir=new URL("../public/demo/tic-tac-toe/",import.meta.url),recording=JSON.parse(readFileSync(new URL("recording.json",dir)));
const manifest=()=>JSON.parse(readFileSync(new URL("manifest.json",dir)));
const hash=b=>createHash("sha256").update(b).digest("hex");
test("publication contains conversation without private paths or credentials",()=>{
 const text=JSON.stringify(recording);
 for(const pattern of [/\/Users\//,/\.codex\/sessions/,/\.claude\/projects/,/sk-[a-zA-Z0-9]{16,}/,/Bearer\s+[a-zA-Z0-9]/,/alrik@/i])assert(!pattern.test(text),String(pattern));
 assert.equal(recording.project.engine,"Bevy 0.20.0");
 const messages=recording.events.filter(e=>e.kind==="message");
 assert.equal(messages.length,recording.provenance.conversation.messages);
 assert.equal(messages.filter(e=>e.role==="user").length,2);
 assert.equal(messages.filter(e=>e.role==="assistant").length,31);
 assert.equal(recording.events[0].role,"user");assert.equal(recording.events.at(-1).source.phase,"final_answer");
 assert.equal(stateAt(recording,0).steps.length,0);
 assert(messages.every(e=>e.output&&/^[a-f0-9]{64}$/.test(e.source.sha256)));
});
test("the cursor cannot reveal future records, checks or file contents",()=>{
 for(let i=0;i<recording.events.length;i++){
  const state=stateAt(recording,i),past=recording.events.slice(0,i+1);
  for(const s of state.steps)assert(past.some(e=>e.step?.step_number===s.step_number));
  for(const c of state.checks)assert(past.some(e=>e.id===c.eventId));
  for(const f of state.files)assert(past.some(e=>e.file?.path===f.path&&e.file.content===f.content));
 }
 assert.equal(stateAt(recording,0).checks.length,0);
});
test("the completed project has actual prerequisite relationships and retained failed checks",()=>{
 const state=stateAt(recording,recording.events.length-1),ids=new Set(state.chunks.map(c=>c.id));
 assert.equal(state.chunks.length,4);
 assert(state.chunks.every(c=>c.status==="done"));
 for(const c of state.chunks)for(const dep of c.deps)assert(ids.has(dep));
 assert.equal(state.chunks.find(c=>c.id==="example-ttt-rematch").deps.length,2);
 assert(state.checks.some(c=>c.passed===false&&c.verified));
 assert(state.checks.some(c=>c.passed===true&&c.name==="Rules, opponent and rematches"));
 const steps=new Set(state.steps.map(s=>s.step_number));
 for(const b of state.branches){assert(steps.has(b.from_step));if(b.status==="merged")assert(steps.has(b.merged_into));}
});
test("every published action has a concise explanation, with no raw response fallback",()=>{
 assert.deepEqual(Object.keys(explanations).sort(),recording.events.map(e=>e.id).sort());
 for(const event of recording.events){
  const explanation=explanationFor(event.id),copy=JSON.stringify(explanation);
  assert(explanation.title&&explanation.summary,event.id);
  assert(explanation.details.length<=2,event.id+" has at most two supporting points");
  assert(!/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f-]{23}|ttt-(rules|opponent|board|rematch)|required gate\(s\)|current tree|step 175\d/.test(copy),event.id+" exposes internal log language");
  assert(copy.split(/\s+/).length<100,event.id+" is short enough to scan");
 }
 assert.throws(()=>explanationFor("new-action"),/Missing recording explanation/);
 assert.throws(()=>explanationFor("__proto__"),/Missing recording explanation/);
});
test("explanations format labels and filenames without allowing markup from text",()=>{
 const html=explanationHTML({summary:'Read `Cargo.lock` and <script>alert(1)</script>.',details:[["Next","Run it again."]]});
 assert(html.includes("<code>Cargo.lock</code>"));
 assert(html.includes("<dt>Next</dt><dd>Run it again.</dd>"));
 assert(!html.includes("<script>"));assert(html.includes("&lt;script&gt;"));
});
test("the walkthrough ends with the last recorded project action; the later conversation stays in the full session",()=>{
 const end=walkthroughEnd(recording);
 assert.equal(minutes(recording,end),Math.round((Date.parse(recording.events[end].at)-Date.parse(recording.events[0].at))/60000));
 assert(end>0&&end<recording.events.length-1);
 assert.notEqual(recording.events[end].kind,"message");
 assert(recording.events.slice(end+1).every(e=>e.kind==="message"),"only conversation follows the finished game");
 assert.equal(stateAt(recording,end).chunks.filter(c=>c.status==="done").length,4,"every planned piece is finished at the end of the walkthrough");
 assert(chapters(recording).every(p=>p.index<=end));
});
test("chapters open at the start of each phase, in order, inside the walkthrough",()=>{
 const parts=chapters(recording);assert.equal(parts.length,6);assert.equal(parts[0].index,0);
 assert(parts.every(p=>p.index>=0&&p.index<=walkthroughEnd(recording)));
 assert(parts.every(p=>p.label&&p.note));
 for(const [i,kind] of [[1,"roadmap"],[2,"roadmap"],[3,"roadmap"],[4,"roadmap"]])assert.equal(recording.events[parts[i].index].operation,"roadmap_start_chunk",parts[i].label+" opens when its piece starts ("+kind+")");
 assert.equal(recording.events[parts[5].index].operation,"roadmap_complete_chunk","the last chapter is the finished game");
 assert(parts.every((p,i)=>i===0||p.index>parts[i-1].index));
 for(let i=1;i<recording.events.length;i++)assert(recording.events[i].at>=recording.events[i-1].at);
 assert.equal(new Set(recording.events.map(e=>e.id)).size,recording.events.length);
});
test("download and captures match the recorded manifest; source archive is allowlisted",()=>{
 const m=manifest();for(const asset of m.assets)assert.equal(hash(readFileSync(new URL(asset.path,dir))),asset.sha256,asset.path);
 for(const source of m.sources)assert.equal(hash(readFileSync(new URL("../../../examples/bevy-tic-tac-toe/"+source.path,import.meta.url))),source.sha256,source.path);
 const entries=JSON.parse(execFileSync("python3",["-c","import zipfile,sys,json; z=zipfile.ZipFile(sys.argv[1]); print(json.dumps(z.namelist()))",new URL("source.zip",dir).pathname],{encoding:"utf8"}));
 assert.deepEqual(entries,m.sources.map(s=>"tic-tac-toe/"+s.path));
});

test("conversation import includes every visible turn once and excludes internal messages",async()=>{
 const {conversationMessages,auditConversation}=await import("../scripts/audit-conversation.mjs");
 const make=(role,phase,text,type="response_item")=>JSON.stringify({timestamp:"2026-10-21T08:00:00Z",type,payload:{type:"message",role,phase,content:[{type:"output_text",text}]}});
 const jsonl=[make("user",null,"A personal request"),make("assistant","analysis","Private deliberation"),make("system",null,"System instructions"),make("assistant","commentary","Starting now."),make("assistant","final_answer","Finished."),make("assistant","commentary","Duplicate","event_msg")].join("\n");
 const bounds={start:"2026-10-21T07:00:00Z",end:"2026-10-21T09:00:00Z",messages:3};
 const source=conversationMessages(jsonl,bounds);
 assert.deepEqual(source.map(m=>m.role),["user","assistant","assistant"]);
 const events=source.map(m=>({kind:"message",role:m.role,at:m.at,source:{phase:m.phase,sha256:m.sha256},output:m.role==="user"?"A reviewed request":m.text}));
 assert.equal(auditConversation({events,provenance:{conversation:bounds}},jsonl).messages,3);
 assert.throws(()=>auditConversation({events:events.slice(1),provenance:{conversation:bounds}},jsonl),/Every visible message/);
});
