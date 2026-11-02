import {element} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
import {renderCheck,checkNote} from "./work_checks.mjs";
const outcomes={all:()=>true,failed:check=>!check.passed,passed:check=>check.passed,reported:check=>!check.verified,executed:check=>check.verified,required:check=>check.required,optional:check=>!check.required};
export function renderCheckIndex(run,{copy},view){
  const root=element("section","","work-record work-check-index"),title=element("h3","Checks");title.tabIndex=-1;title.dataset.focusKey="check-index-title";const heading=element("header","","work-index-heading");heading.append(title);root.append(heading);
  if(!run){root.append(element("p","No work is selected."));return root;}
  root.append(element("p",run.goal.description,"work-index-goal"),element("p",checkNote,"work-check-note"));
  const attempts=run.steps.flatMap(step=>step.checks.map((check,index)=>({step,check,index})));
  const matches=attempts.filter(({step,check})=>(outcomes[view.status]??outcomes.all)(check)&&matchesText([step.title,check.name,check.type,check.command,check.details,JSON.stringify(check.results??{}),JSON.stringify(check.report??{})].join("\n"),view.query));
  heading.append(element("span",attempts.length+" recorded check attempts","work-record-count"));
  const page=pageControls(matches.length,view.page,view.change);if(page.nav.dataset.paged==="true"||view.query.trim()||view.status!=="all")root.append(page.nav);
  let group,stepId;
  for(const {step,check,index} of matches.slice(page.start,page.end)){
    if(step.id!==stepId){stepId=step.id;group=element("section","","work-check-attempt");group.append(element("h4",step.title));root.append(group);}
    group.append(renderCheck(run,step,check,index,copy));
  }
  if(!matches.length)root.append(element("p",attempts.length?"No check attempts match these filters.":"No checks have been recorded."));
  return root;
}
