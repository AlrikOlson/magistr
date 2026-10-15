import { decorateProject } from "./project_identity.mjs";
import { addressKey } from "./context.mjs";

// A view of the existing project catalog and resident sessions, never another registry.
export function projectGroups(projects,sessions) {
  const groups=new Map();
  for(const project of projects)if(!groups.has(project.id))groups.set(project.id,{...project,sessions:[]});
  for(const session of sessions) {
    const id=session.address.project;
    if(!groups.has(id))groups.set(id,{id,name:session.name,directory:session.address.workspace,sessions:[]});
    groups.get(id).sessions.push(session);
  }
  return [...groups.values()];
}
const node=(tag,className,value="")=>{const el=document.createElement(tag);el.className=className;el.textContent=value;return el;};
function place(parent,children) {
  children.forEach((child,index)=>{if(parent.children[index]!==child)parent.insertBefore(child,parent.children[index]??null);});
  while(parent.children.length>children.length)parent.lastChild.remove();
}
export class ProjectNavigation {
  constructor(root,{select,open,act,close,label}) {
    Object.assign(this,{root,select,open,act,close,label});this.groups=new Map();this.rows=new Map();
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
      menu.addEventListener("toggle",()=>{if(menu.open)for(const other of this.rows.values())if(other!==row)other.menu.open=false;});
    }
    row.session=session;
    const name=this.label(session);
    if(row.title.textContent!==name)row.title.textContent=name;
    const state=[session.pending.length?"Needs your attention":session.status,
      session.activity?.queued_turns?session.activity.queued_turns+" queued":null,
      session.address.workspace!==project.directory?session.address.workspace.split(/[\\/]/).filter(Boolean).at(-1):null].filter(Boolean).join(" · ");
    if(row.state.textContent!==state)row.state.textContent=state;
    row.root.dataset.state=session.pending.length?"waiting":session.activity?.active?"running":"ready";
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
        const emblem=node("span","project-emblem");emblem.setAttribute("aria-hidden","true");
        const path=node("small","project-path"),body=node("div","project-conversations"),open=node("button","project-open","Open project");
        heading.append(emblem,name,count);root.append(heading,path,body,open);
        group={root,heading,emblem,name,count,path,body,open,project};this.groups.set(project.id,group);
        open.onclick=()=>this.open(group.project.directory);
      }
      group.project=project;group.name.textContent=project.name;
      group.emblem.textContent=decorateProject(group.root,project);
      group.root.dataset.selected=String(project.sessions.some(session=>addressKey(session.address)===addressKey(selected)));
      group.heading.title=project.directory;group.heading.setAttribute("aria-label",project.name+" · "+project.directory);
      group.count.textContent=String(project.sessions.length);group.count.hidden=!project.sessions.length;
      group.path.textContent=project.directory;group.path.hidden=names.get(project.name)<2;
      group.open.hidden=project.sessions.length>0;
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