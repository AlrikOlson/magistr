import {element,inlineFacts} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
import {statusLabel} from "./work_records.mjs";
export const executionRecords=data=>[...(data?.current?[data.current]:[]),...(data?.retained??[]).toReversed()];
function searchable(run){return [
  run.goal.description,...run.goal.acceptance_criteria,...run.goal.constraints,
  ...run.steps.flatMap(step=>[step.title,step.waiting_reason,...step.progress.flatMap(entry=>[entry.description,entry.result,...entry.files_touched,...entry.tools_used]),
    ...step.checks.flatMap(check=>[check.name,check.details,check.command,...(check.results?.failures??[]).flatMap(f=>[f.name,f.message])]),
    ...step.artifacts.flatMap(artifact=>[artifact.description,artifact.ref])])
].join("\n");}
export function searchExecutions(data,query,page,change,open){
  const records=executionRecords(data).filter(run=>matchesText(searchable(run),query)),article=element("article","","work-record"),pages=pageControls(records.length,page,change);
  article.append(element("h3","Work search results","plan-track-title"),pages.nav);const list=element("ol","","plan-items");
  for(const run of records.slice(pages.start,pages.end)){
    const row=element("li"),button=element("button",run.goal.description,"plan-item-open");button.dataset.runId=run.run_id;button.dataset.focusKey="run:"+run.run_id;button.onclick=()=>open(run.run_id);
    row.append(button,inlineFacts([["Status",statusLabel(run.goal.status)],["Steps",run.steps.length]]));list.append(row);
  }
  article.append(records.length?list:element("p","No saved work matches this search."));return article;
}
