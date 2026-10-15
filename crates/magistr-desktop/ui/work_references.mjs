import {element} from "./work_dom.mjs";
import {readState} from "./work_records.mjs";
export function resolveReference(snapshot,reference){
  const colon=reference.indexOf(":"),kind=reference.slice(0,colon),id=reference.slice(colon+1);
  const family={chunk:"roadmap",think:"think",signal:"signals"}[kind];
  if(!family)return {message:"This reference does not identify a saved record that can be opened here."};
  const observed=readState(snapshot[family]);
  if(!observed.value)return {message:"The related records cannot be read. Use Refresh to try again."};
  let target;
  if(kind==="chunk"){const record=observed.value.chunks.find(record=>record.id===id);if(record)target={kind,id,title:record.title};}
  if(kind==="signal"){const record=observed.value.find(record=>record.id===id);if(record)target={kind,id,title:record.content?.summary||record.body.slice(0,180)};}
  if(kind==="think")for(const history of observed.value.sessions){const step=history.steps.find(step=>String(step.step_number)===id);if(step){target={kind,id:step.record_id,session:JSON.stringify(history.session_id??null),title:step.purpose};break;}}
  return target?{target,stale:!observed.current}:{message:"The related record is no longer available."};
}
export function renderReferences(references,{snapshot,navigate,copy},label="Related work"){
  if(!references.length)return null;
  const section=element("section","","roadmap-relations");section.setAttribute("aria-label",label);section.append(element("h4",label));
  const list=element("ul");
  for(const reference of references){
    const row=element("li"),resolved=resolveReference(snapshot,reference);
    if(resolved.target){
      const button=element("button",resolved.target.title);button.dataset.focusKey="reference:"+reference;button.onclick=()=>navigate(resolved.target,button.dataset.focusKey);row.append(button);
      if(resolved.stale)row.append(element("span","From the last saved records","roadmap-relation-state"));
    }else row.append(element("p",resolved.message,"work-record-warning"));
    const copyButton=element("button","Copy reference");copyButton.onclick=()=>copy(reference);row.append(copyButton);list.append(row);
  }
  section.append(list);return section;
}
