import {element} from "./work_dom.mjs";
const status={backlog:"Backlog",pending:"Planned",in_progress:"In progress",blocked:"Blocked",done:"Completed",obsoleted:"Retired"};
export function renderRelations(item,{navigate,current}){
  const section=element("section","","roadmap-relations roadmap-dependencies"),readiness=item.readiness;
  const state=readiness.state==="ready"?"Ready to start.":readiness.state==="waiting"?"Waiting for required work.":readiness.state==="blocked"?(readiness.blocker?.reason??"Work is blocked."):"";
  if(state){const note=element("p",(current?"":"Last recorded state: ")+state,"roadmap-readiness");note.dataset.ready=String(current&&readiness.state==="ready");section.append(note);}
  for(const [kind,title,empty] of [["dependencies","Required work","No prerequisites were recorded."],["dependents","Work that depends on this","No dependent items were recorded."]]){
    const group=element("section"),heading=element("header");group.setAttribute("aria-label",title);heading.append(element("h4",title),element("span",String(item[kind].length),"work-record-count"));group.append(heading);
    const list=element("ul");
    for(const relation of item[kind]){
      const row=element("li");row.dataset.available=String(relation.status!==null);
      if(relation.status!==null){
        const button=element("button",relation.title);button.dataset.focusKey=kind+":"+relation.id;button.onclick=()=>navigate(relation.id,button.dataset.focusKey);row.append(button);
        row.append(element("span",(relation.group??"Other work")+" · "+status[relation.status],"roadmap-relation-state"));
      }else row.append(element("strong","Required item unavailable"),element("span","This reference has no saved record.","roadmap-relation-state"));
      list.append(row);
    }
    group.append(item[kind].length?list:element("p",empty,"roadmap-relation-empty"));section.append(group);
  }
  return section;
}
