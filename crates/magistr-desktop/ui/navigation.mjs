import { addressKey } from "./context.mjs";
import { sessionActivity, catalogActivity, activityKinds } from "./activity.mjs";

// A view of the existing project catalog and resident sessions, never another registry.
export function projectGroups(projects,sessions,query="",label=session=>session.conversation?.name??"") {
  const groups=new Map();
  for(const project of projects)if(!groups.has(project.id))groups.set(project.id,{...project,sessions:[]});
  for(const session of sessions) {
    const id=session.address.project;
    if(!groups.has(id))groups.set(id,{id,name:session.name,directory:session.address.workspace,sessions:[]});
    groups.get(id).sessions.push(session);
  }
  const words=query.normalize("NFKC").toLocaleLowerCase().trim().split(/\s+/u);
  return [...groups.values()].filter(group=>{const text=[group.name,group.directory,...group.sessions.map(label)].join(" ").normalize("NFKC").toLocaleLowerCase();return words.every(word=>text.includes(word));});
}
// Stable visual identity derives only from the canonical project id.
export function projectVariant(id) {
  let hash=0;for(const character of id)hash=(Math.imul(hash,31)+character.codePointAt(0))>>>0;
  return String(hash%4);
}
function projectWorkLinks(work,project){
  const nav=node("nav","project-work-links");nav.setAttribute("aria-label","Project work");
  for(const [area,label] of [["roadmap","Open workspace"]]){const button=node("button","",label);button.dataset.workArea=area;button.onclick=()=>work(project(),area,button);nav.append(button);}
  return nav;
}
const node=(tag,className,value="")=>{const el=document.createElement(tag);el.className=className;el.textContent=value;return el;};
function place(parent,children) {
  children.forEach((child,index)=>{if(parent.children[index]!==child)parent.insertBefore(child,parent.children[index]??null);});
  while(parent.children.length>children.length)parent.lastChild.remove();
}
export class ProjectNavigation {
  constructor(root,{select,work,open,act,close,label}) {
    Object.assign(this,{root,select,work,open,act,close,label});this.groups=new Map();this.rows=new Map();
    document.addEventListener("pointerdown",event=>{
      for(const row of this.rows.values())if(row.menu.open&&!row.menu.contains(event.target))row.menu.open=false;
    });
    root.addEventListener("keydown",event=>{
      if(event.key!=="Escape"||event.isComposing)return;
      const row=[...this.rows.values()].find(row=>row.menu.open&&row.menu.contains(event.target));
      if(row){event.preventDefault();row.menu.open=false;row.more.focus();}
    });
  }
  row(session,project) {
    const key=addressKey(session.address);let row=this.rows.get(key);
    if(!row) {
      const root=node("div","session-row"),select=node("button","session-select");
      const title=node("span","session-title"),state=node("small","session-state");
      select.append(title,state);
      const menu=node("details","session-menu"),more=node("summary","session-more","···");
      const actions=node("div","session-menu-actions");
      const stop=node("button","","Stop reply"),reconnect=node("button","","Reconnect"),close=node("button","destructive","Close conversation");
      actions.append(stop,reconnect,close);menu.append(more,actions);root.append(select,menu);
      row={root,select,title,state,menu,more,stop,reconnect,close,session};this.rows.set(key,row);
      select.onclick=()=>this.select(row.session.address);
      for(const [button,action] of [[stop,"cancel"],[reconnect,"reconnect"]])button.onclick=()=>{menu.open=false;this.act(row.session.target,action);};
      close.onclick=()=>{menu.open=false;this.close(row.session.address);};
      menu.addEventListener("toggle",()=>{if(menu.open){for(const other of this.rows.values())if(other!==row)other.menu.open=false;actions.scrollIntoView({block:"nearest"});}});
    }
    row.session=session;
    const name=this.label(session);
    if(row.title.textContent!==name)row.title.textContent=name;
    const state=[session.pending.length?"Needs your attention":session.status,
      session.activity?.queued_turns?session.activity.queued_turns+" queued":null,
      session.address.workspace!==project.directory?session.address.workspace.split(/[\\/]/).filter(Boolean).at(-1):null].filter(Boolean).join(" · ");
    if(row.state.textContent!==state)row.state.textContent=state;
    row.root.dataset.state=sessionActivity(session)??"closed";
    row.select.setAttribute("aria-label",name);row.select.title=name+"\n"+session.address.workspace;
    row.more.setAttribute("aria-label","Actions for "+name);
    row.stop.hidden=!session.activity?.active||session.closed;
    row.stop.disabled=!!session.activity?.cancellation_requested;
    row.reconnect.hidden=session.closed||session.status!=="Connection lost";
    row.reconnect.disabled=session.activity?.can_resume!==true;row.close.disabled=session.closed;
    row.stop.setAttribute("aria-label","Stop "+name);row.reconnect.setAttribute("aria-label","Reconnect "+name);row.close.setAttribute("aria-label","Close "+name);
    return row;
  }
  render(projects,sessions,selected) {
    const groups=projectGroups(projects,sessions),usedRows=new Set(),usedGroups=new Set();
    const names=new Map();for(const group of groups)names.set(group.name,(names.get(group.name)??0)+1);
    const elements=groups.map(project=>{
      usedGroups.add(project.id);let group=this.groups.get(project.id);
      if(!group) {
        const root=node("details","project-group");root.open=true;
        const heading=node("summary","project-heading"),name=node("span","project-title"),count=node("span","project-count");
        const path=node("small","project-path"),body=node("div","project-conversations"),open=node("button","project-open","Open project");
        const icon=node("span","project-monogram");icon.setAttribute("aria-hidden","true");
        const strip=node("div","project-status-strip");strip.setAttribute("aria-hidden","true");
        for(const [kind] of activityKinds){const part=node("span","");part.dataset.kind=kind;strip.append(part);}
        heading.append(icon,name,count);
        const work=projectWorkLinks(this.work,()=>group.project),tools=node("div","project-tools");
        tools.append(strip,work);root.append(heading,tools,path,body,open);
        group={root,heading,name,count,path,body,open,strip,icon,project,work};this.groups.set(project.id,group);
        open.onclick=()=>this.open(group.project.directory);
      }
      group.project=project;group.root.dataset.project=String(project.id);group.work.setAttribute("aria-label",project.name+" work");group.root.dataset.variant=projectVariant(project.id);group.name.textContent=project.name;
      group.icon.textContent=Array.from(project.name.trim()).slice(0,2).join("").toLocaleUpperCase();
      group.root.dataset.selected=String(project.sessions.some(session=>addressKey(session.address)===addressKey(selected)));
      group.heading.title=project.directory;group.heading.setAttribute("aria-label",project.name+" · "+project.directory);
      group.count.textContent=String(project.sessions.length);group.count.hidden=!project.sessions.length;
      const counts=catalogActivity(project.sessions);
      group.strip.hidden=!project.sessions.length;
      for(const part of group.strip.children){const count=counts[part.dataset.kind];part.style.flex=String(count);part.hidden=!count;}
      group.path.textContent=project.directory;group.path.hidden=names.get(project.name)<2;
      group.open.hidden=project.sessions.length>0;group.work.hidden=!project.sessions.some(session=>!session.closed);
      const rows=project.sessions.map(session=>{
        const row=this.row(session,project),key=addressKey(session.address);usedRows.add(key);
        row.select.setAttribute("aria-current",String(addressKey(selected)===key));return row.root;
      });
      place(group.body,rows);return group.root;
    });
    place(this.root,elements);
    for(const key of this.rows.keys())if(!usedRows.has(key))this.rows.delete(key);
    for(const key of this.groups.keys())if(!usedGroups.has(key))this.groups.delete(key);
  }
  reveal(address) {
    const group=this.groups.get(address.project),row=this.rows.get(addressKey(address));
    if(group)group.root.open=true;
    row?.root.scrollIntoView({block:"nearest",inline:"nearest"});
  }
}

