import {element} from "./work_dom.mjs";
import {WorkSurface} from "./work_surface.mjs";
import {pageControls} from "./work_search.mjs";
import {roadmapContext} from "./work_roadmap_context.mjs";
import {roadmapPositions} from "./work_roadmap_layout.mjs";
const labels={backlog:"Later",pending:"Planned",in_progress:"In progress",blocked:"Blocked",done:"Completed",obsoleted:"Retired"};
const marks={backlog:"◇",pending:"○",in_progress:"◉",blocked:"!",done:"✓",obsoleted:"–"};
function piece(record,readiness,next){
  const button=element("button","","work-piece");button.setAttribute("aria-label",record.title);button.dataset.state=record.status??"missing";button.dataset.ready=String(readiness?.state==="ready");button.dataset.next=String(next);button.title=record.title+(readiness?.blocker?.reason?"\n"+readiness.blocker.reason:"");
  const name=element("span","","work-piece-name");name.append(element("span",marks[record.status]??"?","work-piece-mark"),element("strong",record.title));button.append(name);
  const footer=element("span","","work-piece-footer");footer.append(element("span",next?"Next ready":readiness?.blocker?.reason?"Blocked · "+readiness.blocker.reason:readiness?.state==="waiting"?"Waiting for work":labels[record.status]??"Unavailable","work-piece-state"));
  const checks=(record.content?.sections??[]).flatMap(s=>s.list??[]).filter(item=>item.done!=null);
  if(checks.length){const tally=element("span","","work-piece-tally");tally.setAttribute("aria-label",checks.filter(c=>c.done).length+" of "+checks.length+" checklist items complete");for(const check of checks.slice(0,12)){const mark=element("i");mark.dataset.done=String(check.done);tally.append(mark);}footer.append(tally);}
  button.append(footer);return button;
}
export class RoadmapSurface{
  constructor(changed){this.changed=changed;this.surface=new WorkSurface("Roadmap");this.page=0;}
  reset(){this.surface.reset();this.page=0;this.choice=null;}
  render(frontier,data,records,hasChoice,open,mode,selected){
    const article=element("article","","roadmap-overview roadmap-spatial");
    if(!frontier){article.append(element("h3",hasChoice?"This track is no longer available.":"No roadmap items yet."));return article;}
    const head=element("header","","surface-heading"),title=element("div");title.append(element("h3",frontier.group??"Other work"));
    const counts=frontier.activity.readiness,summary=element("div","","surface-readiness");summary.append(element("span",counts.ready+" ready"),element("span",counts.waiting+" waiting"),element("span",counts.blocked+" blocked"));head.append(title,summary);article.append(head);
    const all=new Map(records.map(record=>[record.id,record])),items=new Map((data.frontiers??[]).flatMap(f=>f.items).map(item=>[item.id,item]));
    const choice=JSON.stringify([frontier.group,mode]);if(choice!==this.choice){this.choice=choice;this.page=0;}
    const primary=mode==="closed"?records.filter(r=>(r.group??null)===frontier.group&&["done","obsoleted"].includes(r.status)).map(r=>r.id):frontier.items.map(item=>item.id);
    const pages=pageControls(primary.length,this.page,page=>{this.page=page;this.surface.reset();this.changed();});article.append(pages.nav);
    const context=roadmapContext(primary.slice(pages.start,pages.end),all,selected),ids=[...context.records.keys()],layout=roadmapPositions(ids,context.records,primary,frontier.activity.next_ready),nodes=[],edges=[];
    for(const id of ids){
      const record=context.records.get(id),position=layout.positions.get(id),bundle=context.bundles.get(id);
      let control;
      if(bundle){control=element("button","","dependency-bundle");const detail=element("span","","dependency-bundle-detail");detail.append(element("span",bundle.more?"More prerequisites":"Prerequisites"),element("small",bundle.completed+" completed"));control.append(element("strong",String(bundle.count)),detail);control.setAttribute("aria-label","View "+bundle.count+(bundle.more?" more prerequisites for ":" prerequisites for ")+all.get(bundle.target).title);}
      else {control=piece(record,items.get(id)?.readiness,frontier.activity.next_ready===id);if(!all.has(id))control.disabled=true;}
      nodes.push({id,label:record.title,...position,w:248,h:76,element:control});
      for(const dep of record.deps??[])if(context.records.has(dep))edges.push({from:dep,to:id,missing:!all.has(dep)&&!context.bundles.has(dep),caption:"Needed for",label:(context.records.get(dep)?.title??"Unavailable work")+" is required for "+record.title});
    }
    const groups=[];
    if(!nodes.length)article.append(element("p",mode==="closed"?"This track has no completed or retired items.":"This track has no unfinished items."));
    else article.append(this.surface.update({key:JSON.stringify([frontier.group,mode,pages.page]),nodes,edges,groups,selected,focusId:frontier.activity.next_ready,onSelect:id=>open(context.bundles.get(id)?.target??id)}));
    return article;
  }
}