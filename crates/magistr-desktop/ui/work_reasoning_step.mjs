import {element} from "./work_dom.mjs";
export function prose(label,value,key,copy){
  const section=element("section","","reasoning-prose"),header=element("header");header.append(element("h4",label));
  const button=element("button","Copy");button.setAttribute("aria-label","Copy "+label.toLowerCase());button.onclick=()=>copy(value);header.append(button);section.append(header);
  const body=element("div",value,"reasoning-text");body.dataset.reading=key;section.append(body);return section;
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
