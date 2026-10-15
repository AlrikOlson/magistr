import { element,retainReading } from "./work_dom.mjs";
import { addressKey } from "./context.mjs";
import { readState } from "./work_records.mjs";
import { renderDocument } from "./transcript.mjs";
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
  const title=element("h3",record.title,"roadmap-track-title");title.tabIndex=-1;title.dataset.focusKey="roadmap-item-title";article.append(title);
  article.append(element("p",[record.group??"Other work",statuses[record.status]??"Status unavailable"].join(" · "),"work-record-status"));
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
      for(const entry of section.list){const row=element("li");if(entry.done!=null)row.append(element("span",entry.done?"Completed":"To do","roadmap-checklist-state"));row.append(element("span",entry.text));list.append(row);}
      article.append(list);
    }
  }
  const requirements=element("section","","roadmap-requirements");requirements.append(element("h4","Requirements"));
  const list=element("ol");
  for(const [index,value] of record.acceptance.entries()){const row=element("li","","message-body");row.dataset.reading=record.id+":requirement:"+index;renderDocument(row,item.requirements[index].nodes,actions.openLink,actions.copy);list.append(row);}
  requirements.append(record.acceptance.length?list:element("p","No requirements have been recorded."));article.append(requirements);
  if(record.notes)article.append(documentPart("Notes",record.notes,item.notes,record.id+":notes",actions));
  if(record.obsoleted_reason)article.append(element("p",record.obsoleted_reason,"work-record-warning"));
  return article;
}
export class RoadmapItemReader {
  constructor(root,invoke,back){this.root=root;this.invoke=invoke;this.back=back;this.id=null;this.generation=0;this.loading=false;this.read=null;this.error="";}
  close(){++this.generation;this.id=null;this.loading=false;this.read=null;this.error="";}
  open(address,id,revision){
    this.close();this.address=address;this.id=id;this.wanted=revision;this.originScroll=this.root.scrollTop;this.load(true);
  }
  observe(address,revision){
    if(addressKey(address)!==addressKey(this.address)){this.close();return;}
    if(revision>this.wanted){this.wanted=revision;if(!this.loading)this.load();}
  }
  async load(focus=false){
    if(!this.id||this.loading)return;
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
    if(!this.id)return;
    const observed=readState(this.read),shell=element("div"),back=element("button","Back to track");
    back.className="roadmap-back";back.dataset.focusKey="roadmap-back";back.onclick=()=>this.back();shell.append(back);
    const state=element("p",this.error||(this.loading?"Reading this item…":observed.message),"roadmap-item-state");state.setAttribute("role","status");shell.append(state);
    if(this.error||!observed.current&&!this.loading){const retry=element("button","Retry");retry.onclick=()=>this.load();shell.append(retry);}
    const actions={copy:value=>this.perform(()=>navigator.clipboard.writeText(value)),openLink:destination=>this.perform(()=>this.invoke("open_link",{destination}))};
    if(observed.value)shell.append(renderItem(observed.value,actions));
    else if(!this.loading)shell.append(element("h3",observed.current?"This item is no longer available.":"This item cannot be read."));
    retainReading(this.root,()=>this.root.replaceChildren(shell));
  }
}
