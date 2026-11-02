import {element} from "./work_dom.mjs";
const status={idea:"Backlog",to_do:"Planned",in_progress:"In progress",waiting:"Blocked",done:"Completed",retired:"Retired"};
export function renderRelations(item,{navigate,current}){
  const section=element("section","","plan-relations plan-dependencies"),readiness=item.readiness;
  const state=readiness.state==="ready"?"Ready to start.":readiness.state==="dependent"?"Waiting for required work.":readiness.state==="waiting"?(readiness.waiting_on?.reason??"Work is blocked."):"";
  if(state){const note=element("p",(current?"":"Last recorded state: ")+state,"plan-readiness");note.dataset.ready=String(current&&readiness.state==="ready");section.append(note);}
  for(const [kind,title] of [["dependencies","Required work"],["dependents","Work that depends on this"]]){
    const group=element("section"),heading=element("header");group.setAttribute("aria-label",title);heading.append(element("h4",title),element("span",item[kind].length?String(item[kind].length):"None","work-record-count"));group.append(heading);
    const list=element("ul");
    for(const relation of item[kind]){
      const row=element("li");row.dataset.available=String(relation.status!==null);
      if(relation.status!==null){
        const button=element("button",relation.title);button.dataset.focusKey=kind+":"+relation.id;button.onclick=()=>navigate(relation.id,button.dataset.focusKey);row.append(button);
        row.append(element("span",(relation.feature??"Other work")+" · "+status[relation.status],"plan-relation-state"));
      }else row.append(element("strong","Required item unavailable"),element("span","This reference has no saved record.","plan-relation-state"));
      list.append(row);
    }
    if(item[kind].length)group.append(list);section.append(group);
  }
  const selected=element("div","","plan-relation-current");selected.setAttribute("aria-label","Selected work");selected.dataset.before=String(item.dependencies.length>0);selected.dataset.after=String(item.dependents.length>0);selected.append(element("span","This work"),element("strong",item.record.title),element("small",status[item.record.status]));section.append(selected);
  return section;
}