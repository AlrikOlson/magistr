import {element,retainReading} from "./work_dom.mjs";
import {readState,renderRun} from "./work_records.mjs";
import {WorkRuns} from "./work_runs.mjs";
import {searchExecutions} from "./work_execution_search.mjs";
import {renderCheckIndex} from "./work_check_index.mjs";
import {renderActivityIndex} from "./work_activity_index.mjs";
export class WorkExecutionBrowser{
  constructor($,invoke,changed){
    this.$=$;this.invoke=invoke;this.changed=changed;
    this.runs=new WorkRuns($("work-browser-runs"),()=>{this.resetTasks();this.key="";changed();});
    this.reset();
    $("work-browser-work-search").oninput=()=>{this.workSearchPage=0;this.workSearchSelected=false;this.key="";changed();};
    for(const id of ["work-browser-task-search","work-browser-task-status","work-browser-check-search","work-browser-check-status","work-browser-activity-search","work-browser-activity-status","work-browser-execution-view"])$(id).addEventListener(id.endsWith("search")?"input":"change",()=>{this.taskPage=0;this.checkPage=0;this.activityPage=0;this.key="";changed();});
  }
  resetTasks(){this.taskPage=0;this.checkPage=0;this.activityPage=0;for(const kind of ["task","check","activity"]){this.$("work-browser-"+kind+"-search").value="";this.$("work-browser-"+kind+"-status").value="all";}}
  reset(){this.$("work-browser-execution-view").querySelector('input[value="tasks"]').checked=true;this.lastRun=null;this.runs.reset();this.$("work-browser-work-search").value="";this.workSearchPage=0;this.workSearchSelected=false;this.key="";this.resetTasks();}
  show(visible){
    if(this.visible!==visible){this.key="";this.visible=visible;}
    const results=!!this.$("work-browser-work-search").value.trim()&&!this.workSearchSelected;
    this.$("work-execution-search").hidden=!visible;this.$("work-run-selection").hidden=!visible||results;this.$("work-task-filters").hidden=!visible||results||this.$("work-browser-execution-view").querySelector("input:checked").value!=="tasks";
    this.$("work-check-filters").hidden=!visible||results||this.$("work-browser-execution-view").querySelector("input:checked").value!=="checks";
    this.$("work-activity-filters").hidden=!visible||results||this.$("work-browser-execution-view").querySelector("input:checked").value!=="activity";
  }
  readingIdentity(){return JSON.stringify([this.runs.choice,this.lastRun,this.$("work-browser-execution-view").querySelector("input:checked").value,this.$("work-browser-work-search").value,this.workSearchSelected,this.workSearchPage]);}
  render(read){
    const content=this.$("work-browser-content");
    const observed=readState(read);
    this.$("work-browser-state").textContent=observed.message;
    this.$("work-browser-state").dataset.current=String(observed.current);
    const query=this.$("work-browser-work-search").value.trim();
    if(query&&!this.workSearchSelected&&observed.value){
      retainReading(content,()=>content.replaceChildren(searchExecutions(observed.value,query,this.workSearchPage,page=>{this.workSearchPage=page;this.changed();content.scrollTop=0;},id=>{this.runs.choice="run:"+id;this.workSearchSelected=true;this.resetTasks();this.key="";this.changed();content.scrollTop=0;content.querySelector("h3")?.focus();})));return;
    }
    const run=this.runs.update(observed.value);
    if(run){if(this.lastRun&&this.lastRun!==run.run_id)this.resetTasks();this.lastRun=run.run_id;}
    const taskView={query:this.$("work-browser-task-search").value,status:this.$("work-browser-task-status").value,page:this.taskPage,change:page=>{this.taskPage=page;this.key="";this.changed();}};
    const checkView={query:this.$("work-browser-check-search").value,status:this.$("work-browser-check-status").value,page:this.checkPage,change:page=>{this.checkPage=page;this.key="";this.changed();}};
    const activityView={query:this.$("work-browser-activity-search").value,status:this.$("work-browser-activity-status").value,page:this.activityPage,change:page=>{this.activityPage=page;this.key="";this.changed();}};
    const mode=this.$("work-browser-execution-view").querySelector("input:checked").value;
    const key=JSON.stringify([query,this.workSearchSelected,this.runs.choice,run,taskView,checkView,activityView,mode,!!observed.value]);
    if(key===this.key)return;this.key=key;
    const copy=value=>navigator.clipboard.writeText(value).catch(error=>{this.$("work-browser-state").textContent="Could not copy the text. "+String(error);});
    const openLink=destination=>this.invoke("open_link",{destination}).catch(error=>{this.$("work-browser-state").textContent=String(error);});
    retainReading(content,()=>content.replaceChildren(!observed.value?element("p","Work records are unavailable. Use Refresh to try again.","work-record-empty"):
      this.runs.choice!=="current"&&!run?element("p","This saved work is no longer available. Choose another item.","work-record-empty"):mode==="activity"?renderActivityIndex(run,{copy,openLink},activityView):mode==="checks"?renderCheckIndex(run,{copy},checkView):renderRun(run,{copy,openLink},taskView)));
    if(query&&this.workSearchSelected){
      const back=element("button","Back to work results");back.dataset.focusKey="work-search-back";back.onclick=()=>{const id=run?.run_id;this.workSearchSelected=false;this.key="";this.changed();[...content.querySelectorAll("[data-run-id]")].find(node=>node.dataset.runId===id)?.focus({preventScroll:true});};content.prepend(back);
    }
  }
}
