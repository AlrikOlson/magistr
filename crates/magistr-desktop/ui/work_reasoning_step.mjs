import {element} from "./work_dom.mjs";
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
  const date=step.timestamp?new Date(step.timestamp).toLocaleString():"Time not recorded";
  article.append(element("p","Step "+step.step_number+" · "+date+(step.pinned?" · Pinned":""),"work-record-status"));
  for(const [field,label] of [["context","Context"],["thought","Reasoning"],["rationale","Rationale"],["outcome","Outcome"],["uncertainty_notes","Uncertainty"],["revision_reason","Reason for revision"]]){
    if(step[field])article.append(prose(label,step[field],step.record_id+":"+field,copy));
  }
  if(step.next_action)article.append(prose("Next action",typeof step.next_action==="string"?step.next_action:JSON.stringify(step.next_action,null,2),step.record_id+":next",copy));
  if(step.confidence!=null)article.append(element("p","Reported confidence: "+Math.round(step.confidence*100)+"%."));
  if(step.tools_used?.length)article.append(prose("Tools mentioned",step.tools_used.join("\n"),step.record_id+":tools",copy));
  return article;
}
