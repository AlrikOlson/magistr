import {element} from "./work_dom.mjs";
import {branchSummary} from "./work_reasoning_branch.mjs";
import {reasoningRow} from "./work_reasoning_step.mjs";
const branchStatus={active:"In progress",merged:"Included in conclusions",abandoned:"Abandoned"};
const edgeNumber=edge=>typeof edge==="number"?edge:edge.step;
const edgeLabel=edge=>({supports:"Supports",refutes:"Refutes",depends_on:"Depends on"})[edge.relation]??"References";
const svgNode=(tag,attributes={})=>{const node=document.createElementNS("http://www.w3.org/2000/svg",tag);for(const [key,value] of Object.entries(attributes))node.setAttribute(key,String(value));return node;};

// Disposable geometry over the current page. Native records own membership and every edge.
export class ReasoningMap{
  constructor(){this.observer=new ResizeObserver(()=>this.draw());}
  clear(){this.observer.disconnect();this.root=null;this.svg=null;this.nodes=new Map();this.edges=[];}
  render(history,steps,open,selectedBranch,references){
    this.clear();
    const reference=(number,prefix="Step ",kind="")=>{const record=references.find(number),node=element(record?"button":"span",prefix+number+(record?"":" · unavailable"));if(record){node.dataset.focusKey="map-reference:"+kind+prefix+number;node.onclick=()=>references.open(record,node.dataset.focusKey);}return node;};
    const viewport=element("div","","reasoning-map-viewport");viewport.tabIndex=0;viewport.setAttribute("role","region");viewport.setAttribute("aria-label","Decision map");
    const root=element("div","","reasoning-map"),lanes=element("div","","reasoning-map-lanes");
    const keys=[...new Set(steps.map(step=>step.branch_id??""))].sort((a,b)=>a===""?-1:b===""?1:0);
    root.style.setProperty("--map-lanes",String(keys.length));this.root=root;
    const svg=svgNode("svg",{"aria-hidden":"true",class:"reasoning-map-lines"}),defs=svgNode("defs"),marker=svgNode("marker",{id:"reasoning-arrow",viewBox:"0 0 8 8",refX:7,refY:4,markerWidth:6,markerHeight:6,orient:"auto-start-reverse"});marker.append(svgNode("path",{d:"M1 1 L7 4 L1 7",fill:"none",stroke:"currentColor","stroke-width":1.4}));defs.append(marker);svg.append(defs);root.append(svg,lanes);this.svg=svg;
    for(const key of keys){
      const branch=history.branches?.find(branch=>branch.id===key),group=element("section","","reasoning-map-lane"),head=element("header"),members=steps.filter(step=>(step.branch_id??"")===key);
      head.append(element("h4",key?branch?.name??"Investigation unavailable":"Main reasoning"));
      const meta=element("div","","reasoning-lane-facts");meta.append(element("span",members.length+(members.length===1?" step":" steps")));
      if(branch){meta.append(element("span",branchStatus[branch.status]??"Status unavailable"));head.append(meta);const route=element("p","","reasoning-branch-route");route.append(reference(branch.from_step,"From step "));if(branch.merged_into!=null)route.append(reference(branch.merged_into,"Conclusions at step "));head.append(route);}
      else head.append(meta);
      group.append(branch&&selectedBranch?branchSummary(branch,reference):head);const list=element("ol","","roadmap-items reasoning-map-nodes");
      for(const step of members){
        const row=reasoningRow(step,()=>open(step));row.dataset.record=step.record_id;row.dataset.pinned=String(!!step.pinned);
        row.firstChild.textContent=String(step.step_number);row.firstChild.setAttribute("aria-label","Step "+step.step_number);
        if(step.outcome){const outcome=element("p",step.outcome,"reasoning-outcome-preview");row.lastChild.append(outcome);}
        const relations=[...(step.dependencies??[]).map(edge=>edgeLabel(edge)+" step "+edgeNumber(edge))];if(step.revises_step!=null)relations.push("Revises step "+step.revises_step);if(step.revised_by!=null)relations.push("Revised by step "+step.revised_by);
        if(relations.length)row.lastChild.append(element("p",relations.join(" · "),"reasoning-map-references"));
        list.append(row);this.nodes.set(step.step_number,row);
        for(const edge of step.dependencies??[])this.edges.push({from:step.step_number,to:edgeNumber(edge),kind:"relation"});
        if(step.revises_step!=null)this.edges.push({from:step.step_number,to:step.revises_step,kind:"revision"});
      }
      if(branch){this.edges.push({from:branch.from_step,to:members[0].step_number,kind:"investigation"});if(branch.merged_into!=null)this.edges.push({from:members.at(-1).step_number,to:branch.merged_into,kind:"conclusion"});}
      group.append(list);lanes.append(group);
    }
    const legend=element("div","","reasoning-map-legend");
    for(const kind of new Set(this.edges.filter(edge=>this.nodes.has(edge.from)&&this.nodes.has(edge.to)&&edge.from!==edge.to).map(edge=>edge.kind))){const label=element("span",({investigation:"Investigation",conclusion:"Conclusion",revision:"Revision",relation:"Recorded relationship"})[kind]);label.dataset.kind=kind;legend.append(label);}
    if(legend.childNodes.length)viewport.append(legend);viewport.append(root);
    this.observer.observe(root);for(const node of this.nodes.values())this.observer.observe(node);
    return viewport;
  }
  draw(){
    if(!this.root?.isConnected||!this.root.clientWidth)return;
    const bounds=this.root.getBoundingClientRect();this.svg.setAttribute("viewBox","0 0 "+bounds.width+" "+bounds.height);this.svg.querySelectorAll(".reasoning-connection").forEach(node=>node.remove());
    const used=new Set();
    for(const edge of this.edges){
      const from=this.nodes.get(edge.from),to=this.nodes.get(edge.to),key=edge.from+":"+edge.to+":"+edge.kind;if(!from||!to||from===to||used.has(key))continue;used.add(key);
      const a=from.getBoundingClientRect(),b=to.getBoundingClientRect();let path;
      if(Math.abs(a.x-b.x)<4){const x=a.left-bounds.left+18,y=a.bottom-bounds.top+2,end=b.top-bounds.top-2;path="M "+x+" "+y+" C "+(x-28)+" "+y+" "+(x-28)+" "+end+" "+x+" "+end;}
      else{const forward=a.x<b.x,x=(forward?a.right:a.left)-bounds.left,y=a.top-bounds.top+a.height*.55+(edge.kind==="conclusion"?-16:0),endX=(forward?b.left:b.right)-bounds.left,endY=b.top-bounds.top+b.height*.55+(edge.kind==="conclusion"?-16:0),bend=(x+endX)/2;path="M "+x+" "+y+" C "+bend+" "+y+" "+bend+" "+endY+" "+endX+" "+endY;}
      this.svg.append(svgNode("path",{d:path,class:"reasoning-connection","data-kind":edge.kind,fill:"none","marker-end":"url(#reasoning-arrow)"}));
    }
  }
}