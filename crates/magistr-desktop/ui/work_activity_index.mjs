import {element} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
import {renderAction,renderArtifact} from "./work_activity.mjs";
export function renderActivityIndex(run,actions,view){
  const root=element("section","","work-record work-activity-index"),title=element("h3","Activity and produced items","plan-track-title");title.tabIndex=-1;title.dataset.focusKey="activity-index-title";root.append(title);
  if(!run){root.append(element("p","No work is selected."));return root;}
  root.append(element("p",run.goal.description,"work-index-goal"));
  const entries=run.steps.flatMap(step=>[...step.progress.map((record,index)=>({step,record,index,kind:"activity"})),...step.artifacts.map((record,index)=>({step,record,index,kind:"items"}))]);
  const matches=entries.filter(({step,record,kind})=>(view.status==="all"||view.status===kind)&&matchesText([step.title,record.description,record.result,record.ref,record.type,...record.files_touched??[],...record.tools_used??[]].join("\n"),view.query));
  const page=pageControls(matches.length,view.page,view.change);root.append(page.nav);
  for(const {step,record,index,kind} of matches.slice(page.start,page.end)){
    const row=element("section","","work-activity-entry");row.append(element("h4",step.title));
    if(kind==="activity"){const list=element("ol","","work-action-list");list.start=index+1;list.append(renderAction(record,actions.copy));row.append(list);}
    else{const items=element("div","","work-artifacts");items.append(renderArtifact(run,step,record,index,actions));row.append(items);}
    root.append(row);
  }
  if(!matches.length)root.append(element("p",entries.length?"No entries match these filters.":"No activity or produced items have been recorded."));
  return root;
}
