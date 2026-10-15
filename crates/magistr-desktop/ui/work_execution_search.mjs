import {element,inlineFacts} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
import {statusLabel} from "./work_records.mjs";
export const executionRecords=data=>[...(data?.current?[data.current]:[]),...(data?.retained??[]).toReversed()];
function searchable(run){return [
  run.objective.description,...run.objective.acceptance_criteria,...run.objective.constraints,
  ...run.tasks.flatMap(task=>[task.title,task.blocked_reason,...task.actions.flatMap(action=>[action.description,action.result,...action.files_touched,...action.tools_used]),
    ...task.checks.flatMap(check=>[check.name,check.details,check.command,...(check.results?.failures??[]).flatMap(f=>[f.name,f.message])]),
    ...task.artifacts.flatMap(artifact=>[artifact.description,artifact.ref])])
].join("\n");}
export function searchExecutions(data,query,page,change,open){
  const records=executionRecords(data).filter(run=>matchesText(searchable(run),query)),article=element("article","","work-record"),pages=pageControls(records.length,page,change);
  article.append(element("h3","Work search results","roadmap-track-title"),pages.nav);const list=element("ol","","roadmap-items");
  for(const run of records.slice(pages.start,pages.end)){
    const row=element("li"),button=element("button",run.objective.description,"roadmap-item-open");button.dataset.runId=run.run_id;button.dataset.focusKey="run:"+run.run_id;button.onclick=()=>open(run.run_id);
    row.append(button,inlineFacts([["Status",statusLabel(run.objective.status)],["Tasks",run.tasks.length]]));list.append(row);
  }
  article.append(records.length?list:element("p","No saved work matches this search."));return article;
}
