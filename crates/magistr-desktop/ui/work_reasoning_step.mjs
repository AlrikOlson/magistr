import {element,inlineFacts} from "./work_dom.mjs";
import {prose} from "./work_prose.mjs";
export function reasoningRow(step,open){
  const row=element("li","","reasoning-row"),body=element("div"),button=element("button",step.purpose,"roadmap-item-open");
  button.dataset.stepId=step.record_id;button.dataset.focusKey="step:"+step.record_id;button.onclick=open;
  row.append(element("span","Step "+step.step_number,"reasoning-step-number"));
  body.append(button);
  const labels=[step.branch_name,step.pinned?"Pinned":null,step.revised_by?"Revised":null].filter(Boolean);
  if(labels.length)body.append(element("span",labels.join(" · "),"roadmap-item-state"));
  row.append(body);return row;
}
export function reasoningStep(step,copy){
  const article=element("article","","work-record reasoning-step"),title=element("h3",step.purpose,"roadmap-track-title");title.tabIndex=-1;title.dataset.focusKey="reasoning-title";article.append(title);
  const date=step.timestamp?new Date(step.timestamp):null,time=element("time",date&&!Number.isNaN(date.getTime())?date.toLocaleString():"Not recorded");
  if(date&&!Number.isNaN(date.getTime()))time.dateTime=date.toISOString();
  const facts=[["Step",String(step.step_number)],["Recorded",time]];
  if(step.pinned)facts.push(["Pinned","Yes"]);
  if(step.confidence!=null)facts.push(["Reported confidence",Math.round(step.confidence*100)+"%"]);
  const metadata=inlineFacts(facts);metadata.classList.add("work-reasoning-facts");article.append(metadata);
  for(const [field,label] of [["outcome","Outcome"],["context","Context"],["thought","Reasoning"],["rationale","Rationale"],["uncertainty_notes","Uncertainty"],["revision_reason","Reason for revision"]]){
    if(step[field]){const section=prose(label,step[field],step.record_id+":"+field,copy);section.dataset.emphasis=String(field==="outcome");article.append(section);}
  }
  if(step.next_action)article.append(prose("Next action",typeof step.next_action==="string"?step.next_action:JSON.stringify(step.next_action,null,2),step.record_id+":next",copy));
  if(step.tools_used?.length)article.append(prose("Tools mentioned",step.tools_used.join("\n"),step.record_id+":tools",copy));
  return article;
}
