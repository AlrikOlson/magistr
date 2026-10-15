import {element} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
const types={implement:"Implementation",test:"Testing",review:"Review",config:"Configuration",docs:"Documentation",research:"Research"};
export function taskWindow(tasks,view){
  const matches=tasks.filter(task=>(view.status==="all"||task.status===view.status)&&matchesText([task.title,types[task.type],task.blocked_reason].join("\n"),view.query));
  const pages=pageControls(matches.length,view.page,view.change);
  return {tasks:matches.slice(pages.start,pages.end),navigation:pages.nav,total:matches.length};
}
export function taskFacts(task){
  const facts=element("p","","roadmap-relation-state"),parts=[types[task.type]??"Task"];
  for(const [label,value] of [["Started",task.started_at],["Finished",task.completed_at]])if(value){const date=new Date(value);if(!Number.isNaN(date.getTime()))parts.push(label+" "+date.toLocaleString());}
  facts.textContent=parts.join(" · ");return facts;
}
