import {element} from "./work_dom.mjs";
import {readState} from "./work_records.mjs";
import {renderSourceDetail} from "./source_details.mjs";
const families=[["ship","Work","work"],["roadmap","Roadmap","roadmap"],["think","Reasoning","reasoning"],["signals","Feedback","feedback"]];
function counts(kind,value){
  if(kind==="ship"){const runs=[...(value.current?[value.current]:[]),...value.retained],tasks=runs.flatMap(run=>run.tasks);return [["Current work",value.current?1:0],["Saved work",value.retained.length],["Tasks",tasks.length],["Check attempts",tasks.reduce((sum,task)=>sum+task.checks.length,0)]];}
  if(kind==="roadmap")return [["Roadmap items",value.chunks.length]];
  if(kind==="signals")return [["Feedback items",value.length]];
  return [["Reasoning histories",value.sessions.length],["Recorded steps",value.sessions.reduce((sum,history)=>sum+history.steps.length,0)],["Investigations",value.sessions.reduce((sum,history)=>sum+(history.branches?.length??0),0)]];
}
function savedTime(source){
  if(source?.kind!=="file")return null;
  // StoreStamp.modified is Serde's SystemTime representation, not the read time.
  const stamp=source.stamp.modified,millis=stamp?.secs_since_epoch*1000+(stamp?.nanos_since_epoch??0)/1e6;
  const date=new Date(millis);return Number.isNaN(date.getTime())?null:date;
}
export function renderRecordStatus(snapshot,{copy,navigate}){
  const root=element("article","","work-record work-status"),title=element("h3","Record status");title.tabIndex=-1;title.dataset.focusKey="record-status-title";
  root.append(title,element("p","Each part of the project is read separately. Use Refresh to read the records again."));
  for(const [kind,label,area] of families){
    const record=snapshot[kind],observed=readState(record),section=element("section","","work-status-family");section.setAttribute("aria-label",label+" records");section.dataset.current=String(observed.current);
    const state=record?.state??"unobserved",missing=observed.source?.kind==="missing";
    const heading=element("header"),status=state==="current"?(missing?"No saved records":"Read successfully"):state==="stale"?"Last known records":state==="unavailable"?"Could not read records":"Not read yet";
    heading.append(element("h4",label),element("p",status,"work-status-result"));section.append(heading);
    if(observed.value){
      if(observed.source?.kind==="memory")section.append(element("p","Read from the running project.","work-status-source"));
      if(observed.source?.kind==="file")section.append(element("p","Read from saved project records.","work-status-source"));
      const facts=element("dl","","roadmap-content-facts");
      for(const [name,total] of counts(kind,observed.value)){const pair=element("div");pair.append(element("dt",name),element("dd",String(total)));facts.append(pair);}
      const modified=savedTime(observed.source);
      if(modified){const pair=element("div"),time=element("time",modified.toLocaleString()),value=element("dd");time.dateTime=modified.toISOString();value.append(time);pair.append(element("dt","Saved records last changed"),value);facts.append(pair);}
      section.append(facts);
    }
    const error=state==="stale"?record.data.error:state==="unavailable"?record.data:null;
    if(error){const detail=element("section");renderSourceDetail(detail,{label:label+" read error",source:error.message},copy);section.append(detail,element("p","Use Refresh to try again.","work-status-source"));}
    if(state==="unobserved")section.append(element("p","These records have not been read yet. Use Refresh to try again."));
    const open=element("button","Open "+label.toLowerCase());open.dataset.focusKey="record-status-"+area;open.onclick=()=>navigate(area);section.append(open);root.append(section);
  }
  return root;
}
export function recordStatusMessage(snapshot){return families.every(([kind])=>snapshot[kind]?.state==="current")?"All four record groups were read successfully.":"Some records need another read.";}
