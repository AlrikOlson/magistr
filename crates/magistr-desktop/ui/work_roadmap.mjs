import { element } from "./work_dom.mjs";
const statuses={backlog:"Backlog",pending:"Planned",in_progress:"In progress",blocked:"Blocked",done:"Completed",obsoleted:"Retired"};
export class RoadmapBrowser {
  constructor(select,changed){this.select=select;this.choice=undefined;this.keys="";select.onchange=()=>{this.choice=select.value;changed();};}
  reset(){this.choice=undefined;this.keys="";}
  update(data){
    const frontiers=data?.frontiers??[];
    if(this.choice===undefined&&frontiers.length)this.choice=JSON.stringify((frontiers.find(f=>f.items.length)??frontiers[0]).group);
    const choices=frontiers.map(f=>[JSON.stringify(f.group),(f.group??"Other work")+" · "+(f.activity.counts.total?f.activity.counts.total+" open":"Past work")]);
    if(this.choice!==undefined&&!choices.some(([id])=>id===this.choice)&&frontiers.length)choices.push([this.choice,"Track no longer available"]);
    const key=JSON.stringify(choices);
    if(key!==this.keys){this.select.replaceChildren(...choices.map(([id,label])=>{const option=element("option",label);option.value=id;return option;}));this.keys=key;}
    this.select.value=this.choice??"";this.select.disabled=!frontiers.length;
    return frontiers.find(f=>JSON.stringify(f.group)===this.choice);
  }
}
function openItem(item,open){
  const button=element("button",item.title??item.name,"roadmap-item-open");button.dataset.openItem=item.id;button.dataset.focusKey="roadmap-open:"+item.id;button.onclick=()=>open(item.id);return button;
}
function roadmapCard(item,chunk,next,byId,open){
  const row=element("li","","roadmap-card");row.dataset.chunk=item.id;row.dataset.next=String(next);row.dataset.readiness=item.readiness.state;
  const head=element("header","","roadmap-card-meta");head.append(element("span",statuses[item.status]??"Status unavailable","roadmap-item-state"));
  if(next)head.append(element("span","Next ready","roadmap-next"));
  row.append(head,openItem(chunk??item,open));
  if(item.readiness.state==="blocked"&&item.readiness.blocker?.reason)row.append(element("p",item.readiness.blocker.reason,"work-record-warning"));
  if(chunk?.content?.summary)row.append(element("p",chunk.content.summary,"roadmap-card-summary"));
  const tasks=(chunk?.content?.sections??[]).flatMap(section=>section.list??[]).filter(task=>task.done!=null);
  if(tasks.length){
    const progress=element("div","","roadmap-card-progress"),done=tasks.filter(task=>task.done).length,meter=element("progress");meter.max=tasks.length;meter.value=done;meter.setAttribute("aria-label","Recorded checklist: "+done+" of "+tasks.length+" complete");
    progress.append(meter,element("span",done+"/"+tasks.length+" checked"));row.append(progress);
  }
  const footer=element("footer","","roadmap-card-footer");
  if(chunk?.acceptance?.length)footer.append(element("span",chunk.acceptance.length+(chunk.acceptance.length===1?" requirement":" requirements")));
  const dependencies=chunk?.deps??[];
  if(dependencies.length){
    const links=element("details","","roadmap-card-links"),summary=element("summary",dependencies.length+(dependencies.length===1?" prerequisite":" prerequisites"));links.dataset.readingKey="dependencies:"+item.id;links.append(summary);
    for(const id of dependencies){const related=byId.get(id);if(related){const button=element("button",related.title);button.dataset.focusKey="card-dependency:"+item.id+":"+id;button.onclick=()=>{button.focus({preventScroll:true});open(id);};links.append(button);}else links.append(element("span","Required item unavailable"));}
    footer.append(links);
  }
  if(footer.childNodes.length)row.append(footer);return row;
}
export function renderRoadmap(frontier,data,chunks,selected,open,mode="open"){
  const article=element("article","","work-record roadmap-overview");
  if(!frontier){article.append(element("h3",selected?"This track is no longer available.":"No roadmap items yet."));return article;}
  const history=(frontier.group===null?data.catalog.ungrouped_history:data.catalog.history[frontier.group])??{done:0,obsoleted:0};
  const header=element("header","","roadmap-overview-heading");header.append(element("h3",frontier.group??"Other work","roadmap-track-title"));
  const total=frontier.activity.counts.total+history.done;
  if(total){const completion=element("div","","roadmap-completion"),progress=element("progress");progress.max=total;progress.value=history.done;progress.setAttribute("aria-label","Completed work in this track");completion.append(progress,element("span",history.done+" of "+total+" completed"));header.append(completion);}
  article.append(header);
  const facts=element("dl","","roadmap-facts"),activity=frontier.activity;
  for(const [label,value] of [["Open items",activity.counts.total],["Ready",activity.readiness.ready],["Waiting",activity.readiness.waiting],["Blocked",activity.readiness.blocked],["Completed",history.done],["Retired",history.obsoleted]]){
    const fact=element("div");fact.append(element("dt",label),element("dd",String(value)));facts.append(fact);
  }
  article.append(facts);
  if(mode==="closed"){
    article.append(element("h4","Completed and retired"));
    const closed=chunks.filter(item=>(item.group??null)===frontier.group&&["done","obsoleted"].includes(item.status));
    const list=element("ol","","roadmap-items");
    for(const item of closed){const row=element("li");row.dataset.chunk=item.id;row.append(openItem(item,open),element("span",statuses[item.status],"roadmap-item-state"));if(item.obsoleted_reason)row.append(element("p",item.obsoleted_reason,"roadmap-waiting"));list.append(row);}
    article.append(closed.length?list:element("p","This track has no completed or retired items."));return article;
  }
  if(!frontier.items.length){article.append(element("p","This track has no unfinished items."));return article;}
  const byId=new Map(chunks.map(chunk=>[chunk.id,chunk])),board=element("div","","roadmap-board");
  const laneOf=item=>item.status==="in_progress"?"active":item.readiness.state;
  const lanes=[["active","In progress","◉"],["ready","Ready","○"],["waiting","Waiting","↳"],["blocked","Blocked","!"],["inactive","Later","◇"]];
  for(const [key,label,mark] of lanes){
    const items=frontier.items.filter(item=>laneOf(item)===key);if(!items.length)continue;
    const lane=element("section","","roadmap-lane");lane.dataset.lane=key;lane.setAttribute("aria-label",label+" work");
    const heading=element("header"),icon=element("span",mark,"roadmap-lane-mark");icon.setAttribute("aria-hidden","true");heading.append(icon,element("h4",label),element("span",String(items.length),"work-record-count"));
    const list=element("ol","","roadmap-items");for(const item of items)list.append(roadmapCard(item,byId.get(item.id),activity.next_ready===item.id,byId,open));
    lane.append(heading,list);board.append(lane);
  }
  article.append(board);return article;
}