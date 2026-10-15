import {element} from "./work_dom.mjs";
import {pageControls} from "./work_search.mjs";
import {reasoningStep} from "./work_reasoning_step.mjs";
export const reasoningName=history=>history.session_id==null?"Project reasoning":history.steps[0]?.purpose||"Untitled reasoning";
export class ReasoningBrowser{
  constructor(select,root,changed,copy){this.select=select;this.root=root;this.changed=changed;this.copy=copy;this.reset();select.onchange=()=>{this.choice=select.value;this.stepId=null;this.page=0;changed();};}
  reset(){this.choice=undefined;this.stepId=null;this.page=0;this.keys="";}
  choose(step){this.originScroll=this.root.scrollTop;this.stepId=step.record_id;this.changed();this.root.scrollTop=0;this.root.querySelector("h3")?.focus({preventScroll:true});}
  back(){const id=this.stepId;this.stepId=null;this.changed();this.root.scrollTop=this.originScroll;[...this.root.querySelectorAll("[data-step-id]")].find(node=>node.dataset.stepId===id)?.focus({preventScroll:true});}
  render(data){
    const sessions=data?.sessions??[],keyOf=history=>JSON.stringify(history.session_id??null);
    if(this.choice===undefined&&sessions.length)this.choice=keyOf(sessions.find(h=>h.session_id==null)??sessions[0]);
    const options=sessions.map(history=>[keyOf(history),reasoningName(history)]);
    if(this.choice!==undefined&&!options.some(([id])=>id===this.choice))options.push([this.choice,"Reasoning no longer available"]);
    const keys=JSON.stringify(options);if(keys!==this.keys){this.select.replaceChildren(...options.map(([id,label])=>{const option=element("option",label);option.value=id;return option;}));this.keys=keys;}
    this.select.value=this.choice??"";this.select.disabled=!sessions.length;
    const history=sessions.find(history=>keyOf(history)===this.choice),article=element("article","","work-record");
    if(!history){article.append(element("h3",this.choice===undefined?"No reasoning has been saved.":"This reasoning is no longer available."));return article;}
    if(this.stepId){
      const back=element("button","Back to reasoning");back.dataset.focusKey="reasoning-back";back.onclick=()=>this.back();article.append(back);
      const step=history.steps.find(step=>step.record_id===this.stepId);
      article.append(step?reasoningStep(step,this.copy):element("h3","This step is no longer available."));return article;
    }
    article.append(element("h3",reasoningName(history),"roadmap-track-title"),element("p",history.steps.length+" recorded "+(history.steps.length===1?"step":"steps")+" · "+(history.completed?"Finished":"In progress"),"work-record-status"));
    const pages=pageControls(history.steps.length,this.page,page=>{this.page=page;this.changed();this.root.scrollTop=0;});article.append(pages.nav);
    const list=element("ol","","roadmap-items");
    for(const step of history.steps.slice(pages.start,pages.end)){
      const row=element("li"),button=element("button",step.purpose,"roadmap-item-open");button.dataset.stepId=step.record_id;button.dataset.focusKey="step:"+step.record_id;button.onclick=()=>this.choose(step);
      row.append(button,element("span","Step "+step.step_number+(step.pinned?" · Pinned":"")+(step.revised_by?" · Revised":""),"roadmap-item-state"));list.append(row);
    }
    article.append(history.steps.length?list:element("p","This session has no recorded steps."));return article;
  }
}
