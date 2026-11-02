// Presentation of the workflow owner's read states and authored records.
import { element,inlineFacts } from "./work_dom.mjs";
import { renderChecks } from "./work_checks.mjs";
import { stepWindow,stepFacts } from "./work_step_view.mjs";
import { renderActivity } from "./work_activity.mjs";
export function readState(record,kind="work") {
  if(record?.state==="current")return {value:record.data.value,source:record.data.source,current:true,
    message:record.data.source.kind==="missing"?"No saved "+kind+" yet.":"Up to date."};
  if(record?.state==="stale")return {value:record.data.last.value,source:record.data.last.source,current:false,
    message:"Showing the last saved records. "+record.data.error.message};
  return {value:null,current:false,message:record?.data?.message??"Work records have not been read yet."};
}
const labels={open:"Open",active:"Active",finished:"Finished",abandoned:"Abandoned",planned:"Planned",waiting:"Waiting",done:"Done",skipped:"Skipped"};
export const statusLabel=status=>labels[status]??"Status unavailable";
export function authoredList(title,values,key) {
  if(!values?.length)return null;
  const section=element("details","","work-goal-details"),list=element("ul"),summary=element("summary");
  section.dataset.readingKey=key;summary.dataset.focusKey=key;
  summary.append(element("span",title),element("span",String(values.length),"work-record-count"));section.append(summary);
  for(const value of values)list.append(element("li",value));
  section.append(list);return section;
}
export function renderRun(run,actions,view) {
  const article=element("article","","work-record");
  if(!run){article.append(element("h3","No current work"),element("p","Start planned work to see its goal and steps here."));return article;}
  article.dataset.run=run.run_id;
  const head=element("header","","work-record-heading");
  const title=element("h3",run.goal.description);title.tabIndex=-1;title.dataset.focusKey="run-title";
  head.append(title,element("p",statusLabel(run.goal.status),"work-record-status"));
  const dates=element("dl","","work-record-dates");
  for(const [label,value] of [["Started",run.goal.created_at],["Finished",run.goal.completed_at]]){
    if(!value)continue;
    const date=new Date(value);if(Number.isNaN(date.getTime()))continue;
    const pair=element("div"),time=element("time",date.toLocaleString());time.dateTime=value;
    const description=element("dd");description.append(time);pair.append(element("dt",label),description);dates.append(pair);
  }
  if(dates.children.length)head.append(dates);
  article.append(head);
  for(const section of [authoredList("Requirements",run.goal.acceptance_criteria,run.run_id+":requirements"),authoredList("Constraints",run.goal.constraints,run.run_id+":constraints")])if(section)article.append(section);
  const steps=element("section","","work-record-section"),list=element("ol","","work-steps");
  const overview=element("header","","work-step-overview"),counts=new Map();
  for(const step of run.steps)counts.set(step.status,(counts.get(step.status)??0)+1);
  overview.append(element("h3","Steps"),inlineFacts([["Total",run.steps.length],...["done","active","planned","waiting","skipped"].filter(status=>counts.has(status)).map(status=>[statusLabel(status),counts.get(status)])]));steps.append(overview);
  const window=stepWindow(run.steps,view);if(view.query.trim()||view.status!=="all"||window.navigation.dataset.paged==="true")steps.append(window.navigation);
  for(const step of window.steps){
    const item=element("li"),heading=element("div","","work-step-heading");
    item.dataset.step=run.step_instances[step.id];item.dataset.status=step.status;
    item.value=run.steps.indexOf(step)+1;item.dataset.position=String(item.value).padStart(2,"0");
    heading.append(element("strong",step.title),element("span",statusLabel(step.status),"work-record-status"));item.append(heading,stepFacts(step));
    if(step.waiting_reason)item.append(element("p",step.waiting_reason,"work-record-warning"));
    const checks=renderChecks(run,step,actions.copy);if(checks)item.append(checks);
    const activity=renderActivity(run,step,actions);if(activity)item.append(activity);
    list.append(item);
  }
  if(window.total)steps.append(list);else steps.append(element("p",run.steps.length?"No steps match these filters.":"No steps have been added."));
  article.append(steps);return article;
}
