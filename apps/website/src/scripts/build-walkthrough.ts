import {stateAt,chapters,elapsed,kindLabels,statusLabels} from "../data/build-recording.mjs";
import {RoadmapSurface} from "../../../../crates/magistr-desktop/ui/work_roadmap_surface.mjs";
import {ReasoningMap} from "../../../../crates/magistr-desktop/ui/work_reasoning_map.mjs";
import "../../../../crates/magistr-desktop/ui/work_surface.css";
import {TerminalPlayback} from "./terminal-playback";
import {explanationFor,explanationHTML} from "../data/build-explanations.mjs";

const el=(tag:string,text="",cls="")=>{const node=document.createElement(tag);node.textContent=text;if(cls)node.className=cls;return node;};
const root=document.querySelector<HTMLElement>("[data-build]");
if(root)setup(root);

async function setup(root:HTMLElement){
 const failure=root.querySelector<HTMLElement>(".build-error")!;
 try{
  const responses=await Promise.all([fetch(root.dataset.url!),fetch(root.dataset.playbackUrl!)]);
  if(responses.some(r=>!r.ok))throw Error("Recording unavailable");
  const [recording,playback]=await Promise.all(responses.map(r=>r.json()));
  if(recording.version!==1||playback.version!==2||!recording.events?.length||playback.events?.length!==recording.events.length||playback.events.some((e:any,i:number)=>e.id!==recording.events[i].id))throw Error("Recording unavailable");
  const events=recording.events,parts=chapters(recording);
 events.forEach(e=>explanationFor(e.id));
  const host=root.querySelector<HTMLElement>("[data-terminal]")!,details=root.querySelector<HTMLDialogElement>("#build-details")!;
  const panel=root.querySelector<HTMLElement>("#build-panel")!,records=root.querySelector<HTMLElement>("[data-record-view]")!;
  const slider=root.querySelector<HTMLInputElement>('input[type="range"]')!,play=root.querySelector<HTMLButtonElement>("[data-play]")!;
  const prev=root.querySelector<HTMLButtonElement>("[data-prev]")!,next=root.querySelector<HTMLButtonElement>("[data-next]")!,speed=root.querySelector<HTMLSelectElement>("[data-speed]")!;
  const views=[...root.querySelectorAll<HTMLButtonElement>("[data-view]")],chapterButtons=[...root.querySelectorAll<HTMLButtonElement>("[data-chapter]")];
  const openAction=root.querySelector<HTMLButtonElement>("[data-open-action]")!;
  const compact=matchMedia("(max-width:1000px)");
  let index=Number(root.dataset.index),view="",timer:ReturnType<typeof setTimeout>|undefined,playing=false,ended=false,cycle=0;
  let terminal:TerminalPlayback;
  let opener:HTMLElement=openAction;
  const roadmap=new RoadmapSurface(()=>renderRecords()),reasoning=new ReasoningMap();
  const button=(text:string,fn:()=>void)=>{const b=el("button",text) as HTMLButtonElement;b.type="button";b.onclick=fn;return b;};
  const stop=()=>{playing=false;++cycle;clearTimeout(timer);timer=undefined;terminal?.pause();play.textContent="Play";play.setAttribute("aria-pressed","false");};
  const cloneOutput=(id:string,diff=false)=>{
   const source=root.querySelector<HTMLElement>('[data-event="'+CSS.escape(id)+'"] [data-event-'+(diff?"diff":"output")+"]")!;
   return source.firstElementChild!.cloneNode(true) as HTMLElement;
  };
  const closeDetails=(focus=true)=>{view="";details.close();document.documentElement.classList.remove("record-dialog-open");root.dataset.view="session";views.forEach(b=>b.setAttribute("aria-expanded","false"));if(focus)opener.focus({preventScroll:true});};
  const showDetails=(name:string,focus=true)=>{
   stop();view=name;if(!details.open){details.showModal();document.documentElement.classList.add("record-dialog-open");}root.dataset.view=name;panel.dataset.mode=name;panel.scrollTop=0;
   root.querySelector("#build-detail-title")!.textContent=({roadmap:"Roadmap",reasoning:"Reasoning",work:"Work",checks:"Checks",file:"Changes",output:"Session message",transcript:"Full session"} as any)[name]+" · "+elapsed(recording,index);
   views.forEach(b=>b.setAttribute("aria-expanded",String(b.dataset.view===name)));
   if(focus){panel.focus({preventScroll:true});}
  };
  const openView=(name:string,focus=true)=>{showDetails(name,false);renderRecords();if(focus){panel.focus({preventScroll:true});}};
  const reader=(title:string,back:string)=>{
   panel.dataset.mode="reader";panel.scrollTop=0;
   const article=el("article","","record-reader");
   if(back!=="recording")article.append(button("← Back to "+back,()=>{renderRecords();panel.focus({preventScroll:true});}));
   article.append(el("h3",title));
   records.replaceChildren(article);article.tabIndex=-1;article.focus({preventScroll:true});return article;
  };
  const paragraph=(article:HTMLElement,label:string,value:string)=>{if(value)article.append(el("h4",label),el("p",value));};
  const copyButton=(text:string,label:string)=>{
   const b=button(label,async()=>{try{await navigator.clipboard.writeText(text);b.textContent="Copied";}catch{b.textContent="Select the text to copy";}});
   return b;
  };
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
  function openCheck(check:any){
   const a=reader(check.name,view);
   a.append(el("p",(check.passed?"Passed":"Failed")+" · "+(check.verified?"Automated check":"Visual review"),"record-status"),copyButton(check.details,"Copy output"));
   a.append(check.eventId?cloneOutput(check.eventId):el("pre",check.details));
  }
  function openFile(mode="changes"){
   showDetails("file",false);
   const e=events[index],a=reader(e.file.path,"recording"),controls=el("div","","file-views");a.classList.add("file-reader");a.querySelector("h3")!.remove();
   root.querySelector("#build-detail-title")!.textContent=e.file.path+" · "+elapsed(recording,index);
   for(const [value,label]of [["changes","Changes"],["file","Full file"]]){
    const b=button(label,()=>openFile(value));b.setAttribute("aria-pressed",String(mode===value));controls.append(b);
   }
   controls.append(copyButton(e.file.content,"Copy file"));a.append(controls);
   if(mode==="changes")a.append(el("p",playback.events[index].changes.first?"First recorded version.":"Compared with the previous recorded version.","record-status"));
   a.append(cloneOutput(e.id,mode==="changes"));
  }
  function openOutput(){
   showDetails("output",false);const e=events[index],a=reader(e.kind==="message"?(e.role==="user"?"You":"Codex"):e.title,"recording");a.append(copyButton(e.output,e.kind==="message"?"Copy message":"Copy output"),cloneOutput(e.id));
  }
  function list(items:any[],label:(x:any)=>string,status:(x:any)=>string,open:(x:any)=>void,empty:string){
   if(!items.length){records.replaceChildren(el("p",empty,"record-empty"));return;}
   const ul=el("ul","","record-list");
   for(const item of items){const li=el("li"),b=button("",()=>open(item));b.append(el("span",label(item)),el("small",status(item)));li.append(b);ul.append(li);}
   records.replaceChildren(ul);
  }
  function renderRecords(){
   const s=stateAt(recording,index);panel.dataset.mode=view;panel.scrollTop=0;
   if(view==="transcript"){
    const list=el("ol","","session-conversation");
    for(const [i,e]of events.entries()){
     const item=el("li");item.dataset.role=e.role??"tool";
     const heading=el("div","","conversation-heading");
     heading.append(el("strong",e.kind==="message"?(e.role==="user"?"You":"Codex"):explanationFor(e.id).title),button(elapsed(recording,i)+" ↗",()=>{closeDetails();seek(i,true);}));
     item.append(heading);
     if(e.kind==="message")item.append(cloneOutput(e.id));
     list.append(item);
    }
    records.replaceChildren(list);return;
   }
   if(view==="roadmap"){
    if(!s.chunks.length){records.replaceChildren(el("p","No roadmap has been saved at this point.","record-empty"));return;}
    if(compact.matches){list(s.chunks,c=>c.title,c=>statusLabels[c.status]??c.status,openChunk,"No roadmap has been saved at this point.");return;}
    const all=new Map(s.chunks.map(c=>[c.id,c]));
    const items=s.chunks.map(c=>({id:c.id,readiness:{state:c.status==="pending"?(c.deps.every(d=>all.get(d)?.status==="done")?"ready":"waiting"):c.status}}));
    const readiness={ready:items.filter(i=>i.readiness.state==="ready").length,waiting:items.filter(i=>i.readiness.state==="waiting").length,blocked:items.filter(i=>i.readiness.state==="blocked").length};
    const frontier={group:"Website example",items,activity:{readiness,next_ready:null}};
    const node=roadmap.render(frontier,{frontiers:[frontier]},s.chunks,true,(id:string)=>{const c=all.get(id);if(c)openChunk(c);},"recording",undefined);
    node.querySelector(".surface-heading h3")!.textContent="Tic-tac-toe";records.replaceChildren(node);return;
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
  function annotate(){
   const e=events[index],s=stateAt(recording,index),facts=root.querySelector<HTMLElement>("[data-action-facts]")!;
   root.querySelector("[data-action-kind]")!.textContent=e.kind==="message"?(e.role==="user"?"You":"Codex"):kindLabels[e.kind];
   root.querySelector("[data-action-number]")!.textContent=(index+1)+" / "+events.length;
   const explanation=explanationFor(e.id);
   root.querySelector("[data-action-title]")!.textContent=explanation.title;
   root.querySelector("[data-action-explanation]")!.innerHTML=explanationHTML(explanation);
   facts.replaceChildren();
   if(e.operation==="roadmap_add_chunk"){
    const ul=el("ul","","annotation-roadmap");
    for(const c of s.chunks){const li=el("li");li.dataset.status=c.status;li.append(el("span",c.name??c.title),el("small",statusLabels[c.status]??c.status));ul.append(li);}facts.append(ul);
   }else if(e.file){
    const change=playback.events[index].changes;
    facts.append(el("p",e.file.path,"annotation-file"));
    const counts=el("p","","change-counts");counts.append(el("span","+"+change.added+" added","change-added"),el("span","−"+change.removed+" removed","change-removed"));facts.append(counts);
   }else if(e.check){
    facts.append(el("p",(e.check.passed?"✓ Passed":"× Failed")+(e.check.verified?" · Automated check":" · Visual review"),e.check.passed?"check-passed":"check-failed"));
   }
   const target=({roadmap:"roadmap",reasoning:"reasoning",work:"work",check:"checks"} as Record<string,string>)[e.kind];
   openAction.textContent=e.file?"Open changes":e.check?"Inspect this check":target?"Open "+({roadmap:"the roadmap",reasoning:"the reasoning",work:"the work plan"} as any)[e.kind]:e.kind==="message"?"Read message":"Read full output";
   openAction.onclick=()=>{
    opener=openAction;
    if(e.file)openFile();
    else if(e.check){showDetails("checks",false);openCheck({...e.check,eventId:e.id});}
    else if(target)openView(target,false);
    else openOutput();
    panel.focus({preventScroll:true});
   };
  }
  function render(){
   root.dataset.index=String(index);slider.value=String(index);slider.setAttribute("aria-valuetext",elapsed(recording,index)+" · "+explanationFor(events[index].id).title);
   root.querySelector("[data-time]")!.textContent=elapsed(recording,index);
   prev.disabled=index===0;next.disabled=index===events.length-1;
   const chapter=parts.reduce((found,p,i)=>index>=p.index?i:found,0);
   chapterButtons.forEach((b,i)=>{if(i===chapter)b.setAttribute("aria-current","step");else b.removeAttribute("aria-current");});
   root.querySelector("[data-explanation]")!.textContent=parts[chapter].note;
   annotate();
   if(view){showDetails(view,false);renderRecords();}
  }
  const seek=(value:number,announce=false)=>{
   ended=false;index=Math.max(0,Math.min(events.length-1,value));if(view==="file"||view==="output")closeDetails(false);
   render();void terminal.seek(index);if(announce)root.querySelector(".build-announcement")!.textContent=explanationFor(events[index].id).title;
  };
  const playCurrent=async()=>{
   const run=++cycle;
   await terminal.play(index,Number(speed.value));
   if(!playing||run!==cycle)return;
   if(index>=events.length-1){ended=true;stop();return;}
   const delay=Math.max(800,Math.min(4000,(Date.parse(events[index+1].at)-Date.parse(events[index].at))))/Number(speed.value);
   timer=setTimeout(()=>{timer=undefined;index++;render();void playCurrent();},delay);
  };
  const toggle=()=>{
   if(playing){stop();return;}
   if(details.open)closeDetails(false);
   if(ended)seek(0);
   playing=true;play.textContent="Pause";play.setAttribute("aria-pressed","true");void playCurrent();
  };
  await document.fonts.ready;host.hidden=false;
  terminal=new TerminalPlayback(host,playback.events,key=>{if(key===" ")toggle();else{stop();seek(index+(key==="ArrowRight"?1:-1),true);}},stop);
  play.onclick=toggle;
  slider.oninput=()=>{stop();seek(Number(slider.value));};prev.onclick=()=>{stop();seek(index-1,true);};next.onclick=()=>{stop();seek(index+1,true);};
  speed.onchange=()=>{terminal.setSpeed(Number(speed.value));if(playing&&timer){clearTimeout(timer);timer=undefined;void playCurrent();}};
  for(const b of chapterButtons){b.disabled=false;b.onclick=()=>{stop();seek(parts[Number(b.dataset.chapter)].index,true);};}
  for(const b of views)b.onclick=()=>{opener=b;if(view===b.dataset.view)closeDetails();else openView(b.dataset.view!);};
  root.querySelector<HTMLButtonElement>("[data-close-details]")!.onclick=()=>closeDetails();
  details.addEventListener("cancel",event=>{event.preventDefault();closeDetails();});
  let backdrop=false;
  details.addEventListener("pointerdown",event=>{const r=details.getBoundingClientRect();backdrop=event.target===details&&(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom);});
  details.addEventListener("click",event=>{if(backdrop&&event.target===details)closeDetails();backdrop=false;});
  const transcriptButton=root.querySelector<HTMLButtonElement>("[data-open-transcript]")!;
  transcriptButton.hidden=false;transcriptButton.onclick=()=>{opener=transcriptButton;openView("transcript");};
  root.querySelector<HTMLElement>("[data-transcript]")!.hidden=true;
  compact.addEventListener("change",()=>{if(view&&view!=="file"&&view!=="output")renderRecords();});
  const syncTheme=()=>{document.documentElement.dataset.theme=document.documentElement.classList.contains("dark")?"dark":"light";};
  const theme=new MutationObserver(syncTheme);theme.observe(document.documentElement,{attributes:true,attributeFilter:["class"]});syncTheme();
  document.addEventListener("visibilitychange",()=>{if(document.hidden)stop();});
  for(const selector of [".build-views",".build-transport","[data-terminal-help]","[data-open-action]"])root.querySelector<HTMLElement>(selector)!.hidden=false;
  root.querySelector<HTMLElement>("[data-terminal-poster]")!.hidden=true;
  render();await terminal.seek(index);root.dataset.ready="true";
 }catch(error){
  root.querySelector<HTMLElement>("[data-terminal]")!.hidden=true;
  root.querySelector<HTMLElement>("[data-terminal-poster]")!.hidden=false;
  failure.hidden=false;failure.textContent="The interactive recording could not load. You can still read the full session below.";root.dataset.ready="error";
 }
}
