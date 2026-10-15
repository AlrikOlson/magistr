import {element} from "./work_dom.mjs";
const labels={active:"In progress",merged:"Included in conclusions",abandoned:"Abandoned"};
export class ReasoningBranches{
  constructor(select,changed){this.select=select;this.changed=changed;this.reset();select.onchange=()=>{this.choice=select.value;changed();};}
  reset(){this.choice="all";this.keys="";}
  update(history){
    const branches=history.branches??[],options=[["all","All steps"],["main","Main reasoning"],...branches.map(branch=>["branch:"+branch.id,branch.name])];
    if(!options.some(([id])=>id===this.choice))options.push([this.choice,"Investigation no longer available"]);
    const key=JSON.stringify(options);if(key!==this.keys){this.select.replaceChildren(...options.map(([id,label])=>{const option=element("option",label);option.value=id;return option;}));this.keys=key;}
    this.select.value=this.choice;
    const branch=branches.find(branch=>"branch:"+branch.id===this.choice),missing=this.choice.startsWith("branch:")&&!branch;
    const steps=history.steps.filter(step=>this.choice==="all"||this.choice==="main"&&!step.branch_id||branch&&step.branch_id===branch.id);
    return {branch,steps,missing};
  }
}
export function branchSummary(branch){
  const section=element("section","","roadmap-relations");section.append(element("h4",branch.name),element("p",labels[branch.status]??"Status unavailable","work-record-status"));
  section.append(element("p","Started from step "+branch.from_step+"."));
  if(branch.merged_into!=null)section.append(element("p","Conclusions recorded in step "+branch.merged_into+"."));
  if(branch.created_at)section.append(element("p","Started "+new Date(branch.created_at).toLocaleString()+".","roadmap-relation-state"));
  return section;
}
