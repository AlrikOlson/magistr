import { element } from "./work_dom.mjs";
import { renderSourceDetail } from "./source_details.mjs";
const types={commit:"Commit",pr:"Pull request",file:"File",config:"Configuration",deployment:"Deployment",release:"Release"};
const actionLabels={code:"Code change",test:"Check",debug:"Investigation",research:"Research",config:"Configuration",refactor:"Code cleanup",review:"Review"};
const activityIdentity=(run,step)=>JSON.stringify([run.run_id,run.step_instances[step.id],"activity"]);
export function renderAction(action,copy){
  const row=element("li"),date=new Date(action.timestamp);
  row.dataset.action=String(action.id);
  if(!Number.isNaN(date.getTime())){const time=element("time",date.toLocaleString());time.dateTime=action.timestamp;row.append(time);}
  row.append(element("span",actionLabels[action.type]??"Activity","work-action-kind"),element("p",action.description,"work-action-description"));
  if(action.result){const result=element("section");renderSourceDetail(result,{label:"Result",source:action.result},copy);row.append(result);}
  for(const [label,values] of [["Files mentioned",action.files_touched],["Tools mentioned",action.tools_used]]){
    if(!values?.length)continue;
    const references=element("div","","work-action-references"),list=element("ul");references.append(element("h5",label));
    for(const value of values)list.append(element("li",value));references.append(list);row.append(references);
  }
  return row;
}
export function renderArtifact(run,step,artifact,index,{copy,openLink}){
  const identity=activityIdentity(run,step);
  const row=element("article"),heading=element("header");
  heading.append(element("span",types[artifact.type]||"Item","work-artifact-type"),element("h5",artifact.description||"Saved reference"));
  row.append(heading,element("p",artifact.ref,"work-artifact-reference"));
  const copyButton=element("button","Copy reference");copyButton.dataset.focusKey=identity+":artifact:"+index;
  copyButton.onclick=()=>copy(artifact.ref);const controls=element("div","","work-artifact-actions");controls.append(copyButton);row.append(controls);
  if(/^https?:\/\//i.test(artifact.ref)){
    const open=element("button","Open link");open.onclick=()=>openLink(artifact.ref);controls.append(open);
  }
  return row;
}
export function renderActivity(run,step,actions){
  if(!step.progress.length&&!step.artifacts.length)return null;
  const details=element("details","","work-activity"),summary=element("summary","Activity and files");
  const identity=activityIdentity(run,step);
  details.dataset.readingKey=identity;summary.dataset.focusKey=identity;details.append(summary);
  if(step.progress.length){
    const list=element("ol","","work-action-list");list.setAttribute("aria-label","Activity for "+step.title);
    for(const entry of step.progress)list.append(renderAction(entry,actions.copy));
    details.append(list);
  }
  if(step.artifacts.length){
    const section=element("section","","work-artifacts");section.setAttribute("aria-label","Produced items for "+step.title);
    section.append(element("h4","Produced items"));
    for(const [index,artifact] of step.artifacts.entries())section.append(renderArtifact(run,step,artifact,index,actions));
    details.append(section);
  }
  return details;
}
