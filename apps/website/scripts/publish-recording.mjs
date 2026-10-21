// Publication is an explicit allowlist. Never ship a raw Codex rollout.
const pick = (value, keys) => Object.fromEntries(keys.filter(key => value?.[key] !== undefined).map(key => [key, value[key]]));
export function scrubText(text) {
 return text
  .replace(/\/(?:Users|home)\/[^/\s"'\\]+\/Code\/magistr(?=[/\s"'\\]|$)/g, "[project]")
  .replace(/\/(?:Users|home)\/[^/\s"'\\]+/g, "[home]")
  .replace(/\b(?:sk-(?:proj-|live-|test-)?[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9_]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[A-Z0-9]{16})\b/g, "[redacted credential]")
  .replace(/(authorization\s*[:=]\s*["']?(?:Bearer|Basic)\s+)[^\s"']+/gi, "$1[redacted credential]")
  .replace(/((?:api[_-]?key|access[_-]?token|client[_-]?secret|password)\s*[:=]\s*["']?)[A-Za-z0-9_./+=-]{12,}/gi, "$1[redacted credential]");
}
function scrub(value) {
 if(typeof value === "string") return scrubText(value);
 if(Array.isArray(value)) return value.map(scrub);
 if(value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key,item])=>[key,scrub(item)]));
 return value;
}
export function publishRecording(recording, messages) {
 const events=recording.events.map(event=>{
  const input=pick(event.input,["action","task_id","task_type","title","reason","path","content","file","old","new","purpose","context","thought","outcome","rationale","next_action","name","type","command","passed","details","required","artifacts"]);
  let output={};
  if(event.output?.ok===false) output={ok:false,receipts:[{result:{message:event.output.receipts[0].result.message}}]};
  else if(event.output?.check?.value) output={check:{value:pick(event.output.check.value,["receipt_id","type","name","passed","details","required","verified","command","exit_code","timestamp"])}};
  else if(event.output?.results) output={ok:true,results:event.output.results.map(row=>pick(row,["path","action","applied","contentHash"]))};
  else output=pick(event.output,["id","title","status"]);
  return scrub({...pick(event,["id","chapter","at","endedAt","kind","title","operation"]),input,output});
 });
 const allowedTimes=new Set([
  "2026-10-21T04:48:06.968Z","2026-10-21T04:48:06.975Z",
  "2026-10-21T04:48:09.908Z","2026-10-21T04:48:54.761Z",
  "2026-10-21T04:51:18.222Z","2026-10-21T04:53:36.636Z"
 ]);
 const paraphrases=new Map([
  ["2026-10-21T04:48:06.968Z","Record a real project being built from start to finish."],
  ["2026-10-21T04:48:06.975Z","Let visitors scrub through the recording. Use a realistic project with a few moving parts, small enough to follow from start to finish."]
 ]);
 const excerpt=messages.filter(message=>allowedTimes.has(message.at)&&["user","assistant"].includes(message.role)&&!message.channel)
  .map((message,index)=>scrub({id:"message-"+index,...pick(message,["at","role","text"]),...(message.role==="user"?{text:paraphrases.get(message.at),presentation:"paraphrased"}:{})}));
 if(excerpt.length!==6)throw new Error("The six reviewed public conversation messages are required.");
 return {
  schemaVersion:2,project:"Support inbox",agent:"Codex",
  startedAt:recording.startedAt,endedAt:recording.endedAt,
  description:"A real support-inbox implementation. The conversation comes from the matching local Codex session. User requests are paraphrased for publication; agent replies and tool results retain their recorded wording.",
  publication:{
   format:"Reconstructed session; not a terminal screen recording.",
   scope:"Two paraphrased user requests, four recorded agent messages and 47 implementation actions from one build. Unrelated setup and website work are omitted.",
   redactions:["User requests are rewritten with the owner’s permission; the page uses ordinary speaker labels.","Personal filesystem prefixes replaced with [project] or [home].","Raw session identifiers, machine metadata, system instructions, private reasoning and unrelated conversation omitted.","Only the recorded project decision is included as saved reasoning; no private model reasoning is published."],
   source:"Local Codex session and its captured Magistr tool actions"
  },
  messages:excerpt,events
 };
}