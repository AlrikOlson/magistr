import {stateAt,chapters,elapsed,preview,kindLabels,statusLabels} from "../data/build-recording.mjs";
import {RoadmapSurface} from "../../../../crates/magistr-desktop/ui/work_roadmap_surface.mjs";
import {ReasoningMap} from "../../../../crates/magistr-desktop/ui/work_reasoning_map.mjs";
import "../../../../crates/magistr-desktop/ui/work_surface.css";

const root=document.querySelector<HTMLElement>("[data-build]");
const el=(tag:string,text="",cls="")=>{const e=document.createElement(tag);e.textContent=text;if(cls)e.className=cls;return e;};
if(root) setup(root);
async function setup(root:HTMLElement){
 const failure=root.querySelector<HTMLElement>(".build-error")!;
 try {
  const response=await fetch(root.dataset.url!);if(!response.ok)throw Error("Recording unavailable");
  const recording=await response.json();if(recording.version!==1||!recording.events?.length)throw Error("Recording unavailable");
  const events=recording.events,parts=chapters(recording),panel=root.querySelector<HTMLElement>("#build-panel")!,session=root.querySelector<HTMLElement>("[data-session]")!,records=root.querySelector<HTMLElement>("[data-record-view]")!;
  const slider=root.querySelector<HTMLInputElement>('input[type="range"]')!,play=root.querySelector<HTMLButtonElement>("[data-play]")!,prev=root.querySelector<HTMLButtonElement>("[data-prev]")!,next=root.querySelector<HTMLButtonElement>("[data-next]")!;
  const tabs=[...root.querySelectorAll<HTMLButtonElement>("[data-view]")],chapterButtons=[...root.querySelectorAll<HTMLButtonElement>("[data-chapter]")];
  let index=Number(root.dataset.index),view="session",timer:ReturnType<typeof setTimeout>|undefined,selected:string|undefined;
  const roadmap=new RoadmapSurface(()=>renderRecords()),reasoning=new ReasoningMap();
  const compact=matchMedia("(max-width:1000px)");
  const button=(text:string,fn:()=>void)=>{const b=el("button",text) as HTMLButtonElement;b.type="button";b.onclick=fn;return b;};
  const stop=()=>{clearTimeout(timer);timer=undefined;play.textContent="Play steps";play.setAttribute("aria-pressed","false");};
  const openView=(name:string,focus=false)=>{stop();view=name;selected=undefined;render();if(focus)panel.focus({preventScroll:true});};
  const reader=(title:string,kind:string)=>{selected=title;panel.dataset.mode="reader";panel.scrollTop=0;const article=el("article","","record-reader");article.append(button("← Back to "+kind,()=>{selected=undefined;if(view==="session")render();else renderRecords();panel.focus({preventScroll:true});}),el("h3",title));records.replaceChildren(article);article.tabIndex=-1;article.focus({preventScroll:true});return article;};
  const paragraph=(article:HTMLElement,label:string,value:string)=>{if(!value)return;article.append(el("h4",label),el("p",value));};
  function openChunk(chunk:any){
   const a=reader(chunk.title,"roadmap");a.append(el("p",statusLabels[chunk.status]??chunk.status,"record-status"));
   if(chunk.content?.summary)a.append(el("p",chunk.content.summary));
   a.append(el("h4","Done when"));const list=el("ul");for(const requirement of chunk.acceptance)list.append(el("li",requirement));a.append(list);
   const s=stateAt(recording,index);
   for(const [heading,pieces]of [["Needs first",s.chunks.filter(c=>chunk.deps.includes(c.id))],["Makes possible",s.chunks.filter(c=>c.deps.includes(chunk.id))]] as any){
    if(!pieces.length)continue;a.append(el("h4",heading));for(const c of pieces)a.append(button(c.title,()=>openChunk(c)));
   }
  }
  function openStep(step:any){
   const a=reader(step.purpose,"reasoning");a.append(el("p","Step "+step.step_number+(step.pinned?" · Pinned":""),"record-status"),el("p",step.outcome,"record-outcome"));
   paragraph(a,"Context",step.context);paragraph(a,"Finding",step.thought);paragraph(a,"Why",step.rationale);paragraph(a,"Next action",typeof step.next_action==="string"?step.next_action:step.next_action?.action);
   const s=stateAt(recording,index);
   for(const dep of step.dependencies??[]){const number=typeof dep==="number"?dep:dep.step;const other=s.steps.find(x=>x.step_number===number);if(other)a.append(button((dep.relation==="refutes"?"Refutes":dep.relation==="depends_on"?"Depends on":"Supports")+" step "+number+": "+other.purpose,()=>openStep(other)));}
  }
  function openCheck(check:any){const a=reader(check.name,view);a.append(el("p",(check.passed?"Passed":"Failed")+" · "+(check.verified?"Executed command":"Written review"),"record-status"),el("pre",check.details));}
  function list(items:any[],label:(x:any)=>string,status:(x:any)=>string,open:(x:any)=>void,empty:string){
   if(!items.length){records.replaceChildren(el("p",empty,"record-empty"));return;}
   const ul=el("ul","","record-list");for(const item of items){const li=el("li"),b=button("",()=>open(item));b.append(el("span",label(item)),el("small",status(item)));li.append(b);ul.append(li);}records.replaceChildren(ul);
  }
  function renderRecords(){
   const s=stateAt(recording,index);panel.dataset.mode=view;panel.scrollTop=0;
   if(view==="roadmap"){
    if(compact.matches){list(s.chunks,c=>c.title,c=>statusLabels[c.status]??c.status,openChunk,"No roadmap has been saved at this point.");return;}
    const all=new Map(s.chunks.map(c=>[c.id,c]));
    const items=s.chunks.map(c=>({id:c.id,readiness:{state:c.status==="pending"?(c.deps.every(d=>all.get(d)?.status==="done")?"ready":"waiting"):c.status}}));
    const readiness={ready:items.filter(i=>i.readiness.state==="ready").length,waiting:items.filter(i=>i.readiness.state==="waiting").length,blocked:items.filter(i=>i.readiness.state==="blocked").length};
    const frontier={group:"Website example",items,activity:{readiness,next_ready:null}};
    const node=roadmap.render(frontier,{frontiers:[frontier]},s.chunks,true,(id:string)=>{const c=all.get(id);if(c)openChunk(c);},"recording",undefined);
    node.querySelector(".surface-heading h3")!.textContent="Tic-tac-toe";
    records.replaceChildren(node);return;
   }
   if(view==="reasoning"){
    if(!s.steps.length){records.replaceChildren(el("p","No reasoning has been saved at this point.","record-empty"));return;}
    const references={find:(n:number)=>s.steps.find(x=>x.step_number===n),open:(step:any)=>openStep(step)};
    const branch=s.branches.find(b=>b.id===events[index].step?.branch_id)??s.branches[0];
    const visible=branch?s.steps.filter(step=>step.branch_id===branch.id||step.step_number===branch.merged_into):s.steps;
    if(compact.matches){list(visible,x=>x.purpose,x=>x.branch_id?"Investigation":"Conclusion",openStep,"No reasoning has been saved at this point.");return;}
    records.replaceChildren(reasoning.render({session_id:"tic-tac-toe",steps:s.steps,branches:s.branches},visible,openStep,branch,references,null));return;
   }
   if(view==="checks"){list([...s.checks].reverse(),c=>c.name,c=>(c.passed?"Passed":"Failed")+(c.verified?" · Ran":" · Review"),openCheck,"No checks have run at this point.");return;}
   if(view==="work"){
    list(s.tasks,t=>t.title,t=>statusLabels[t.status]??t.status,t=>{
     const a=reader(t.title,"work");a.append(el("p",statusLabels[t.status]??t.status,"record-status"));
     const checks=s.checks.filter(c=>c.task===t.id);if(checks.length){a.append(el("h4","Recorded checks"));for(const c of checks)a.append(button((c.passed?"Passed: ":"Failed: ")+c.name,()=>openCheck(c)));}
    },"No implementation task has started at this point.");
   }
  }
  function renderSession(){
   const e=events[index],body=preview(e);session.replaceChildren(el("p",(e.kind==="change"?"Workspace":"Magistr")+" · "+kindLabels[e.kind],"terminal-context"),el("h3",e.title),el("pre",body.text));
   const destination=({roadmap:"roadmap",reasoning:"reasoning",check:"checks",work:"work"} as Record<string,string>)[e.kind];
   if(destination){const b=button(({roadmap:"Open the roadmap",reasoning:"Open the reasoning",check:"Inspect this check",work:"Open the work plan"} as any)[e.kind],()=>openView(destination,true));b.className="action-open";session.append(b);}
   if(body.omitted>0||e.kind==="change"){const b=button(e.kind==="change"?"Read the recorded file":"Read the full output",()=>{session.hidden=true;records.hidden=false;const a=reader(e.title,"session");a.append(el("pre",e.output));});b.className="action-open";session.append(b);}
  }
  function render(){
   root.dataset.index=String(index);root.dataset.view=view;slider.value=String(index);slider.setAttribute("aria-valuetext",elapsed(recording,index)+" · "+events[index].title);
   root.querySelector("[data-time]")!.textContent=elapsed(recording,index);
   prev.disabled=index===0;next.disabled=index===events.length-1;
   const chapter=parts.reduce((found,p,i)=>index>=p.index?i:found,0);
   chapterButtons.forEach((b,i)=>{if(i===chapter)b.setAttribute("aria-current","step");else b.removeAttribute("aria-current");});
   root.querySelector("[data-explanation]")!.textContent=parts[chapter].note;
   tabs.forEach(b=>{const active=b.dataset.view===view;b.setAttribute("aria-selected",String(active));b.tabIndex=active?0:-1;});
   panel.setAttribute("aria-labelledby","build-tab-"+view);panel.dataset.mode=view;session.hidden=view!=="session";records.hidden=view==="session";
   if(view==="session")renderSession();else renderRecords();
  }
  function seek(value:number,announce=false){index=Math.max(0,Math.min(events.length-1,value));selected=undefined;render();if(announce)root.querySelector(".build-announcement")!.textContent=events[index].title;}
  const schedule=()=>{timer=setTimeout(()=>{if(index>=events.length-1){stop();return;}seek(index+1);schedule();},2400);};
  play.onclick=()=>{if(timer){stop();return;}if(index===events.length-1)seek(0);play.textContent="Pause";play.setAttribute("aria-pressed","true");schedule();};
  slider.oninput=()=>{stop();seek(Number(slider.value));};prev.onclick=()=>{stop();seek(index-1,true);};next.onclick=()=>{stop();seek(index+1,true);};
  for(const b of chapterButtons){b.disabled=false;b.onclick=()=>{stop();seek(parts[Number(b.dataset.chapter)].index,true);};}
  tabs.forEach((b,i)=>{b.onclick=()=>openView(b.dataset.view!);b.onkeydown=e=>{let j=i;if(e.key==="ArrowRight")j=(i+1)%tabs.length;else if(e.key==="ArrowLeft")j=(i-1+tabs.length)%tabs.length;else if(e.key==="Home")j=0;else if(e.key==="End")j=tabs.length-1;else return;e.preventDefault();tabs[j].focus();openView(tabs[j].dataset.view!);};});
  compact.addEventListener("change",()=>{if(view!=="session"){selected=undefined;render();}});
  const theme=new MutationObserver(()=>{document.documentElement.dataset.theme=document.documentElement.classList.contains("dark")?"dark":"light";});theme.observe(document.documentElement,{attributes:true,attributeFilter:["class"]});document.documentElement.dataset.theme=document.documentElement.classList.contains("dark")?"dark":"light";
  document.addEventListener("visibilitychange",()=>{if(document.hidden)stop();});
  root.querySelector<HTMLElement>(".build-views")!.hidden=false;root.querySelector<HTMLElement>(".build-transport")!.hidden=false;root.dataset.ready="true";render();
 }catch(error){failure.hidden=false;failure.textContent="The interactive recording could not load. You can still read the recorded actions below.";root.dataset.ready="error";}
}
