import { element } from "./work_dom.mjs";
import { renderSourceDetail } from "./source_details.mjs";
const types={commit:"Commit",pr:"Pull request",file:"File",config:"Configuration",deployment:"Deployment",release:"Release"};
const activityIdentity=(run,task)=>JSON.stringify([run.run_id,run.task_instances[task.id],"activity"]);
export function renderAction(action,copy){
  const row=element("li"),date=new Date(action.timestamp);
  row.dataset.action=String(action.id);
  if(!Number.isNaN(date.getTime())){const time=element("time",date.toLocaleString());time.dateTime=action.timestamp;row.append(time);}
  row.append(element("p",action.description,"work-action-description"));
  if(action.result){const result=element("section");renderSourceDetail(result,{label:"Result",source:action.result},copy);row.append(result);}
  for(const [label,values] of [["Files mentioned",action.files_touched],["Tools mentioned",action.tools_used]]){
    if(!values?.length)continue;
    row.append(element("h5",label));const references=element("ul");
    for(const value of values)references.append(element("li",value));row.append(references);
  }
  return row;
}
export function renderArtifact(run,task,artifact,index,{copy,openLink}){
  const identity=activityIdentity(run,task);
  const row=element("article"),heading=element("header");
  heading.append(element("h5",artifact.description||types[artifact.type]||"Produced item"),element("span",types[artifact.type]||"Item"));
  row.append(heading,element("p",artifact.ref,"work-artifact-reference"));
  const copyButton=element("button","Copy reference");copyButton.dataset.focusKey=identity+":artifact:"+index;
  copyButton.onclick=()=>copy(artifact.ref);row.append(copyButton);
  if(/^https?:\/\//i.test(artifact.ref)){
    const open=element("button","Open link");open.onclick=()=>openLink(artifact.ref);row.append(open);
  }
  return row;
}
export function renderActivity(run,task,actions){
  if(!task.actions.length&&!task.artifacts.length)return null;
  const details=element("details","","work-activity"),summary=element("summary","Activity and files");
  const identity=activityIdentity(run,task);
  details.dataset.readingKey=identity;summary.dataset.focusKey=identity;details.append(summary);
  if(task.actions.length){
    const list=element("ol","","work-action-list");list.setAttribute("aria-label","Activity for "+task.title);
    for(const action of task.actions)list.append(renderAction(action,actions.copy));
    details.append(list);
  }
  if(task.artifacts.length){
    const section=element("section","","work-artifacts");section.setAttribute("aria-label","Produced items for "+task.title);
    section.append(element("h4","Produced items"));
    for(const [index,artifact] of task.artifacts.entries())section.append(renderArtifact(run,task,artifact,index,actions));
    details.append(section);
  }
  return details;
}
