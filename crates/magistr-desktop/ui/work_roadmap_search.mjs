import {element} from "./work_dom.mjs";
import {matchesText,pageControls} from "./work_search.mjs";
const states={backlog:"Backlog",pending:"Planned",in_progress:"In progress",blocked:"Blocked",done:"Completed",obsoleted:"Retired"};
export function searchRoadmap(records,query,page,change,open){
  const matches=records.filter(item=>matchesText([item.title,item.description,item.notes,...item.acceptance,
    item.content?.summary,...(item.content?.facts??[]).flatMap(f=>[f.label,f.value]),
    ...(item.content?.sections??[]).flatMap(s=>[s.heading,s.prose,...(s.list??[]).map(e=>e.text)])].join("\n"),query));
  const article=element("article","","work-record"),pages=pageControls(matches.length,page,change);
  article.append(element("h3","Search results","roadmap-track-title"),pages.nav);
  const list=element("ol","","roadmap-items");
  for(const item of matches.slice(pages.start,pages.end)){
    const row=element("li"),title=element("button",item.title,"roadmap-item-open");title.dataset.openItem=item.id;title.dataset.focusKey="roadmap-open:"+item.id;title.onclick=()=>open(item.id);
    row.append(title,element("span",(item.group??"Other work")+" · "+states[item.status],"roadmap-item-state"));list.append(row);
  }
  article.append(matches.length?list:element("p","No roadmap items match this search."));return article;
}
