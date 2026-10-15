import { resumeReason } from "./session_controls.mjs";
import { renderDocument } from "./transcript.mjs";
export const historyName=record=>record.name??((record.purpose==="roadmap"?"Planning in ":"Conversation in ")+record.workspace.split(/[\\/]/).filter(Boolean).at(-1));
export function matchesHistory(record,query) {
  const words=query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  const content=[historyName(record),record.workspace,record.provider,record.model,record.summary].filter(Boolean).join(" ").toLocaleLowerCase();
  return words.every(word=>content.includes(word));
}
const documentExcerpt=nodes=>nodes.map(node=>node.text||documentExcerpt(node.children??[])).join(" ");
const text=(tag,value)=>{const element=document.createElement(tag);element.textContent=value;return element;};

// Only browsing state lives here. SavedDraftEditor owns text and attachment writes.
export class HistoryBrowser {
  constructor(element,invoke,{drafts,sessions,resume}) {
    Object.assign(this,{$:element,invoke,drafts,sessions,resume});
    this.records=[];this.current=null;this.rows=new Map();this.generation=0;this.selection=0;
    this.loadError=null;this.loading=false;this.opening=null;this.resuming=false;this.closing=false;this.returnToComposer=false;
    this.$("saved-conversations").onclick=()=>this.open();
    this.$("history-search").oninput=()=>this.renderList();
    this.$("history-refresh").onclick=()=>this.refresh();
    this.$("history-close").onclick=()=>this.close();
    this.$("history-picker").oncancel=event=>{event.preventDefault();this.close();};
    this.$("history-picker").onclose=()=>{
      ++this.generation;++this.selection;this.drafts.cancelOpen();
      (this.returnToComposer?this.$("draft"):this.$("saved-conversations")).focus();
    };
    this.$("history-resume").onclick=()=>this.resumeCurrent();
    this.$("history-copy").onclick=()=>navigator.clipboard.writeText(this.current?.summary??"").catch(error=>this.error(error));
  }
  error(error) {this.$("history-error").textContent=error?.message??String(error);this.$("history-error").hidden=false;}
  async open() {
    if(document.querySelector("dialog[open]"))return;
    this.returnToComposer=false;this.$("history-error").hidden=true;
    this.$("history-picker").showModal();this.$("history-search").focus();
    await this.refresh();
  }
  async refresh() {
    if(this.resuming||this.closing)return;
    const generation=++this.generation;this.loadError=null;this.loading=true;this.update();
    this.$("history-error").hidden=true;
    try {
      await this.drafts.flush();
      const records=await this.invoke("saved_conversations");
      if(generation!==this.generation)return;
      this.records=records;
      const current=records.find(record=>record.id===this.current?.id);
      if(current){this.current=current;this.renderSummary();}
      else if(records.length)await this.choose(records[0],false);
      else {this.current=null;this.$("history-summary").hidden=true;this.$("history-empty").hidden=false;}
    } catch(error) {if(generation===this.generation){this.loadError=error;this.error(error);}}
    finally {if(generation===this.generation){this.loading=false;this.update();}}
  }
  renderList() {
    const visible=this.records.filter(record=>matchesHistory(record,this.$("history-search").value));
    const nodes=visible.map(record=>{
      let row=this.rows.get(record.id);
      if(!row) {
        const button=text("button",""),project=text("small",""),name=text("strong",""),meta=text("time",""),excerpt=text("span","");
        excerpt.className="history-excerpt";button.append(project,meta,name,excerpt);row={button,project,name,meta,excerpt,record};this.rows.set(record.id,row);
        button.onclick=()=>this.choose(row.record);
      }
      row.record=record;row.project.textContent=record.workspace.split(/[\\/]/).filter(Boolean).at(-1);
      row.name.textContent=historyName(record);row.meta.textContent=new Date(record.updated_at).toLocaleDateString(undefined,{month:"short",day:"numeric",year:"numeric"});
      row.meta.dateTime=record.updated_at;row.excerpt.textContent=record.document?documentExcerpt(record.document.nodes):"";row.excerpt.hidden=!record.document;
      row.button.setAttribute("aria-label",historyName(record));row.button.title=record.workspace;
      row.button.setAttribute("aria-current",String(record.id===this.current?.id));
      row.button.setAttribute("aria-busy",String(record.id===this.opening));
      row.button.disabled=this.resuming||this.closing;
      return row.button;
    });
    const list=this.$("history-list");
    if(!nodes.length) {
      const message=this.loading&&!this.records.length?"Reading saved conversations…":this.records.length?"No conversations match your search.":this.loadError?"Could not read saved conversations.":"No saved conversations yet.";
      const empty=text("p",message);empty.className="history-list-empty";nodes.push(empty);
    }
    nodes.forEach((node,index)=>{if(list.children[index]!==node)list.insertBefore(node,list.children[index]??null);});
    while(list.children.length>nodes.length)list.lastChild.remove();
    for(const id of this.rows.keys())if(!this.records.some(record=>record.id===id))this.rows.delete(id);
    this.$("history-count").textContent=this.loading?"Refreshing…":this.loadError&&!this.records.length?"Unavailable":visible.length===this.records.length?this.records.length+(this.records.length===1?" conversation":" conversations"):visible.length+" of "+this.records.length;
    list.setAttribute("aria-busy",String(this.loading||!!this.opening));
  }
  renderSummary() {
    const record=this.current;if(!record)return;
    this.$("history-empty").hidden=true;this.$("history-summary").hidden=false;
    this.$("history-name").textContent=historyName(record);this.$("history-project").textContent=record.workspace;
    const metrics=[["Agent",record.provider],["Turns",record.turns==="unknown"?"Not reported":record.turns],["Spending",record.cost==="unknown"?"Not reported":record.cost]];
    this.$("history-details").replaceChildren(...metrics.map(([label,value])=>{
      const item=text("div","");item.append(text("dt",label),text("dd",value));return item;
    }));
    const saved=text("time",new Date(record.ended_at??record.updated_at).toLocaleString());saved.dateTime=record.ended_at??record.updated_at;
    this.$("history-saved-date").replaceChildren(document.createTextNode(record.ended_at?"Finished ":"Last saved "),saved);
    this.$("history-copy").disabled=!record.summary;
    if(record.document)renderDocument(this.$("history-text"),record.document.nodes,
      destination=>this.invoke("open_link",{destination}).catch(error=>this.error(error)),
      value=>navigator.clipboard.writeText(value).catch(error=>this.error(error)));
    else this.$("history-text").textContent="No reply summary was saved.";
  }
  update() {
    this.$("history-refresh").disabled=this.loading||this.resuming||this.closing;
    this.$("history-close").disabled=this.resuming||this.closing;
    this.$("history-search").disabled=this.resuming||this.closing;
    const reason=this.current?resumeReason(this.current,this.sessions()):"Choose a conversation to continue.";
    this.$("history-resume").disabled=!this.current||!!reason||this.resuming||this.closing||!!this.opening;
    this.$("history-resume").textContent=this.resuming?"Opening conversation…":"Resume conversation";
    this.$("history-resume-reason").textContent=this.resuming?"Opening with the original agent…":reason??"Continue with the original agent. Your unsent message will be there.";
    this.renderList();
  }
  async choose(record,focus=true) {
    if(this.resuming||this.closing)return;
    if(this.current?.id===record.id){if(focus)this.$("history-summary").focus();return;}
    const selection=++this.selection;this.opening=record.id;this.update();this.$("history-error").hidden=true;
    try {
      if(!await this.drafts.open(record)||selection!==this.selection)return;
      this.current=record;this.renderSummary();
      this.$("saved-draft").open=!!this.$("saved-message").value||!!this.drafts.files.files.length;
      this.$("history-reading").scrollTop=0;
      if(focus)this.$("history-summary").focus();
    } catch(error){if(selection===this.selection)this.error(error);}
    finally{if(selection===this.selection){this.opening=null;this.update();}}
  }
  async close() {
    if(this.resuming||this.closing)return;
    ++this.generation;++this.selection;this.opening=null;this.loading=false;this.drafts.cancelOpen();
    this.closing=true;this.update();
    try {await this.drafts.flush();this.$("history-picker").close();}
    catch(error){this.error(error);this.$("history-error").focus();}
    finally{this.closing=false;this.update();}
  }
  async resumeCurrent() {
    if(!this.current||this.resuming||this.opening)return;
    const record=this.current;this.resuming=true;this.$("history-error").hidden=true;this.update();
    try {
      await this.drafts.flush();
      const reason=resumeReason(record,this.sessions());if(reason)throw new Error(reason);
      await this.resume(record);this.returnToComposer=true;this.$("history-picker").close();
    } catch(error){this.error(error);this.$("history-error").focus();}
    finally{this.resuming=false;this.update();}
  }
}
