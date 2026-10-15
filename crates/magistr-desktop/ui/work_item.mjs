import { element,retainReading } from "./work_dom.mjs";
import { addressKey } from "./context.mjs";
import { readState } from "./work_records.mjs";
import { renderDocument } from "./transcript.mjs";
import { roadmapProposals } from "./work_proposals.mjs";
import { renderRelations } from "./work_relations.mjs";
const statuses={backlog:"Backlog",pending:"Planned",in_progress:"In progress",blocked:"Blocked",done:"Completed",obsoleted:"Retired"};
function documentPart(label,source,document,key,actions){
  const section=element("section","","roadmap-document"),head=element("header");
  head.append(element("h4",label));
  const copy=element("button","Copy");copy.setAttribute("aria-label","Copy "+label.toLowerCase());copy.onclick=()=>actions.copy(source);head.append(copy);section.append(head);
  const body=element("div","","message-body");body.dataset.reading=key;
  renderDocument(body,document.nodes,actions.openLink,actions.copy);section.append(body);return section;
}
function renderItem(item,actions){
  const {record}=item,article=element("article","","work-record roadmap-item-detail");
  const heading=element("header","","roadmap-item-heading");
  const title=element("h3",record.title,"roadmap-track-title");title.tabIndex=-1;title.dataset.focusKey="roadmap-item-title";heading.append(title);
  heading.append(element("p",[record.group??"Other work",statuses[record.status]??"Status unavailable"].join(" · "),"work-record-status"));article.append(heading);
  const proposals=roadmapProposals(record,actions);if(proposals)article.append(proposals);
  article.append(renderRelations(item,actions));
  if(record.content){
    article.append(element("p",record.content.summary,"roadmap-item-summary"));
    const facts=element("dl","","roadmap-content-facts");
    for(const fact of record.content.facts??[]){const pair=element("div");pair.append(element("dt",fact.label),element("dd",fact.value));facts.append(pair);}
    if(facts.children.length)article.append(facts);
  }
  if(record.description)article.append(documentPart("Description",record.description,item.description,record.id+":description",actions));
  for(const [index,section] of (record.content?.sections??[]).entries()){
    if(section.prose)article.append(documentPart(section.heading,section.prose,item.sections[index],record.id+":section:"+index,actions));
    else article.append(element("h4",section.heading));
    if(section.list?.length){
      const list=element("ul","","roadmap-checklist");
      for(const entry of section.list){const row=element("li");if(entry.done!=null){const state=element("span",entry.done?"✓":"○","roadmap-checklist-state");state.setAttribute("role","img");state.setAttribute("aria-label",entry.done?"Completed":"To do");state.title=entry.done?"Completed":"To do";state.dataset.done=String(entry.done);row.append(state);}row.append(element("span",entry.text));list.append(row);}
      article.append(list);
    }
  }
  const requirements=element("section","","roadmap-requirements"),requirementsHead=element("header");requirementsHead.append(element("h4","Requirements"),element("span",String(record.acceptance.length),"work-record-count"));requirements.append(requirementsHead);
  const list=element("ol");
  for(const [index,value] of record.acceptance.entries()){const row=element("li","","message-body");row.dataset.reading=record.id+":requirement:"+index;renderDocument(row,item.requirements[index].nodes,actions.openLink,actions.copy);list.append(row);}
  requirements.append(record.acceptance.length?list:element("p","No requirements have been recorded."));article.append(requirements);
  if(record.notes)article.append(documentPart("Notes",record.notes,item.notes,record.id+":notes",actions));
  if(record.obsoleted_reason)article.append(element("p",record.obsoleted_reason,"work-record-warning"));
  const navigation=element("label","","work-section-jump");navigation.append(element("span","Jump to section"));
  const select=element("select"),placeholder=element("option","Choose a section…");select.setAttribute("aria-label","Jump to section");placeholder.value="";select.append(placeholder);
  const headings=[...article.querySelectorAll("h4")];
  for(const [index,heading] of headings.entries()){
    heading.tabIndex=-1;heading.dataset.focusKey="roadmap-section:"+record.id+":"+index;
    const option=element("option",heading.textContent);option.value=String(index);select.append(option);
  }
  select.onchange=()=>{if(select.value==="")return;const heading=headings[Number(select.value)];heading.scrollIntoView({block:"start"});heading.focus({preventScroll:true});select.value="";};
  navigation.append(select);heading.after(navigation);
  return article;
}
export class RoadmapItemReader {
  constructor(root,invoke,back,changed){this.root=root;this.invoke=invoke;this.back=back;this.changed=changed;this.active=false;this.id=null;this.generation=0;this.loading=false;this.read=null;this.error="";this.trail=[];}
  setItem(id){++this.generation;this.id=id;this.loading=false;this.read=null;this.error="";this.changed();}
  close(){this.setItem(null);this.trail=[];this.active=false;}
  suspend(){++this.generation;this.loading=false;this.active=false;}
  resume(address,revision){
    if(!this.id)return;
    if(addressKey(address)!==addressKey(this.address)){this.close();return;}
    this.active=true;this.wanted=Math.max(this.wanted,revision);return this.load();
  }
  navigate(id,focus){
    if(id===this.id){this.root.querySelector("h3")?.focus();return;}
    this.trail.push({id:this.id,scroll:this.root.scrollTop,focus:focus??document.activeElement?.dataset.focusKey});if(this.trail.length>64)this.trail.shift();
    this.setItem(id);this.load(true);
  }
  async previous(){
    const previous=this.trail.pop();if(!previous){this.back();return;}
    this.setItem(previous.id);const pending=this.load(),generation=this.generation;await pending;
    if(this.generation!==generation)return;
    this.root.scrollTop=previous.scroll;
    [...this.root.querySelectorAll("[data-focus-key]")].find(node=>node.dataset.focusKey===previous.focus)?.focus({preventScroll:true});
  }
  open(address,id,revision){
    this.close();this.active=true;this.address=address;this.id=id;this.originId=id;this.wanted=revision;this.originScroll=this.root.scrollTop;this.changed();return this.load(true);
  }
  observe(address,revision){
    if(!this.active)return;
    if(addressKey(address)!==addressKey(this.address)){this.close();return;}
    if(revision>this.wanted){this.wanted=revision;if(!this.loading)this.load();}
  }
  async load(focus=false){
    if(!this.active||!this.id||this.loading)return;
    const generation=++this.generation,id=this.id,address=this.address;let applied=false,readRevision=-1;
    this.loading=true;this.error="";this.render();
    try{
      const result=await this.invoke("read_roadmap_item",{address,id});
      if(generation!==this.generation||result.id!==this.id||addressKey(result.address)!==addressKey(this.address))return;
      this.read=result.item;readRevision=result.revision;this.wanted=Math.max(this.wanted,result.revision);applied=true;
    }catch(error){if(generation===this.generation)this.error="Could not read this item. "+String(error);}
    finally{
      if(generation===this.generation){
        this.loading=false;this.render();
        if(focus){this.root.scrollTop=0;this.root.querySelector("h3")?.focus({preventScroll:true});}
        if(applied&&this.wanted>readRevision)this.load();
      }
    }
  }
  async perform(action){try{await action();}catch(error){this.error=String(error);this.render();}}
  render(){
    if(!this.active||!this.id)return;
    const observed=readState(this.read),shell=element("div"),back=element("button",this.trail.length?"Back to previous item":"Back to track");
    back.className="roadmap-back";back.dataset.focusKey="roadmap-back";back.onclick=()=>this.previous();const navigation=element("div","","work-record-navigation");navigation.append(back);shell.append(navigation);
    const state=element("p",this.error||(this.loading?"Reading this item…":observed.message),"roadmap-item-state");state.setAttribute("role","status");navigation.append(state);
    if(this.error||!observed.current&&!this.loading){const retry=element("button","Retry");retry.onclick=()=>this.load();shell.append(retry);}
    const actions={navigate:(id,focus)=>this.navigate(id,focus),current:observed.current,copy:value=>this.perform(()=>navigator.clipboard.writeText(value)),openLink:destination=>this.perform(()=>this.invoke("open_link",{destination}))};
    if(observed.value)shell.append(renderItem(observed.value,actions));
    else if(!this.loading)shell.append(element("h3",observed.current?"This item is no longer available.":"This item cannot be read."));
    retainReading(this.root,()=>this.root.replaceChildren(shell));
  }
}
