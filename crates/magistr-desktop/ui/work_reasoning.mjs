import {element,retainReading,saveReading,restoreReading} from "./work_dom.mjs";
import {pageControls} from "./work_search.mjs";
import {ReasoningBranches} from "./work_reasoning_branch.mjs";
import {searchReasoning} from "./work_reasoning_search.mjs";
import {reasoningLinks} from "./work_reasoning_links.mjs";
import {ReasoningMap} from "./work_reasoning_map.mjs";
import {reasoningStep} from "./work_reasoning_step.mjs";
export const reasoningName=history=>history.session_id==null?"Project reasoning":history.steps[0]?.purpose||"Untitled reasoning";
export class ReasoningBrowser{
  constructor(select,root,changed,copy,branches,inspector){this.inspector=inspector;this.select=select;this.root=root;this.changed=changed;this.copy=copy;this.map=new ReasoningMap();this.branches=new ReasoningBranches(branches,()=>{this.page=0;this.stepId=null;this.trail=[];changed();});this.reset();select.onchange=()=>{this.choice=select.value;this.stepId=null;this.trail=[];this.page=0;this.branches.reset();changed();};}
  reset(){this.map.reset();this.query="";this.choice=undefined;this.stepId=null;this.trail=[];this.page=0;this.keys="";this.branches.reset();}
  choose(step){this.trail=[];this.originScroll=this.root.scrollTop;this.stepId=step.record_id;this.changed();this.inspector.scrollTop=0;this.inspector.querySelector("h3")?.focus({preventScroll:true});}
  jump(record,focus){
    this.trail.push({choice:this.choice,stepId:this.stepId,page:this.page,branch:this.branches.choice,query:this.query,reading:saveReading(this.inspector),rootReading:saveReading(this.root),focus});if(this.trail.length>64)this.trail.shift();
    this.choice=JSON.stringify(record.history.session_id??null);this.stepId=record.step.record_id;this.branches.reset();this.page=0;
    this.changed();this.root.scrollTop=0;this.inspector.querySelector("h3")?.focus({preventScroll:true});
  }
  back(){
    const previous=this.trail.pop();
    if(previous){this.choice=previous.choice;this.stepId=previous.stepId;this.page=previous.page;this.branches.choice=previous.branch;this.query=previous.query;this.changed();restoreReading(this.root,previous.rootReading);restoreReading(this.inspector,previous.reading);[...this.root.querySelectorAll("[data-focus-key]"),...this.inspector.querySelectorAll("[data-focus-key]")].find(node=>node.dataset.focusKey===previous.focus)?.focus({preventScroll:true});return;}
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
    if(!history){this.inspector.hidden=true;this.inspector.replaceChildren();article.append(element("h3",this.choice===undefined?"No reasoning has been saved.":"This reasoning is no longer available."));return article;}
    const filtered=this.branches.update(history);
    if(filtered.missing){this.inspector.hidden=true;this.inspector.replaceChildren();article.append(element("h3","This investigation is no longer available."));return article;}
    this.inspector.hidden=!this.stepId;
    if(this.stepId){
      const detailRoot=element("div"),back=element("button",this.trail.at(-1)?.stepId?"Back to previous step":"Back to reasoning");back.dataset.focusKey="reasoning-back";back.onclick=()=>this.back();detailRoot.append(back);
      const step=history.steps.find(step=>step.record_id===this.stepId);
      if(step){
        const detail=reasoningStep(step,this.copy),links=reasoningLinks(step,sessions,(record,focus)=>this.jump(record,focus));(detail.querySelector('[data-emphasis="true"]')??detail.querySelector(".work-reasoning-facts")).after(links);detailRoot.append(detail);
      }else detailRoot.append(element("h3","This step is no longer available."));retainReading(this.inspector,()=>this.inspector.replaceChildren(detailRoot));
    }
    const heading=element("header","","surface-heading reasoning-overview-heading");heading.append(element("h3",reasoningName(history),"roadmap-track-title"),element("p",filtered.steps.length+" recorded "+(filtered.steps.length===1?"step":"steps")+" · "+(history.completed?"Finished":"In progress"),"work-record-status"));article.append(heading);
    const pages=pageControls(filtered.steps.length,this.page,page=>{this.page=page;this.changed();this.root.scrollTop=0;});article.append(pages.nav);
    article.classList.add("reasoning-overview","reasoning-spatial");
    if(filtered.steps.length)article.append(this.map.render(history,filtered.steps.slice(pages.start,pages.end),step=>this.choose(step),filtered.branch,{find:number=>sessions.flatMap(history=>history.steps.map(step=>({history,step}))).find(record=>record.step.step_number===number),open:(record,focus)=>this.jump(record,focus)},this.stepId));else article.append(element("p","No steps are recorded in this selection."));return article;
  }
}