// A second presentation of the same catalog, updated with the rail on every observation.
export class ProjectDirectory {
  constructor(root,{select,work,open,label}) {
    Object.assign(this,{root,select,work,open,label});this.groups=new Map();this.rows=new Map();
  }
  render(projects,sessions,query="") {
    const groups=projectGroups(projects,sessions.filter(session=>!session.closed),query,this.label),usedRows=new Set();
    const elements=groups.map(project=>{
      let group=this.groups.get(project.id);
      if(!group){
        const root=node("section","directory-project"),heading=node("header","directory-project-heading");
        const title=node("div","directory-project-title"),name=node("h2",""),path=node("input","directory-path"),facts=node("dl","directory-project-facts");path.readOnly=true;
        const mark=node("span","project-monogram");mark.setAttribute("aria-hidden","true");
        const opened=node("div",""),waiting=node("div","");
        const openCount=node("dd","","0"),waitCount=node("dd","","0");
        opened.append(node("dt","","Open conversations"),openCount);waiting.append(node("dt","","Waiting for you"),waitCount);facts.append(opened,waiting);
        const open=node("button","directory-open","Open project"),list=node("nav","directory-conversations");
        open.dataset.leadingIcon="arrow";
        const empty=node("p","directory-empty","No open conversations.");
        const cover=node("div","directory-cover");
        title.append(mark,name,path);heading.append(title,open);cover.append(heading,facts);root.append(cover,list,empty);
        const work=projectWorkLinks(this.work,()=>group.project);cover.append(work);
        group={root,name,path,facts,openCount,waitCount,open,list,empty,mark,project,work};this.groups.set(project.id,group);
        open.onclick=()=>this.open(group.project.directory);
      }
      group.project=project;group.root.dataset.project=String(project.id);group.work.setAttribute("aria-label",project.name+" work");group.root.dataset.variant=projectVariant(project.id);group.name.textContent=project.name;group.path.value=project.directory;group.path.setAttribute("aria-label","Project location for "+project.name);
      group.mark.textContent=Array.from(project.name.trim()).slice(0,2).join("").toLocaleUpperCase();
      group.open.setAttribute("aria-label","Open project "+project.name+" in "+project.directory);
      group.list.setAttribute("aria-label","Open conversations in "+project.name);
      const count=project.sessions.length,waiting=project.sessions.filter(session=>session.pending.length).length;
      group.work.hidden=!count;group.openCount.textContent=String(count);group.waitCount.textContent=String(waiting);
      group.waitCount.parentElement.dataset.waiting=String(waiting>0);
      const rows=project.sessions.map(session=>{
        const key=addressKey(session.address);usedRows.add(key);let row=this.rows.get(key);
        if(!row){
          const button=node("button","directory-conversation"),name=node("span","directory-conversation-name"),state=node("small","directory-conversation-state");
          button.append(name,state);row={button,name,state,session};this.rows.set(key,row);
          button.onclick=()=>this.select(row.session.address);
        }
        row.session=session;row.name.textContent=this.label(session);
        row.state.textContent=session.pending.length?"Needs your answer":session.status;
        row.button.dataset.attention=String(session.pending.length>0);
        row.button.dataset.state=sessionActivity(session);
        row.button.setAttribute("aria-label","Continue "+this.label(session)+" in "+project.name);
        row.button.title=session.address.workspace;return row.button;
      });
      place(group.list,rows);group.list.hidden=!rows.length;group.empty.hidden=!!rows.length;return group.root;
    });
    place(this.root,elements);
    for(const key of this.rows.keys())if(!usedRows.has(key))this.rows.delete(key);
    for(const key of this.groups.keys())if(!groups.some(project=>project.id===key))this.groups.delete(key);
    return groups.length;
  }
}