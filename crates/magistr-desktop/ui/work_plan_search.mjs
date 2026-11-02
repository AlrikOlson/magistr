import {element} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
const states={idea:"Backlog",to_do:"Planned",in_progress:"In progress",waiting:"Blocked",done:"Completed",retired:"Retired"};
export function searchPlan(records,query,page,change,open){
  const matches=records.filter(item=>matchesText([item.title,item.description,item.notes,...item.acceptance,
    item.content?.summary,...(item.content?.facts??[]).flatMap(f=>[f.label,f.value]),
    ...(item.content?.sections??[]).flatMap(s=>[s.heading,s.prose,...(s.list??[]).map(e=>e.text)])].join("\n"),query));
  const article=element("article","","work-record"),pages=pageControls(matches.length,page,change);
  article.append(element("h3","Search results","plan-track-title"),pages.nav);
  const list=element("ol","","plan-items");
  for(const item of matches.slice(pages.start,pages.end)){
    const row=element("li"),title=element("button",item.title,"plan-item-open");title.dataset.openItem=item.id;title.dataset.focusKey="plan-open:"+item.id;title.onclick=()=>open(item.id);
    row.append(title,element("span",(item.feature??"Other work")+" · "+states[item.status],"plan-item-state"));list.append(row);
  }
  article.append(matches.length?list:element("p","No plan items match this search."));return article;
}
