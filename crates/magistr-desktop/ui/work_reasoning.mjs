import {element} from "./work_dom.mjs";
import {pageControls} from "./work_search.mjs";
import {ReasoningBranches,branchSummary} from "./work_reasoning_branch.mjs";
import {searchReasoning} from "./work_reasoning_search.mjs";
import {reasoningLinks} from "./work_reasoning_links.mjs";
import {reasoningStep,reasoningRow} from "./work_reasoning_step.mjs";
export const reasoningName=history=>history.session_id==null?"Project reasoning":history.steps[0]?.purpose||"Untitled reasoning";
export class ReasoningBrowser{
  constructor(select,root,changed,copy,branches){this.select=select;this.root=root;this.changed=changed;this.copy=copy;this.branches=new ReasoningBranches(branches,()=>{this.page=0;this.stepId=null;this.trail=[];changed();});this.reset();select.onchange=()=>{this.choice=select.value;this.stepId=null;this.trail=[];this.page=0;this.branches.reset();changed();};}
  reset(){this.query="";this.choice=undefined;this.stepId=null;this.trail=[];this.page=0;this.keys="";this.branches.reset();}
  choose(step){this.trail=[];this.originScroll=this.root.scrollTop;this.stepId=step.record_id;this.changed();this.root.scrollTop=0;this.root.querySelector("h3")?.focus({preventScroll:true});}
  jump(record,focus){
    this.trail.push({choice:this.choice,stepId:this.stepId,page:this.page,branch:this.branches.choice,query:this.query,scroll:this.root.scrollTop,focus});if(this.trail.length>64)this.trail.shift();
    this.choice=JSON.stringify(record.history.session_id??null);this.stepId=record.step.record_id;this.branches.reset();this.page=0;
    this.changed();this.root.scrollTop=0;this.root.querySelector("h3")?.focus({preventScroll:true});
  }
  back(){
    const previous=this.trail.pop();
    if(previous){this.choice=previous.choice;this.stepId=previous.stepId;this.page=previous.page;this.branches.choice=previous.branch;this.query=previous.query;this.changed();this.root.scrollTop=previous.scroll;[...this.root.querySelectorAll("[data-focus-key]")].find(node=>node.dataset.focusKey===previous.focus)?.focus({preventScroll:true});return;}
    const id=this.stepId;this.stepId=null;this.changed();this.root.scrollTop=this.originScroll;[...this.root.querySelectorAll("[data-step-id]")].find(node=>node.dataset.stepId===id)?.focus({preventScroll:true});
  }
  render(data){
    const sessions=data?.sessions??[],keyOf=history=>JSON.stringify(history.session_id??null);
    if(this.choice===undefined&&sessions.length)this.choice=keyOf(sessions.find(h=>h.session_id==null)??sessions[0]);
    const options=sessions.map(history=>[keyOf(history),reasoningName(history)]);
    if(this.choice!==undefined&&!options.some(([id])=>id===this.choice))options.push([this.choice,"Reasoning no longer available"]);
    const keys=JSON.stringify(options);if(keys!==this.keys){this.select.replaceChildren(...options.map(([id,label])=>{const option=element("option",label);option.value=id;return option;}));this.keys=keys;}
    this.select.value=this.choice??"";this.select.disabled=!sessions.length;
    if(this.query.trim()&&!this.stepId)return searchReasoning(sessions,this.query,this.page,page=>{this.page=page;this.changed();this.root.scrollTop=0;},record=>{this.choice=keyOf(record.history);this.branches.reset();this.choose(record.step);});
    const history=sessions.find(history=>keyOf(history)===this.choice),article=element("article","","work-record");
    if(this.stepId){const back=element("button",this.trail.length?"Back to previous step":"Back to reasoning");back.dataset.focusKey="reasoning-back";back.onclick=()=>this.back();article.append(back);}
    if(!history){article.append(element("h3",this.choice===undefined?"No reasoning has been saved.":"This reasoning is no longer available."));return article;}
    const filtered=this.branches.update(history);
    if(filtered.missing){article.append(element("h3","This investigation is no longer available."));return article;}
    if(this.stepId){
      const step=history.steps.find(step=>step.record_id===this.stepId);
      if(step){
        const detail=reasoningStep(step,this.copy);detail.insertBefore(reasoningLinks(step,sessions,(record,focus)=>this.jump(record,focus)),detail.children[2]??null);article.append(detail);
      }else article.append(element("h3","This step is no longer available."));return article;
    }
    article.append(element("h3",reasoningName(history),"roadmap-track-title"),element("p",history.steps.length+" recorded "+(history.steps.length===1?"step":"steps")+" · "+(history.completed?"Finished":"In progress"),"work-record-status"));
    if(filtered.branch)article.append(branchSummary(filtered.branch));
    const pages=pageControls(filtered.steps.length,this.page,page=>{this.page=page;this.changed();this.root.scrollTop=0;});article.append(pages.nav);
    const list=element("ol","","roadmap-items");
    for(const step of filtered.steps.slice(pages.start,pages.end)){
      list.append(reasoningRow(step,()=>this.choose(step)));
    }
    article.append(filtered.steps.length?list:element("p","No steps are recorded in this selection."));return article;
  }
}
