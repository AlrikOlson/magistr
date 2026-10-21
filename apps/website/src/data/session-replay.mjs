import {checkOf,eventStatus,eventText,milestonesAt} from "./project-recording.mjs";

// One cursor owns conversation, source snapshots, checks and task progress.
export function sessionEntries(recording) {
 const entries=[
  ...recording.messages.map(message=>({...message,kind:"message"})),
  ...recording.events.map(event=>({id:"action-"+event.id,kind:"action",event,at:event.endedAt}))
 ].sort((a,b)=>Date.parse(a.at)-Date.parse(b.at));
 let eventIndex=-1;
 return entries.map(entry=>{
  if(entry.kind==="action")eventIndex=entry.event.id;
  return {...entry,eventIndex};
 });
}
export const moments=[
 {label:"Start",message:"message-2"},
 {label:"Plan",event:8},
 {label:"Storage",event:14},
 {label:"API",event:21},
 {label:"Review",event:34},
 {label:"Finish",event:46}
];
export function momentIndex(entries,moment) {
 return entries.findIndex(entry=>moment.message?entry.id===moment.message:entry.event?.id===moment.event);
}
export function timeLabel(entries,index) {
 const seconds=Math.max(0,Math.floor((Date.parse(entries[index].at)-Date.parse(entries[0].at))/1000));
 return Math.floor(seconds/60)+":"+String(seconds%60).padStart(2,"0");
}
export function entryTitle(entry) {
 if(entry.kind==="message")return entry.role==="user"?"You":"Codex";
 const event=entry.event;
 if(event.kind==="write")return "Wrote "+event.input.path.replace("examples/support-inbox/","");
 if(event.kind==="edit")return "Edited "+event.input.file.replace("examples/support-inbox/","");
 return "Called magistr."+event.operation;
}
export function entryBody(entry) {
 return entry.kind==="message"?entry.text:eventText(entry.event);
}
export function entryPreview(entry) {
 const body=entryBody(entry)||"",lines=body.split("\n"),total=lines.length;
 if(entry.kind==="message"){
  const text=entry.id==="message-2"?body.slice(body.indexOf("I’ll use a shared support inbox")):body;
  return {text,total};
 }
 if(entry.event.kind==="decision"){
  const text=entry.event.input.thought.match(/^(?:[^.]+\.){1,3}/)?.[0]||entry.event.input.outcome;
  return {text,total,label:"From the saved decision"};
 }
 const check=checkOf(entry.event);
 if(check?.command && lines.length>8){
  const count=check.passed?5:8;
  const text=lines.slice(-count).filter(line=>!/^[-=]{4,}$/.test(line)).join("\n").replace(/\n{3,}/g,"\n\n").trim();
  return {text,total,label:"End of recorded output",omitted:total-count};
 }
 if(lines.length>10)return {text:lines.slice(0,10).join("\n"),total,omitted:total-10};
 return {text:body,total};
}
export function projectAt(recording,entry) {
 const index=entry.eventIndex;
 const tasks=milestonesAt(recording,index).filter(task=>recording.events.slice(0,index+1).some(event=>event.operation==="plan_work"&&event.input.task_id===task.task));
 const checks=recording.events.slice(0,index+1).filter(checkOf);
 return {
  tasks,checks,
  decision:recording.events.slice(0,index+1).find(event=>event.kind==="decision"),
  status:entry.kind==="message"?"Message":eventStatus(entry.event)
 };
}