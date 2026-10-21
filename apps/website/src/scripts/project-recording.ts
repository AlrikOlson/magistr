import {chapters,checkOf,elapsed,eventStatus,eventText,filesAt,milestonesAt} from "../data/project-recording.mjs";
const root=document.querySelector<HTMLElement>("[data-recording]");
if(root){
 const $=<T extends HTMLElement>(selector:string)=>root.querySelector<T>(selector)!;
 const video=$<HTMLVideoElement>("video");
 const theme=()=>{video.poster=document.documentElement.classList.contains("dark")?video.dataset.posterDark!:video.dataset.posterLight!;};
 theme();
 new MutationObserver(theme).observe(document.documentElement,{attributes:true,attributeFilter:["class"]});
 const initialize=async()=>{
  try{
   const response=await fetch(root.dataset.recordingUrl!);
   if(!response.ok)throw new Error("Recording unavailable");
   const recording=await response.json();
   if(!Array.isArray(recording.events)||!recording.events.length)throw new Error("Recording unavailable");
   const events=recording.events;
   let index=events.length-1, playing=false, timer:ReturnType<typeof setTimeout>|undefined, view="app";
   const range=$<HTMLInputElement>("[data-scrub]"), canvas=$<HTMLCanvasElement>("[data-timeline]"), file=$<HTMLSelectElement>("[data-file]");
   const category={plan:"Plan",start:"Start work",finish:"Completed work",write:"File written",edit:"Source edit",check:"Check",decision:"Decision"};
   const setView=(next:string)=>{
    view=next;
    $(".recording-action").hidden=next!=="action";
    $(".recording-app").hidden=next!=="app";
    root.querySelectorAll<HTMLButtonElement>("[data-view]").forEach(button=>button.setAttribute("aria-pressed",String(button.dataset.view===next)));
    if(next!=="app")video.pause();
   };
   const draw=()=>{
    const rect=canvas.getBoundingClientRect(), dpr=Math.min(devicePixelRatio||1,2);
    if(!rect.width)return;
    canvas.width=Math.round(rect.width*dpr);canvas.height=Math.round(rect.height*dpr);
    const context=canvas.getContext("2d")!;context.scale(dpr,dpr);
    const css=getComputedStyle(root);
    const color=(name:string)=>css.getPropertyValue(name).trim();
    const gap=rect.width/events.length;
    events.forEach((event:any,i:number)=>{
     const check=checkOf(event), height=event.kind==="check"?36:event.kind==="write"||event.kind==="edit"?28:16;
     context.globalAlpha=i<=index?1:.35;
     context.fillStyle=check?color(check.passed?"--color-green":"--color-error"):color("--color-border-hover");
     context.fillRect(i*gap+2,45-height,Math.max(2,gap-4),height);
    });
    context.globalAlpha=1;context.fillStyle=color("--color-accent");
    context.fillRect(index*gap,0,Math.max(2,gap),3);
    canvas.dataset.actions=String(events.length);
   };
   const displayOutput=()=>{
    const event=events[index], snapshots=filesAt(recording,index);
    $("[data-output]").textContent=file.value?snapshots[file.value]:eventText(event);
    $("[data-source-note]").textContent=file.value?"File at this point in the recording":event.kind==="edit"?"Recorded before / after":event.kind==="write"?"Exact written source":"Actual recorded output";
   };
   const render=(announce=false)=>{
    const event=events[index], check=checkOf(event), snapshots=filesAt(recording,index);
    root.dataset.index=String(index);root.dataset.playing=String(playing);
    range.value=String(index);range.setAttribute("aria-valuetext","Action "+(index+1)+" of "+events.length+": "+event.title);
    $("[data-position]").textContent="Action "+(index+1)+" / "+events.length+" · "+elapsed(recording,event);
    $<HTMLButtonElement>("[data-previous]").disabled=index===0;
    $<HTMLButtonElement>("[data-next]").disabled=index===events.length-1;
    $("[data-play]").textContent=playing?"Ⅱ Pause":"▶ Play the build";
    $("[data-play]").setAttribute("aria-pressed",String(playing));
    $("[data-action-title]").textContent=event.title;
    $("[data-action-meta]").textContent=event.at.slice(11,19)+" UTC · "+category[event.kind];
    const status=eventStatus(event);
    $("[data-action-status]").textContent=status;$("[data-action-status]").dataset.result=status;
    const previousFile=file.value;
    file.replaceChildren(new Option("This action",""),...Object.keys(snapshots).map(path=>new Option(path.replace("examples/support-inbox/",""),path)));
    if(snapshots[previousFile]!==undefined)file.value=previousFile;
    displayOutput();
    $("[data-receipt]").textContent=check?"Recorded check · "+(check.verified?"Executed by Magistr":"Visual / planning review")+" · "+check.receipt_id:
      event.output?.results?.[0]?.contentHash?"Saved source · SHA-256 "+event.output.results[0].contentHash:"";
    $("[data-context]").textContent=chapters.find(chapter=>chapter.id===event.chapter)?.summary||"";
    const states=milestonesAt(recording,index);
    root.querySelectorAll<HTMLButtonElement>("[data-chapter]").forEach(button=>{
     const state=states.find(item=>item.id===button.dataset.chapter)!;
     button.dataset.status=state.status;button.querySelector("[data-milestone-status]")!.textContent=state.status;
     if(button.dataset.chapter===event.chapter)button.setAttribute("aria-current","step");else button.removeAttribute("aria-current");
    });
    root.querySelectorAll<HTMLButtonElement>("[data-event]").forEach(button=>{
     if(Number(button.dataset.event)===index)button.setAttribute("aria-current","true");else button.removeAttribute("aria-current");
    });
    const active=$<HTMLButtonElement>('[data-event="'+index+'"]'), list=$(".recording-events");
    list.scrollTop=active.offsetTop-list.offsetTop-list.clientHeight/2+active.clientHeight/2;
    draw();
    if(announce)$("[data-announcement]").textContent=event.title+". "+status+".";
   };
   const stop=()=>{playing=false;clearTimeout(timer);root.dataset.playing="false";$("[data-play]").textContent="▶ Play the build";$("[data-play]").setAttribute("aria-pressed","false");};
   const go=(next:number,announce=true)=>{stop();index=Math.max(0,Math.min(events.length-1,next));file.value="";setView("action");render(announce);};
   const tick=()=>{if(!playing)return;if(index>=events.length-1){stop();return;}index++;file.value="";render();timer=setTimeout(tick,1600/Number($<HTMLSelectElement>("[data-speed]").value));};
   $("[data-play]").addEventListener("click",()=>{
    if(playing){stop();return;}
    if(index===events.length-1)index=0;
    playing=true;file.value="";setView("action");render(true);timer=setTimeout(tick,1600/Number($<HTMLSelectElement>("[data-speed]").value));
   });
   $("[data-previous]").addEventListener("click",()=>go(index-1));$("[data-next]").addEventListener("click",()=>go(index+1));
   range.addEventListener("input",()=>go(Number(range.value)));
   canvas.addEventListener("pointerdown",event=>{const rect=canvas.getBoundingClientRect();go(Math.floor((event.clientX-rect.left)/rect.width*events.length));});
   root.querySelectorAll<HTMLButtonElement>("[data-chapter]").forEach(button=>button.addEventListener("click",()=>go(events.findIndex((event:any)=>event.chapter===button.dataset.chapter))));
   root.querySelectorAll<HTMLButtonElement>("[data-event]").forEach(button=>button.addEventListener("click",()=>go(Number(button.dataset.event))));
   root.querySelectorAll<HTMLButtonElement>("[data-view]").forEach(button=>button.addEventListener("click",()=>{stop();setView(button.dataset.view!);}));
   file.addEventListener("change",displayOutput);
   video.addEventListener("play",stop);
   document.addEventListener("visibilitychange",()=>{if(document.hidden)stop();});
   for(const selector of [".recording-toolbar",".recording-timeline",".recording-chapters",".recording-events",".recording-views"])$(selector).hidden=false;
   new ResizeObserver(draw).observe(canvas);
   new MutationObserver(draw).observe(document.documentElement,{attributes:true,attributeFilter:["class"]});
   setView(view);render();root.dataset.ready="true";
  }catch{
   $("[data-error]").hidden=false;
   $("[data-error]").textContent="The build playback couldn’t load. You can still watch the app above or download the full recording below.";
  }
 };
 initialize();
}
