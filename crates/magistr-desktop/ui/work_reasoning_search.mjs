import {element} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
export function searchReasoning(sessions,query,page,change,open){
  const records=sessions.flatMap(history=>history.steps.map(step=>({history,step}))).filter(({step})=>matchesText(
    [step.purpose,step.context,step.thought,step.rationale,step.outcome,step.uncertainty_notes,step.revision_reason,typeof step.next_action==="string"?step.next_action:JSON.stringify(step.next_action)].join("\n"),query));
  const article=element("article","","work-record"),pages=pageControls(records.length,page,change);
  article.append(element("h3","Reasoning search results","roadmap-track-title"),pages.nav);
  const list=element("ol","","roadmap-items");
  for(const record of records.slice(pages.start,pages.end)){
    const row=element("li"),button=element("button",record.step.purpose,"roadmap-item-open");
    button.dataset.stepId=record.step.record_id;button.dataset.focusKey="step:"+record.step.record_id;button.onclick=()=>open(record);
    row.append(button,element("span","Step "+record.step.step_number+(record.step.branch_name?" · "+record.step.branch_name:""),"roadmap-item-state"));list.append(row);
  }
  article.append(records.length?list:element("p","No recorded reasoning matches this search."));return article;
}
