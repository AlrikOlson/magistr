import { addressKey } from "./context.mjs";
import { readState, renderRun } from "./work_records.mjs";
import { element, retainReading } from "./work_dom.mjs";
import { WorkRuns } from "./work_runs.mjs";
import { WorkTabs } from "./work_tabs.mjs";
import { RoadmapBrowser, renderRoadmap } from "./work_roadmap.mjs";

export class WorkBrowser {
  constructor($,invoke){
    this.$=$;this.invoke=invoke;this.frame=null;this.generation=0;this.revision=-1;this.loading=false;this.key="";
    this.area="work";
    this.tabs=new WorkTabs($("work-browser-tabs"),area=>{this.area=area;this.key="";this.render();});
    this.roadmapBrowser=new RoadmapBrowser($("work-browser-track"),()=>{this.key="";this.render();});
    this.runs=new WorkRuns($("work-browser-runs"),()=>{this.key="";this.render();});
    $("browse-work").onclick=()=>this.open($("browse-work"));
    $("browse-work-header").onclick=()=>this.open($("browse-work-header"));
    $("work-browser-close").onclick=()=>$("work-browser").close();
    $("work-browser-refresh").onclick=()=>this.load();
    $("work-browser").addEventListener("close",()=>{++this.generation;this.loading=false;this.trigger?.focus();});
  }
  observe(frame){
    if(addressKey(this.frame?.address)!==addressKey(frame?.address)){
      ++this.generation;this.revision=-1;this.key="";this.loading=false;this.snapshot=null;
      this.$("work-browser-content").replaceChildren();this.runs.reset();this.roadmapBrowser.reset();this.tabs.select("work");
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
    this.$("work-browser").showModal();this.$("work-browser-title").focus();this.load();
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
      this.revision=result.revision;this.snapshot=result.snapshot;this.roadmap=result.roadmap;applied=true;
      this.render();
    }catch(error){
      if(generation===this.generation)this.$("work-browser-state").textContent=(this.key?"Showing the previous records. ":"")+"Could not read work records. "+String(error);
    }finally{
      if(generation===this.generation){
        this.loading=false;this.$("work-browser-refresh").disabled=false;
        this.$("work-browser-content").setAttribute("aria-busy","false");
        if(applied&&(this.frame?.workflow_revision??0)>this.revision)this.load();
      }
    }
  }
  renderRoadmap(){
    const observed=readState(this.roadmap,"roadmap"),frontier=this.roadmapBrowser.update(observed.value);
    this.$("work-browser-state").textContent=observed.message;
    this.$("work-browser-state").dataset.current=String(observed.current);
    const records=readState(this.snapshot.roadmap).value?.chunks??[];
    const key=JSON.stringify(["roadmap",this.roadmapBrowser.choice,observed.value,records]);if(key===this.key)return;this.key=key;
    const content=this.$("work-browser-content");
    retainReading(content,()=>content.replaceChildren(observed.value?renderRoadmap(frontier,observed.value,records,this.roadmapBrowser.choice!==undefined):element("p","The roadmap is unavailable. Use Refresh to try again.","work-record-empty")));
  }
  render(){
    this.$("work-run-selection").hidden=this.area!=="work";
    this.$("work-track-selection").hidden=this.area!=="roadmap";
    const content=this.$("work-browser-content");content.setAttribute("aria-labelledby","work-tab-"+this.area);
    if(!this.snapshot)return;
    if(this.area==="roadmap"){this.renderRoadmap();return;}
    const observed=readState(this.snapshot.ship);
    this.$("work-browser-state").textContent=observed.message;
    this.$("work-browser-state").dataset.current=String(observed.current);
    const run=this.runs.update(observed.value);
    const key=JSON.stringify([this.runs.choice,run,!!observed.value]);
    if(key===this.key)return;this.key=key;
    const copy=value=>navigator.clipboard.writeText(value).catch(error=>{this.$("work-browser-state").textContent="Could not copy the text. "+String(error);});
    const openLink=destination=>this.invoke("open_link",{destination}).catch(error=>{this.$("work-browser-state").textContent=String(error);});
    retainReading(content,()=>content.replaceChildren(!observed.value?element("p","Work records are unavailable. Use Refresh to try again.","work-record-empty"):
      this.runs.choice!=="current"&&!run?element("p","This saved work is no longer available. Choose another item.","work-record-empty"):renderRun(run,{copy,openLink})));
  }
}
