import {element,inlineFacts} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
const types={implement:"Implementation",test:"Testing",review:"Review",config:"Configuration",docs:"Documentation",research:"Research"};
export function stepWindow(steps,view){
  const matches=steps.filter(step=>(view.status==="all"||step.status===view.status)&&matchesText([step.title,types[step.type],step.waiting_reason].join("\n"),view.query));
  const pages=pageControls(matches.length,view.page,view.change);
  return {steps:matches.slice(pages.start,pages.end),navigation:pages.nav,total:matches.length};
}
export function stepFacts(step){
  const facts=[["Type",types[step.type]??"Step"],["Estimate",step.estimate]];
  for(const [label,value] of [["Started",step.started_at],["Finished",step.completed_at]])if(value){
    const date=new Date(value);if(Number.isNaN(date.getTime()))continue;
    const time=element("time",date.toLocaleString());time.dateTime=value;facts.push([label,time]);
  }
  return inlineFacts(facts);
}
