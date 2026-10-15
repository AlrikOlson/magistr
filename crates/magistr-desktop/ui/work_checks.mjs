import { element } from "./work_dom.mjs";
import { renderSourceDetail } from "./source_details.mjs";

export function renderChecks(run,task,copy){
  if(!task.checks.length)return null;
  const section=element("section","","work-checks");section.setAttribute("aria-label","Checks for "+task.title);
  section.append(element("h4","Checks"),element("p","These results describe the recorded checks. They do not confirm that the files are unchanged today.","work-check-note"));
  for(const [index,check] of task.checks.entries()){
    const identity=JSON.stringify([run.run_id,run.task_instances[task.id],check.receipt_id??[check.timestamp,check.name,index]]);
    const details=element("details","","work-check"),summary=element("summary");
    details.dataset.readingKey=identity;summary.dataset.focusKey=identity;
    details.dataset.verified=String(check.verified);details.dataset.passed=String(check.passed);
    summary.append(element("strong",check.name),element("span",(check.verified?"Executed":"Reported")+" · "+(check.passed?"Passed":"Failed")));
    details.append(summary);
    const facts=element("p","","work-check-facts");
    const date=new Date(check.timestamp);
    facts.textContent=[check.required?"Required check":"Optional check",Number.isNaN(date.getTime())?null:date.toLocaleString(),
      check.exit_code==null?null:"Exit code "+check.exit_code].filter(Boolean).join(" · ");
    details.append(facts);
    if(!check.verified)details.append(element("p","This result was reported without running a command.","work-record-warning"));
    if(check.command){const command=element("section");renderSourceDetail(command,{label:"Command",source:check.command},copy);details.append(command);}
    if(check.results){
      const results=check.results;
      details.append(element("p",results.passed+" passed · "+results.failed+" failed · "+results.skipped+" skipped","work-check-counts"));
      for(const failure of results.failures??[]){const failureView=element("section");renderSourceDetail(failureView,{label:failure.name||"Test failure",source:failure.message},copy);details.append(failureView);}
    }
    if(check.report&&!check.report.parsed)details.append(element("p","The test report could not be read. "+(check.report.error??""),"work-record-warning"));
    const output=element("section");renderSourceDetail(output,{label:"Check output",source:check.details},copy);details.append(output);
    section.append(details);
  }
  return section;
}
