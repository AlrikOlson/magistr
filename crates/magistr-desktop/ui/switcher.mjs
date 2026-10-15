import { addressKey } from "./context.mjs";
import { sessionActivity } from "./activity.mjs";
const text=(tag,value)=>{const node=document.createElement(tag);node.textContent=value;return node;};


function matchedText(node,value,words) {
  if(!words.length){node.textContent=value;return;}
  const pattern=new RegExp([...new Set(words)].sort((a,b)=>b.length-a.length).map(word=>word.replace(/[.*+?^$(){}|[\]\\]/g,"\\$&")).join("|"),"giu");
  const parts=[];let offset=0;
  for(const match of value.matchAll(pattern)){
    parts.push(document.createTextNode(value.slice(offset,match.index)),text("mark",match[0]));
    offset=match.index+match[0].length;
  }
  parts.push(document.createTextNode(value.slice(offset)));node.replaceChildren(...parts);
}

// Current catalog projection only; selection and draft admission remain with the application.
export class ConversationSwitcher {
  constructor(element,{sessions,selected,ready,select,label}) {
    Object.assign(this,{ $:element,sessions,selected,ready,select,label });this.rows=new Map();this.groups=new Map();this.pending=false;
    element("find-conversation").onclick=()=>this.show();
    element("switch-close").onclick=()=>this.close();
    element("switch-picker").oncancel=event=>{event.preventDefault();this.close();};
    element("switch-search").oninput=()=>this.update();
    element("switch-search").onkeydown=event=>{
      if(event.key==="ArrowDown"&&!event.isComposing){event.preventDefault();this.buttons()?.[0]?.focus();}
    };
    element("switch-results").onkeydown=event=>{
      if(event.isComposing||event.altKey||event.ctrlKey||event.metaKey)return;
      const buttons=this.buttons(),index=buttons.indexOf(document.activeElement);
      if(index<0)return;
      const next=event.key==="ArrowDown"?buttons[Math.min(index+1,buttons.length-1)]:event.key==="ArrowUp"?index?buttons[index-1]:element("switch-search"):event.key==="Home"?buttons[0]:event.key==="End"?buttons.at(-1):null;
      if(next){event.preventDefault();next.focus();}
    };
  }
  buttons(){return [...this.$("switch-results").querySelectorAll("button:not(:disabled)")];}
  show() {
    if(document.querySelector("dialog[open]"))return;
    this.returnFocus=document.activeElement;this.$("switch-search").value="";this.$("switch-error").hidden=true;
    this.$("switch-picker").showModal();this.update();this.$("switch-search").focus();
  }
  close() {
    if(this.pending)return;
    this.$("switch-picker").close();
    const target=this.returnFocus?.isConnected&&this.returnFocus.getClientRects().length&&!this.returnFocus.disabled?this.returnFocus:this.$("find-conversation");
    target.focus({preventScroll:true});
  }
  update() {
    if(!this.$("switch-picker").open)return;
    const query=this.$("switch-search").value.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
    const sessions=this.sessions().filter(session=>query.every(word=>[this.label(session),session.name,session.address.workspace].join(" ").toLocaleLowerCase().includes(word)));
    const previous=document.activeElement,used=new Set(),groups=new Map();
    for(const session of sessions){
      const key=JSON.stringify(session.address.project);
      if(!groups.has(key))groups.set(key,[]);groups.get(key).push(session);
    }
    const nodes=[...groups].map(([project,members])=>{
      let group=this.groups.get(project);
      if(!group){
        const section=text("section",""),heading=text("header",""),name=text("h3",""),detail=text("p",""),count=text("span",""),list=text("div","");
        section.className="switch-group";heading.append(name,count,detail);section.append(heading,list);
        group={section,name,detail,count,list};this.groups.set(project,group);
      }
      const copies=new Set(members.map(session=>session.address.workspace));
      matchedText(group.name,members[0].name,query);group.section.setAttribute("aria-label",members[0].name+" conversations");
      matchedText(group.detail,copies.size===1?members[0].address.workspace:copies.size+" working copies",query);
      group.count.textContent=members.length+(members.length===1?" conversation":" conversations");
      const rows=members.map(session=>{
      const key=addressKey(session.address);used.add(key);let row=this.rows.get(key);
      if(!row) {
        const button=document.createElement("button"),body=text("span",""),name=text("strong",""),context=text("small",""),state=text("span",""),current=text("small","Current conversation");
        body.className="switch-body";
        current.className="switch-current";body.append(name,context,current);state.className="switch-state";button.append(body,state);
        row={button,name,context,state,current,session};this.rows.set(key,row);
        button.onclick=()=>this.open(row.session);
      }
      row.session=session;matchedText(row.name,this.label(session),query);matchedText(row.context,session.address.workspace,query);row.context.hidden=copies.size===1;
      row.current.hidden=key!==addressKey(this.selected());row.button.dataset.state=sessionActivity(session)??"closed";
      row.state.textContent=session.closed?"Closing":session.status==="Connection lost"?"Connection lost":session.pending.length?"Needs your attention":session.status;
      row.button.setAttribute("aria-label","Open "+this.label(session)+" in "+session.name);
      row.button.dataset.attention=String(session.pending.length>0);row.button.disabled=this.pending||session.closed;row.button.setAttribute("aria-current",String(key===addressKey(this.selected())));
        return row.button;
      });
      rows.forEach((node,index)=>{if(group.list.children[index]!==node)group.list.insertBefore(node,group.list.children[index]??null);});
      while(group.list.children.length>rows.length)group.list.lastChild.remove();
      return group.section;
    });
    const root=this.$("switch-results");
    nodes.forEach((node,index)=>{if(root.children[index]!==node)root.insertBefore(node,root.children[index]??null);});
    while(root.children.length>nodes.length)root.lastChild.remove();
    for(const key of this.groups.keys())if(!groups.has(key))this.groups.delete(key);
    for(const key of this.rows.keys())if(!used.has(key))this.rows.delete(key);
    if(previous!==document.body&&!previous.isConnected)this.$("switch-search").focus();
    else if(previous!==document.body&&previous.isConnected&&!previous.disabled&&document.activeElement!==previous)previous.focus({preventScroll:true});
    this.$("switch-empty").hidden=sessions.length>0;
    this.$("switch-empty").textContent=!this.ready()?"Loading conversations…":query.length?"No open conversations match your search.":"Open a project to start a conversation.";
    this.$("switch-count").textContent=this.pending?"Opening conversation…":sessions.length+(sessions.length===1?" conversation":" conversations");
    this.$("switch-close").disabled=this.pending;this.$("switch-search").disabled=this.pending;
  }
  async open(session) {
    if(this.pending||session.closed)return;
    this.pending=true;this.$("switch-error").hidden=true;this.update();
    try {
      await this.select(session.address);this.$("switch-picker").close();this.$("view-title").focus({preventScroll:true});
    }catch(error){this.$("switch-error").textContent=String(error);this.$("switch-error").hidden=false;this.$("switch-error").focus();}
    finally{this.pending=false;this.update();}
  }
}
