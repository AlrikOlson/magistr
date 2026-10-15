import { addressKey } from "./context.mjs";
import { readState } from "./work_records.mjs";
import { renderReferences } from "./work_references.mjs";
import { element, retainReading, saveReading, restoreReading } from "./work_dom.mjs";
import { WorkExecutionBrowser } from "./work_execution.mjs";
import { WorkTabs } from "./work_tabs.mjs";
import {renderRecordStatus,recordStatusMessage} from "./work_status.mjs";
import { RoadmapBrowser, renderRoadmap } from "./work_roadmap.mjs";
import { FeedbackBrowser } from "./work_feedback.mjs";
import { ReasoningBrowser } from "./work_reasoning.mjs";
import { searchRoadmap } from "./work_roadmap_search.mjs";
import { RoadmapItemReader } from "./work_item.mjs";

export class WorkBrowser {
  constructor($,invoke){
    this.$=$;this.invoke=invoke;this.frame=null;this.generation=0;this.revision=-1;this.loading=false;this.key="";this.readError="";
    this.area="work";this.searchPage=0;this.returnTrail=[];this.places=new Map();this.navigation=0;
    this.execution=new WorkExecutionBrowser($,invoke,()=>this.render());
    $("work-browser-return").onclick=()=>this.returnToFeedback();
    this.feedback=new FeedbackBrowser($("work-browser-content"),()=>{this.key="";this.render();},value=>navigator.clipboard.writeText(value).catch(error=>{$("work-browser-state").textContent="Could not copy the text. "+String(error);}),destination=>this.invoke("open_link",{destination}).catch(error=>{$("work-browser-state").textContent=String(error);}), (references,label)=>renderReferences(references,{snapshot:this.snapshot,navigate:(target,focus)=>this.openReference(target,focus),copy:value=>navigator.clipboard.writeText(value).catch(error=>{$("work-browser-state").textContent=String(error);})},label));
    this.reasoning=new ReasoningBrowser($("work-browser-reasoning"),$("work-browser-content"),()=>{this.key="";this.render();},value=>navigator.clipboard.writeText(value).catch(error=>{$("work-browser-state").textContent="Could not copy the text. "+String(error);}),$("work-browser-branch"));
    const filterFeedback=()=>{this.feedback.id=null;this.feedback.page=0;this.feedback.query=$("work-browser-feedback-search").value;this.feedback.kind=$("work-browser-feedback-kind").value;this.feedback.status=$("work-browser-feedback-status").value;this.key="";this.render();};
    $("work-browser-feedback-search").oninput=filterFeedback;$("work-browser-feedback-kind").onchange=filterFeedback;$("work-browser-feedback-status").onchange=filterFeedback;
    $("work-browser-reasoning-search").oninput=()=>{this.reasoning.query=$("work-browser-reasoning-search").value;this.reasoning.stepId=null;this.reasoning.trail=[];this.reasoning.page=0;this.key="";this.render();};
    $("work-browser-search").oninput=()=>{this.searchPage=0;this.item.close();this.key="";this.render();};
    this.item=new RoadmapItemReader($("work-browser-content"),invoke,()=>this.backToTrack(),()=>this.updateControls());
    this.tabs=new WorkTabs($("work-browser-tabs"),area=>this.selectArea(area));
    this.roadmapBrowser=new RoadmapBrowser($("work-browser-track"),()=>{this.item.close();this.key="";this.render();});
    this.searchFields=[...$("work-browser").querySelectorAll('input[type="search"]')].map(input=>{
      const field=element("div","","work-search-field"),clear=element("button","×","work-search-clear");
      clear.type="button";clear.setAttribute("aria-label","Clear "+input.labels[0].textContent.toLowerCase());clear.title="Clear search";clear.hidden=!input.value;
      input.before(field);field.append(input,clear);
      clear.onclick=()=>{input.value="";input.dispatchEvent(new Event("input",{bubbles:true}));input.focus();};
      input.addEventListener("input",()=>{clear.hidden=!input.value;});return {input,clear};
    });
    $("work-browser-roadmap-state").onchange=()=>{this.item.close();this.key="";this.render();};
    $("browse-work").onclick=()=>this.open($("browse-work"));
    $("browse-work-header").onclick=()=>this.open($("browse-work-header"));
    $("work-browser-close").onclick=()=>$("work-browser").close();
    $("work-browser-refresh").onclick=()=>{this.load();if(this.item.id)this.item.load();};
    $("work-browser").addEventListener("close",()=>{++this.generation;++this.navigation;this.places.clear();this.loading=false;this.item.close();this.returnTrail=[];this.key="";this.trigger?.focus();});
  }
  readingIdentity(){
    if(this.area==="roadmap")return JSON.stringify([this.item.id,this.roadmapBrowser.choice,this.$("work-browser-roadmap-state").value,this.$("work-browser-search").value,this.searchPage]);
    if(this.area==="work")return this.execution.readingIdentity();
    if(this.area==="reasoning")return JSON.stringify([this.reasoning.choice,this.reasoning.stepId,this.reasoning.query,this.reasoning.page,this.reasoning.branches.choice]);
    if(this.area==="feedback")return JSON.stringify([this.feedback.id,this.feedback.query,this.feedback.kind,this.feedback.status,this.feedback.page]);
    return this.area;
  }
  selectArea(area){
    if(area===this.area&&!this.routing)return;
    const content=this.$("work-browser-content"),navigation=++this.navigation;
    if(!this.routing&&this.snapshot&&this.$("work-browser").open){this.places.set(this.area,{identity:this.readingIdentity(),reading:saveReading(content)});this.returnTrail=[];}
    if(this.routing)this.item.close();else if(area!=="roadmap")this.item.suspend();
    this.area=area;this.key="";
    const pending=area==="roadmap"&&this.item.id?this.item.resume(this.frame.address,this.revision):null;
    this.render();
    if(this.routing)return;
    const restore=()=>{const saved=this.places.get(area);if(navigation!==this.navigation||!saved||saved.identity!==this.readingIdentity())return;restoreReading(content,{...saved.reading,focus:null});};
    if(pending)pending.then(restore);else restore();
  }
  observe(frame){
    if(addressKey(this.frame?.address)!==addressKey(frame?.address)){
      ++this.generation;++this.navigation;this.places.clear();this.item.close();this.revision=-1;this.key="";this.loading=false;this.snapshot=null;this.readError="";
      this.$("work-browser-content").replaceChildren();this.execution.reset();this.feedback.reset();this.$("work-browser-feedback-search").value="";this.$("work-browser-feedback-kind").value="all";this.$("work-browser-feedback-status").value="all";this.reasoning.reset();this.$("work-browser-reasoning-search").value="";this.roadmapBrowser.reset();this.searchPage=0;this.$("work-browser-search").value="";this.$("work-browser-roadmap-state").value="open";this.tabs.select("work");
      if(this.$("work-browser").open)this.$("work-browser").close();
    }
    this.frame=frame;
    for(const id of ["browse-work","browse-work-header"])this.$(id).disabled=!frame||frame.closed;
    if(this.$("work-browser").open&&!this.loading&&(frame.workflow_revision??0)>this.revision)this.load();
  }
  open(trigger){
    if(!this.frame||this.frame.closed)return;
    this.trigger=trigger;
    this.$("work-browser-project").textContent=this.frame.address.workspace;
    this.$("work-browser").showModal();this.render();this.$("work-browser-title").focus();this.load();
  }
  async load(){
    if(this.loading||!this.$("work-browser").open)return;
    const address=this.frame.address,generation=++this.generation;let applied=false;
    this.loading=true;this.$("work-browser-refresh").disabled=true;
    this.$("work-browser-state").textContent="Reading work records…";
    this.$("work-browser-content").setAttribute("aria-busy","true");
    try{
      const result=await this.invoke("read_workflow",{address});
      if(generation!==this.generation||addressKey(result.address)!==addressKey(this.frame?.address))return;
      this.readError="";this.revision=result.revision;this.snapshot=result.snapshot;this.roadmap=result.roadmap;applied=true;
      this.render();
    }catch(error){
      if(generation===this.generation){this.readError=String(error);this.render();}
    }finally{
      if(generation===this.generation){
        this.loading=false;this.$("work-browser-refresh").disabled=false;
        this.$("work-browser-content").setAttribute("aria-busy","false");
        if(applied&&(this.frame?.workflow_revision??0)>this.revision)this.load();
      }
    }
  }
  openReference(target,focus){
    const reading=saveReading(this.$("work-browser-content"));reading.focus=focus;
    this.returnTrail.push({id:this.feedback.id,page:this.feedback.page,query:this.feedback.query,kind:this.feedback.kind,status:this.feedback.status,reading});
    if(this.returnTrail.length>32)this.returnTrail.shift();
    this.routing=true;
    if(target.kind==="chunk"){this.tabs.select("roadmap");this.item.open(this.frame.address,target.id,this.revision);}
    if(target.kind==="think"){this.reasoning.query="";this.$("work-browser-reasoning-search").value="";this.reasoning.choice=target.session;this.reasoning.stepId=target.id;this.reasoning.trail=[];this.reasoning.branches.reset();this.tabs.select("reasoning");}
    if(target.kind==="signal"){this.feedback.id=target.id;this.tabs.select("feedback");}
    this.routing=false;this.$("work-browser-return").hidden=false;
    this.$("work-browser-content").scrollTop=0;this.$("work-browser-content").querySelector("h3")?.focus({preventScroll:true});
  }
  returnToFeedback(){
    const previous=this.returnTrail.pop();if(!previous)return;
    for(const key of ["id","page","query","kind","status"])this.feedback[key]=previous[key];
    this.$("work-browser-feedback-search").value=previous.query;this.$("work-browser-feedback-kind").value=previous.kind;this.$("work-browser-feedback-status").value=previous.status;
    this.routing=true;this.tabs.select("feedback");this.routing=false;restoreReading(this.$("work-browser-content"),previous.reading);
  }
  backToTrack(){
    const id=this.item.originId,scroll=this.item.originScroll;this.item.close();this.key="";this.render();
    const root=this.$("work-browser-content");root.scrollTop=scroll;
    [...root.querySelectorAll("[data-open-item]")].find(node=>node.dataset.openItem===id)?.focus({preventScroll:true});
  }
  renderRoadmap(){
    const observed=readState(this.roadmap,"roadmap"),frontier=this.roadmapBrowser.update(observed.value);
    this.$("work-browser-state").textContent=observed.message;
    this.$("work-browser-state").dataset.current=String(observed.current);
    if(this.item.id){this.item.observe(this.frame.address,this.revision);return;}
    const records=readState(this.snapshot.roadmap).value?.chunks??[];
    const mode=this.$("work-browser-roadmap-state").value,query=this.$("work-browser-search").value.trim();
    const key=JSON.stringify(["roadmap",this.roadmapBrowser.choice,mode,query,this.searchPage,observed.value,records]);if(key===this.key)return;this.key=key;
    const content=this.$("work-browser-content");
    if(query&&observed.value){
      retainReading(content,()=>content.replaceChildren(searchRoadmap(records,query,this.searchPage,page=>{this.searchPage=page;this.key="";this.render();content.scrollTop=0;},id=>this.item.open(this.frame.address,id,this.revision))));return;
    }
    retainReading(content,()=>content.replaceChildren(observed.value?renderRoadmap(frontier,observed.value,records,this.roadmapBrowser.choice!==undefined,id=>this.item.open(this.frame.address,id,this.revision),mode):element("p","The roadmap is unavailable. Use Refresh to try again.","work-record-empty")));
  }
  render(){
    for(const {input,clear} of this.searchFields)clear.hidden=!input.value;
    this.renderContent();
    if(this.readError){this.$("work-browser-state").textContent=(this.snapshot?"Showing the previous records. ":"")+"Could not read work records. "+this.readError;this.$("work-browser-state").dataset.current="false";}
  }
  updateControls(){
    this.$("work-browser-return").hidden=!this.returnTrail.length;
    this.$("work-feedback-filters").hidden=this.area!=="feedback"||!!this.feedback.id;
    this.$("work-feedback-search").hidden=this.area!=="feedback"||!!this.feedback.id;
    this.$("work-reasoning-selection").hidden=this.area!=="reasoning"||!!this.reasoning.query.trim()||!!this.reasoning.stepId;
    this.$("work-reasoning-search").hidden=this.area!=="reasoning"||!!this.reasoning.stepId;
    this.execution.show(this.area==="work");
    this.$("work-track-selection").hidden=this.area!=="roadmap"||!!this.item?.id;
    this.$("work-roadmap-search").hidden=this.area!=="roadmap"||!!this.item?.id;
  }
  renderContent(){
    this.updateControls();
    const content=this.$("work-browser-content");content.setAttribute("aria-labelledby","work-tab-"+this.area);
    if(!this.snapshot)return;
    if(this.area==="status"){
      const message=recordStatusMessage(this.snapshot);this.$("work-browser-state").textContent=message;this.$("work-browser-state").dataset.current=String(Object.values(this.snapshot).every(read=>read.state==="current"));
      retainReading(content,()=>content.replaceChildren(renderRecordStatus(this.snapshot,{copy:value=>navigator.clipboard.writeText(value).catch(error=>{this.$("work-browser-state").textContent="Could not copy the text. "+String(error);}),navigate:area=>this.tabs.select(area)})));return;
    }
    if(this.area==="roadmap"){this.renderRoadmap();return;}
    if(this.area==="feedback"){
      const observed=readState(this.snapshot.signals,"feedback");
      this.$("work-browser-state").textContent=observed.message;this.$("work-browser-state").dataset.current=String(observed.current);
      retainReading(content,()=>content.replaceChildren(observed.value?this.feedback.render(observed.value):element("p","Feedback is unavailable. Use Refresh to try again.")));return;
    }
    if(this.area==="reasoning"){
      const observed=readState(this.snapshot.think,"reasoning");
      this.$("work-browser-state").textContent=observed.message;this.$("work-browser-state").dataset.current=String(observed.current);
      retainReading(content,()=>content.replaceChildren(observed.value?this.reasoning.render(observed.value):element("p","Reasoning records are unavailable. Use Refresh to try again.")));return;
    }
    this.execution.render(this.snapshot.ship);
  }
}
