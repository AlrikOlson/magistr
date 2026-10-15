import { element } from "./work_dom.mjs";
const statuses={backlog:"Backlog",pending:"Planned",in_progress:"In progress",blocked:"Blocked",done:"Completed",obsoleted:"Retired"};
const readiness={ready:"Ready",waiting:"Waiting for other work",blocked:"Blocked",inactive:""};
export class RoadmapBrowser {
  constructor(select,changed){this.select=select;this.choice=undefined;this.keys="";select.onchange=()=>{this.choice=select.value;changed();};}
  reset(){this.choice=undefined;this.keys="";}
  update(data){
    const frontiers=data?.frontiers??[];
    if(this.choice===undefined&&frontiers.length)this.choice=JSON.stringify((frontiers.find(f=>f.items.length)??frontiers[0]).group);
    const choices=frontiers.map(f=>[JSON.stringify(f.group),(f.group??"Other work")+(f.items.length?"":" · Past work")]);
    if(this.choice!==undefined&&!choices.some(([id])=>id===this.choice)&&frontiers.length)choices.push([this.choice,"Track no longer available"]);
    const key=JSON.stringify(choices);
    if(key!==this.keys){this.select.replaceChildren(...choices.map(([id,label])=>{const option=element("option",label);option.value=id;return option;}));this.keys=key;}
    this.select.value=this.choice??"";this.select.disabled=!frontiers.length;
    return frontiers.find(f=>JSON.stringify(f.group)===this.choice);
  }
}
export function renderRoadmap(frontier,data,chunks,selected){
  const article=element("article","","work-record");
  if(!frontier){article.append(element("h3",selected?"This track is no longer available.":"No roadmap items yet."));return article;}
  const history=(frontier.group===null?data.catalog.ungrouped_history:data.catalog.history[frontier.group])??{done:0,obsoleted:0};
  article.append(element("h3",frontier.group??"Other work","roadmap-track-title"));
  const facts=element("dl","","roadmap-facts"),activity=frontier.activity;
  for(const [label,value] of [["Open items",activity.counts.total],["Ready",activity.readiness.ready],["Waiting",activity.readiness.waiting],["Blocked",activity.readiness.blocked],["Completed",history.done],["Retired",history.obsoleted]]){
    const fact=element("div");fact.append(element("dt",label),element("dd",String(value)));facts.append(fact);
  }
  article.append(facts);
  if(!frontier.items.length){article.append(element("p","This track has no unfinished items."));return article;}
  const byId=new Map(chunks.map(chunk=>[chunk.id,chunk])),list=element("ol","","roadmap-items");
  for(const item of frontier.items){
    const row=element("li"),chunk=byId.get(item.id),next=activity.next_ready===item.id;
    row.dataset.chunk=item.id;row.dataset.next=String(next);
    row.append(element("strong",chunk?.title??item.name));
    const state=[statuses[item.status]??"Status unavailable",readiness[item.readiness.state]].filter(Boolean).join(" · ");
    row.append(element("span",state,"roadmap-item-state"));
    if(next)row.append(element("span","Next ready","roadmap-next"));
    if(item.readiness.state==="blocked"&&item.readiness.blocker?.reason)row.append(element("p",item.readiness.blocker.reason,"work-record-warning"));
    if(item.readiness.state==="waiting")row.append(element("p",item.readiness.dependencies.length+" "+(item.readiness.dependencies.length===1?"dependency":"dependencies")+" unfinished or unavailable.","roadmap-waiting"));
    list.append(row);
  }
  article.append(list);return article;
}
