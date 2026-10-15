// Presentation of the workflow owner's read states and authored records.
import { element } from "./work_dom.mjs";
import { renderChecks } from "./work_checks.mjs";
import { taskWindow,taskFacts } from "./work_task_view.mjs";
import { renderActivity } from "./work_activity.mjs";
export function readState(record,kind="work") {
  if(record?.state==="current")return {value:record.data.value,source:record.data.source,current:true,
    message:record.data.source.kind==="missing"?"No saved "+kind+" yet.":"Up to date."};
  if(record?.state==="stale")return {value:record.data.last.value,source:record.data.last.source,current:false,
    message:"Showing the last saved records. "+record.data.error.message};
  return {value:null,current:false,message:record?.data?.message??"Work records have not been read yet."};
}
const labels={defined:"Planned",active:"In progress",completed:"Completed",abandoned:"Stopped",planned:"Planned",blocked:"Blocked",skipped:"Skipped"};
export const statusLabel=status=>labels[status]??"Status unavailable";
export function authoredList(title,values,key) {
  if(!values?.length)return null;
  const section=element("details","","work-objective-details"),list=element("ul"),summary=element("summary");
  section.dataset.readingKey=key;summary.dataset.focusKey=key;
  summary.append(element("span",title),element("span",String(values.length),"work-record-count"));section.append(summary);
  for(const value of values)list.append(element("li",value));
  section.append(list);return section;
}
export function renderRun(run,actions,view) {
  const article=element("article","","work-record");
  if(!run){article.append(element("h3","No current work"),element("p","Start planned work to see its objective and tasks here."));return article;}
  article.dataset.run=run.run_id;
  const head=element("header","","work-record-heading");
  const title=element("h3",run.objective.description);title.tabIndex=-1;title.dataset.focusKey="run-title";
  head.append(title,element("p",statusLabel(run.objective.status),"work-record-status"));
  const dates=element("dl","","work-record-dates");
  for(const [label,value] of [["Started",run.objective.created_at],["Finished",run.objective.completed_at]]){
    if(!value)continue;
    const date=new Date(value);if(Number.isNaN(date.getTime()))continue;
    const pair=element("div"),time=element("time",date.toLocaleString());time.dateTime=value;
    const description=element("dd");description.append(time);pair.append(element("dt",label),description);dates.append(pair);
  }
  if(dates.children.length)head.append(dates);
  article.append(head);
  for(const section of [authoredList("Requirements",run.objective.acceptance_criteria,run.run_id+":requirements"),authoredList("Constraints",run.objective.constraints,run.run_id+":constraints")])if(section)article.append(section);
  const tasks=element("section","","work-record-section"),list=element("ol","","work-tasks");
  tasks.append(element("h3","Tasks"),element("p",run.tasks.length+" "+(run.tasks.length===1?"task":"tasks"),"work-record-count"));
  const window=taskWindow(run.tasks,view);tasks.append(window.navigation);
  for(const task of window.tasks){
    const item=element("li"),heading=element("div","","work-task-heading");
    item.dataset.task=run.task_instances[task.id];item.dataset.status=task.status;
    item.value=run.tasks.indexOf(task)+1;item.dataset.position=String(item.value).padStart(2,"0");
    heading.append(element("strong",task.title),element("span",statusLabel(task.status),"work-record-status"));item.append(heading,taskFacts(task));
    if(task.blocked_reason)item.append(element("p",task.blocked_reason,"work-record-warning"));
    const checks=renderChecks(run,task,actions.copy);if(checks)item.append(checks);
    const activity=renderActivity(run,task,actions);if(activity)item.append(activity);
    list.append(item);
  }
  if(window.total)tasks.append(list);else tasks.append(element("p",run.tasks.length?"No tasks match these filters.":"No tasks have been added."));
  article.append(tasks);return article;
}
