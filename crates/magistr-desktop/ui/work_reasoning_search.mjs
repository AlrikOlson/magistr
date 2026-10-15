import {element} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
import {reasoningRow} from "./work_reasoning_step.mjs";
export function searchReasoning(sessions,query,page,change,open){
  const records=sessions.flatMap(history=>history.steps.map(step=>({history,step}))).filter(({step})=>matchesText(
    [step.purpose,step.context,step.thought,step.rationale,step.outcome,step.uncertainty_notes,step.revision_reason,typeof step.next_action==="string"?step.next_action:JSON.stringify(step.next_action)].join("\n"),query));
  const article=element("article","","work-record"),pages=pageControls(records.length,page,change);
  article.append(element("h3","Reasoning search results","roadmap-track-title"),pages.nav);
  const list=element("ol","","roadmap-items");
  for(const record of records.slice(pages.start,pages.end)){
    list.append(reasoningRow(record.step,()=>open(record)));
  }
  article.append(records.length?list:element("p","No recorded reasoning matches this search."));return article;
}
