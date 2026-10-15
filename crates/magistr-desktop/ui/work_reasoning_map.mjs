import {element} from "./work_dom.mjs";
import {WorkSurface} from "./work_surface.mjs";
const relation=edge=>({supports:"Supports",refutes:"Refutes",depends_on:"Depends on"})[edge.relation]??"References";
function decision(step){
  const button=element("button","","decision-piece");button.setAttribute("aria-label",step.purpose);button.title=step.purpose;button.dataset.stepId=step.record_id;button.dataset.pinned=String(!!step.pinned);
  const head=element("span","","decision-piece-index");head.append(element("span","Step "+step.step_number),element("span",step.revised_by?"Revised":step.pinned?"Pinned":""));
  button.append(head,element("strong",step.purpose),element("span",step.outcome||"No outcome recorded.","decision-piece-outcome"));return button;
}
export class ReasoningMap{
  constructor(){this.surface=new WorkSurface("Reasoning");}
  reset(){this.surface.reset();}
  render(history,steps,open,selectedBranch,references,selected){
    const keys=[...new Set(steps.map(step=>step.branch_id??""))],nodes=[],edges=[],groups=[],numbers=new Map(history.steps.map(step=>[step.step_number,step.record_id]));
    const branches=new Map((history.branches??[]).map(b=>[b.id,b])),byNumber=new Map(history.steps.map(s=>[s.step_number,s]));
    const depth=(key,seen=new Set())=>{if(!key||seen.has(key))return 0;seen.add(key);return 1+depth(byNumber.get(branches.get(key)?.from_step)?.branch_id,seen);};
    keys.sort((a,b)=>depth(a)-depth(b)||(branches.get(a)?.from_step??0)-(branches.get(b)?.from_step??0));
    const edge=(from,to,kind,caption,label)=>{const fromId=numbers.get(from),toId=numbers.get(to);if(fromId&&toId&&fromId!==toId)edges.push({from:fromId,to:toId,kind,caption,label});};
    const bottoms=new Map(),placed=new Map(),firstColumn=Math.min(0,...keys.map(key=>depth(key)));
    for(const key of keys){
      const branch=branches.get(key),members=steps.filter(step=>(step.branch_id??"")===key).sort((a,b)=>a.step_number-b.step_number),allMembers=history.steps.filter(step=>(step.branch_id??"")===key).sort((a,b)=>a.step_number-b.step_number);
      const column=depth(key)-firstColumn,x=column*384,y=Math.max(bottoms.get(column)??0,(placed.get(numbers.get(branch?.from_step))?.y??84)-84);
      const heading=element("div","","thread-heading");heading.append(element("strong",branch?.name??(key?"Investigation unavailable":"Main reasoning")));
      const facts=element("span","","thread-route");
      if(branch){
        facts.append(element("span",({active:"In progress",merged:"Concluded",abandoned:"Abandoned"})[branch.status]??"Status unavailable"));
        for(const [number,label] of [[branch.from_step,"Origin"],[branch.merged_into,"Conclusion"]])if(number!=null){
          const record=references.find(number),link=element(record?"button":"span",label+" · "+number);
          if(record){link.dataset.focusKey="thread:"+key+":"+label;link.onclick=()=>references.open(record,link.dataset.focusKey);}facts.append(link);
        }
        // A partial page must never invent an investigation's start or conclusion.
        if(allMembers.length){edge(branch.from_step,allMembers[0].step_number,"investigation","Investigate","Follow investigation: "+branch.name);if(branch.merged_into!=null)edge(allMembers.at(-1).step_number,branch.merged_into,"conclusion","Conclude","Follow conclusion: "+branch.name);}
      }else facts.append(element("span",members.length+" "+(members.length===1?"step":"steps")+" on this page"));
      heading.append(facts);
      for(const [row,step] of members.entries()){
        const node={id:step.record_id,label:step.purpose,x,y:y+84+row*142,w:264,h:118,element:decision(step)};nodes.push(node);placed.set(node.id,node);
        for(const dependency of step.dependencies??[]){const number=typeof dependency==="number"?dependency:dependency.step;edge(step.step_number,number,"relation",relation(dependency),relation(dependency)+" step "+number);}
        if(step.revises_step!=null)edge(step.step_number,step.revises_step,"revision","Revises","Revises step "+step.revises_step);
      }
      const bottom=y+84+(members.length-1)*142+118;bottoms.set(column,bottom+40);
      groups.push({kind:"thread",x,y,w:264,h:bottom-y,headingHeight:64,element:heading});
    }
    const ids=new Set(nodes.map(n=>n.id)),visible=edges.filter(e=>ids.has(e.from)&&ids.has(e.to));
    const root=this.surface.update({key:JSON.stringify([history.session_id,selectedBranch?.id??"all"]),nodes,edges:visible,groups,selected,focusId:nodes[0]?.id,onSelect:id=>{const step=history.steps.find(s=>s.record_id===id);if(step)open(step);}});
    root.classList.add("reasoning-surface");return root;
  }
}
