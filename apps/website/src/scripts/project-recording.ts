import {checkOf,filesAt} from "../data/project-recording.mjs";
import {sessionEntries,moments,momentIndex,timeLabel,entryTitle,entryBody,projectAt} from "../data/session-replay.mjs";

const root=document.querySelector<HTMLElement>("[data-recording]");
if(root){
 const $=<T extends HTMLElement>(selector:string)=>root.querySelector<T>(selector)!;
 const play=$<HTMLButtonElement>("[data-play]");
 const range=$<HTMLInputElement>("#session-position");
 const actionSelect=$<HTMLSelectElement>("[data-action-select]");
 const file=$<HTMLSelectElement>("[data-file]");
 const inspector=$<HTMLDetailsElement>("[data-inspector]");
 const scroll=$(".session-scroll");
 const rows=[...root.querySelectorAll<HTMLElement>("[data-entry]")];
 let playing=false, timer:ReturnType<typeof setTimeout>|undefined;
 const pause=()=>{
  playing=false;clearTimeout(timer);root.dataset.playing="false";
  play.textContent="Play";play.setAttribute("aria-pressed","false");
  $("[data-session-status]").textContent="Paused";
 };
 const initialize=async()=>{
  try{
   const response=await fetch(root.dataset.recordingUrl!);
   if(!response.ok)throw new Error("Recording unavailable");
   const recording=await response.json();
   if(recording.schemaVersion!==2||recording.events?.length!==47||recording.messages?.length!==6)throw new Error("Recording unavailable");
   const entries=sessionEntries(recording);
   let cursor=momentIndex(entries,moments[0]);
   const snapshots=entries.map(entry=>filesAt(recording,entry.eventIndex));
   const source=()=>{
    const entry=entries[cursor],path=file.value;
    $("[data-output]").textContent=path?snapshots[cursor][path]:entryBody(entry);
    $("[data-source-note]").textContent=path?"File as it existed at "+timeLabel(entries,cursor)+" · "+path.replace("examples/support-inbox/",""):
     entry.kind==="message"?(entry.role==="user"?"User request":"Codex message"):entry.event.kind==="edit"?"Recorded edit · before and after":"Recorded action and result";
    const check=entry.kind==="action"?checkOf(entry.event):undefined;
    $("[data-receipt]").textContent=check&&!path?
     (check.verified?"Executed check":"Recorded review")+" · "+(check.passed?"Passed":"Failed")+" · "+check.receipt_id:"";
   };
   const render=(announce=false,frame=true)=>{
    const entry=entries[cursor],state=projectAt(recording,entry);
    root.dataset.index=String(cursor);root.dataset.event=String(entry.eventIndex);
    range.value=String(cursor);actionSelect.value=String(cursor);
    range.setAttribute("aria-valuetext",timeLabel(entries,cursor)+", "+(cursor+1)+" of "+entries.length+", "+(entry.kind==="message"?entryTitle(entry)+" message":entry.event.title)+", "+state.status);
    $("[data-time]").textContent=timeLabel(entries,cursor)+" / "+timeLabel(entries,entries.length-1);
    $<HTMLButtonElement>("[data-previous]").disabled=cursor===0;
    $<HTMLButtonElement>("[data-next]").disabled=cursor===entries.length-1;
    rows.forEach((row,index)=>{
     row.hidden=index>cursor;row.dataset.selected=String(index===cursor);
     row.querySelector<HTMLElement>(".entry-summary")!.hidden=index===cursor;
     row.querySelector<HTMLElement>(".entry-detail")!.hidden=index!==cursor;
     if(index===cursor)row.setAttribute("aria-current","true");else row.removeAttribute("aria-current");
    });
    const moment= moments.findLastIndex(item=>momentIndex(entries,item)<=cursor);
    $<HTMLSelectElement>("[data-moment-select]").value=String(Math.max(0,moment));
    const taskList=$("[data-task-list]");
    taskList.replaceChildren(...state.tasks.map(task=>{
     const li=document.createElement("li");li.dataset.status=task.status;
     const mark=document.createElement("span");mark.textContent=task.status==="Completed"?"✓":task.status==="In progress"?"→":"·";mark.setAttribute("aria-hidden","true");
     const label=document.createElement("span");label.textContent=task.label;
     const status=document.createElement("small");status.textContent=task.status;
     li.append(mark,label,status);return li;
    }));
    $("[data-no-plan]").hidden=state.tasks.length>0;
    $("[data-task-count]").textContent=state.tasks.length?state.tasks.filter(task=>task.status==="Completed").length+" of "+state.tasks.length+" complete":"No plan yet";
    $("[data-decision]").hidden=!state.decision;
    const failed=state.checks.filter(event=>!checkOf(event).passed).length;
    $("[data-check-count]").textContent=state.checks.length?
     state.checks.length+" checks saved"+(failed?" · "+failed+" failed "+(failed===1?"result":"results")+" kept":""):"No checks recorded yet.";
    const oldFile=file.value;
    file.replaceChildren(new Option("Selected action or message",""),...Object.keys(snapshots[cursor]).map(path=>new Option(path.replace("examples/support-inbox/",""),path)));
    if(oldFile in snapshots[cursor])file.value=oldFile;
    source();
    if(frame){
     const row=rows[cursor];
     scroll.scrollTop=Math.max(0,row.offsetTop-scroll.offsetTop-36);
    }
    if(announce)$("[data-announcement]").textContent=range.getAttribute("aria-valuetext");
   };
   const seek=(index:number)=>{
    pause();cursor=Math.max(0,Math.min(entries.length-1,Math.round(index)));file.value="";render(true);
   };
   const schedule=()=>{
    clearTimeout(timer);
    if(!playing)return;
    if(cursor===entries.length-1){pause();$("[data-session-status]").textContent="End of recorded build";return;}
    const speed=Number($<HTMLSelectElement>("[data-speed]").value);
    const delay=(Date.parse(entries[cursor+1].at)-Date.parse(entries[cursor].at))/speed;
    timer=setTimeout(()=>{if(!playing)return;cursor++;render();schedule();},Math.max(16,delay));
   };
   range.addEventListener("input",()=>seek(Number(range.value)));
   actionSelect.addEventListener("change",()=>seek(Number(actionSelect.value)));
   file.addEventListener("change",()=>{pause();source();});
   $("[data-next]").addEventListener("click",()=>seek(cursor+1));
   $("[data-previous]").addEventListener("click",()=>seek(cursor-1));
   $("[data-moment-select]").addEventListener("change",()=>seek(momentIndex(entries,moments[Number($<HTMLSelectElement>("[data-moment-select]").value)])));
   root.querySelectorAll<HTMLButtonElement>("[data-seek]").forEach(button=>{
    button.disabled=false;button.addEventListener("click",()=>seek(Number(button.dataset.seek)));
   });
   root.querySelectorAll<HTMLButtonElement>("[data-open-action]").forEach(button=>{
    button.hidden=false;button.addEventListener("click",()=>{
     pause();file.value="";source();inspector.open=true;
     actionSelect.focus({preventScroll:true});inspector.scrollIntoView({block:"nearest",behavior:"instant"});
    });
   });
   $("[data-decision]").addEventListener("click",()=>{seek(entries.findIndex(entry=>entry.event?.kind==="decision"));inspector.open=true;actionSelect.focus({preventScroll:true});inspector.scrollIntoView({block:"nearest",behavior:"instant"});});
   play.addEventListener("click",()=>{
    if(playing){pause();return;}
    if(cursor===entries.length-1)cursor=0;
    playing=true;root.dataset.playing="true";play.textContent="Pause";play.setAttribute("aria-pressed","true");
    $("[data-session-status]").textContent="Playing at "+$<HTMLSelectElement>("[data-speed]").value+"×";
    render();schedule();
   });
   $("[data-speed]").addEventListener("change",()=>{if(playing){$("[data-session-status]").textContent="Playing at "+$<HTMLSelectElement>("[data-speed]").value+"×";schedule();}});
   scroll.addEventListener("pointerdown",pause);
   root.addEventListener("focusin",event=>{if(playing&&event.target!==play&&event.target!==$("[data-speed]"))pause();});
   inspector.addEventListener("toggle",()=>{if(inspector.open)pause();});
   document.addEventListener("visibilitychange",()=>{if(document.hidden)pause();});
   new IntersectionObserver(entries=>{if(!entries[0].isIntersecting)pause();}).observe(root);
   root.addEventListener("show-recording",event=>{
    const index=entries.findIndex(entry=>entry.event?.id===(event as CustomEvent<number>).detail);
    if(index>=0)seek(index);
   });
   for(const selector of [".replay-controls",".recording-details"])$(selector).hidden=false;
   $(".complete-transcript").hidden=true;
   render(false,false);root.dataset.ready="true";
  }catch{
   $("[data-error]").textContent="Interactive playback couldn’t load. You can read the complete transcript or download the recording below.";
   $("[data-error]").hidden=false;
  }
 };
 initialize();
}