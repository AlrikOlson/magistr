import {element} from "./work_dom.mjs";
const dependencyNumber=edge=>typeof edge==="number"?edge:edge.note;
export function reasoningLinks(step,sessions,navigate){
  const records=sessions.flatMap(history=>history.notes.map(step=>({history,step}))),byNumber=new Map(records.map(record=>[record.step.note,record]));
  const links=[];
  if(step.revises_note!=null)links.push(["Revises",step.revises_note]);
  if(step.revised_by!=null)links.push(["Revised by",step.revised_by]);
  for(const edge of step.dependencies??[])links.push([({supports:"Supports",refutes:"Refutes",depends_on:"Depends on"})[edge.relation]??"References",dependencyNumber(edge)]);
  for(const record of records)for(const edge of record.step.dependencies??[])if(dependencyNumber(edge)===step.note)links.push([({supports:"Supported by",refutes:"Refuted by",depends_on:"Required by"})[edge.relation]??"Referenced by",record.step.note]);
  const section=element("section","","plan-relations reasoning-links");section.setAttribute("aria-label","Related reasoning");section.append(element("h4","Related reasoning"));
  if(!links.length){section.querySelector("h4").append(element("span","None","work-record-count"));return section;}
  const list=element("ul","","reasoning-relation-cards");
  for(const [index,[label,number]] of links.entries()){
    const record=byNumber.get(number),row=element("li");row.append(element("span",label,"plan-relation-state"));
    if(record){const button=element("button",record.step.purpose);button.dataset.focusKey="related-step:"+index+":"+number;button.onclick=()=>navigate(record,button.dataset.focusKey);row.append(button);}
    else row.append(element("strong","Referenced step unavailable"));
    row.append(element("span","Step "+number,"plan-relation-state"));list.append(row);
  }
  section.append(list);return section;
}