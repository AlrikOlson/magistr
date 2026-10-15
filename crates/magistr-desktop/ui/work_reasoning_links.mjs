import {element} from "./work_dom.mjs";
const dependencyNumber=edge=>typeof edge==="number"?edge:edge.step;
export function reasoningLinks(step,sessions,navigate){
  const records=sessions.flatMap(history=>history.steps.map(step=>({history,step}))),byNumber=new Map(records.map(record=>[record.step.step_number,record]));
  const links=[];
  if(step.revises_step!=null)links.push(["Revises",step.revises_step]);
  if(step.revised_by!=null)links.push(["Revised by",step.revised_by]);
  for(const edge of step.dependencies??[])links.push([({supports:"Supports",refutes:"Refutes",depends_on:"Depends on"})[edge.relation]??"References",dependencyNumber(edge)]);
  for(const record of records)for(const edge of record.step.dependencies??[])if(dependencyNumber(edge)===step.step_number)links.push([({supports:"Supported by",refutes:"Refuted by",depends_on:"Required by"})[edge.relation]??"Referenced by",record.step.step_number]);
  const section=element("section","","roadmap-relations reasoning-links");section.setAttribute("aria-label","Related reasoning");section.append(element("h4","Related reasoning"));
  if(!links.length){section.append(element("p","No related steps were recorded."));return section;}
  const list=element("ul");
  for(const [index,[label,number]] of links.entries()){
    const record=byNumber.get(number),row=element("li");row.append(element("span",label,"roadmap-relation-state"));
    if(record){const button=element("button",record.step.purpose);button.dataset.focusKey="related-step:"+index+":"+number;button.onclick=()=>navigate(record,button.dataset.focusKey);row.append(button);}
    else row.append(element("strong","Referenced step unavailable"));
    row.append(element("span","Step "+number,"roadmap-relation-state"));list.append(row);
  }
  section.append(list);return section;
}
