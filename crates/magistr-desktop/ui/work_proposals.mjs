import {element} from "./work_dom.mjs";
import {prose} from "./work_prose.mjs";
const states={backlog:"Backlog",pending:"Planned",in_progress:"In progress",blocked:"Blocked",done:"Completed",obsoleted:"Retired"};
export function roadmapProposals(record,{copy,current}){
  const proposals=[
    ["Title",record.title_proposal,record.title,record.title_proposal?.suggested_title],
    ["Status",record.status_proposal,states[record.status],states[record.status_proposal?.suggested_status]],
    ["Priority",record.reprioritize,String(record.priority),String(record.reprioritize?.suggested_priority)]
  ].filter(([,proposal])=>proposal);
  if(!proposals.length)return null;
  const section=element("section","","roadmap-proposals");section.setAttribute("aria-label","Suggested changes");section.append(element("h4",current?"Suggested changes":"Last saved suggestions"),element("p","These suggestions have not changed the plan.","work-record-warning"));
  for(const [label,proposal,actual,suggested] of proposals){
    const card=element("article","","roadmap-relations");card.append(element("h5",label));
    const facts=element("dl","","roadmap-content-facts");for(const [name,value] of [["Current",actual],["Suggested",suggested]]){const pair=element("div");pair.append(element("dt",name),element("dd",value));facts.append(pair);}card.append(facts);
    card.append(prose("Reason",proposal.reason,record.id+":proposal:"+label,copy));
    if(proposal.source)card.append(prose("Source reference",proposal.source,record.id+":proposal-source:"+label,copy));
    if(proposal.proposed_at)card.append(element("p","Suggested "+new Date(proposal.proposed_at).toLocaleString()+".","roadmap-relation-state"));
    section.append(card);
  }
  return section;
}
