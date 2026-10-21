import {checkOf,elapsed,eventStatus,eventText,filesAt} from "../data/project-recording.mjs";
import {walkthrough} from "../data/walkthrough.mjs";

const root=document.querySelector<HTMLElement>("[data-recording]");
if(root){
 const $=<T extends HTMLElement>(selector:string)=>root.querySelector<T>(selector)!;
 const video=$<HTMLVideoElement>("video");
 const panels=[...root.querySelectorAll<HTMLElement>("[data-scene-panel]")];
 const chapterButtons=[...root.querySelectorAll<HTMLButtonElement>("[data-chapter]")];
 const details=$<HTMLDetailsElement>(".recording-details");
 let scene=5, intro=true, playing=false, timer:ReturnType<typeof setTimeout>|undefined;
 let review="after";
 const stop=()=>{
  playing=false;clearTimeout(timer);root.dataset.playing="false";
  $("[data-play]").textContent="Play chapters";
  $("[data-play]").setAttribute("aria-pressed","false");
 };
 const originals=()=>{
  const theme=document.documentElement.classList.contains("dark")?"Dark":"Light";
  root.querySelectorAll<HTMLAnchorElement>("[data-capture-original]").forEach(link=>{
   const version=link.dataset.captureOriginal==="3"?"before":link.dataset.captureOriginal==="4"?review:"after";
   const movie=link.dataset.captureOriginal==="5"&&!$("[data-movie]").hidden;
   link.href=movie?video.querySelector("source")!.src:link.dataset[version+theme]!;
   link.textContent=movie?"Open full video ↗":"Open full capture ↗";
  });
 };
 originals();
 new MutationObserver(originals).observe(document.documentElement,{attributes:true,attributeFilter:["class"]});
 const frameCapture=()=>{
  const theme=document.documentElement.classList.contains("dark")?"dark":"light";
  panels[scene].querySelectorAll<HTMLElement>(".capture-window").forEach(frame=>{
   if(frame.hidden)return;
   const img=frame.querySelector<HTMLImageElement>(".capture-"+theme+" img")!;
   img.decode().then(()=>{
    const layout=img.naturalWidth<=600?"compact":"wide";
    if(frame.dataset.framed===layout)return;
    const offset=scene===4?(layout==="compact"?300:220):(layout==="compact"?470:160);
    frame.scrollTop=offset*img.clientWidth/img.naturalWidth;
    frame.dataset.framed=layout;
   }).catch(()=>{/* The original capture link remains available. */});
  });
 };
 const render=(announce=false)=>{
  const chapter=walkthrough[scene];
  root.dataset.scene=String(scene);root.dataset.playing=String(playing);
  panels.forEach((panel,index)=>panel.hidden=index!==scene);
  // A chosen chapter must load even when its stage is above the navigation.
  panels[scene].querySelectorAll<HTMLImageElement>("img").forEach(img=>img.loading="eager");
  frameCapture();
  chapterButtons.forEach((button,index)=>{
   if(index===scene)button.setAttribute("aria-current","step");else button.removeAttribute("aria-current");
  });
  $("[data-story-kicker]").textContent=intro?"One project, from start to finish":"Chapter "+(scene+1)+" / "+walkthrough.length;
  $("[data-story-title]").textContent=chapter.title;
  $("[data-story-description]").textContent=chapter.description;
  $("[data-story-position]").textContent=playing?"8 seconds per chapter":intro?"Choose a chapter or start at the brief":"Chapter "+(scene+1)+" of "+walkthrough.length;
  const next=$("[data-next]");
  next.textContent=intro?"Start the walkthrough →":scene===5?"Back to the brief ↺":"Next: "+walkthrough[scene+1].label+" →";
  $<HTMLButtonElement>("[data-previous]").disabled=intro||scene===0;
  $("[data-watch]").hidden=scene!==5;
  if(scene!==5)video.pause();
  if(announce)$("[data-announcement]").textContent="Chapter "+(scene+1)+": "+chapter.title;
 };
 const hideMovie=()=>{
  video.pause();$("[data-movie]").hidden=true;
  panels[5].querySelector<HTMLElement>(".capture-window")!.hidden=false;
  $("[data-watch]").textContent="Watch the app · 12s";originals();
 };
 const go=(index:number)=>{
  stop();intro=false;scene=Math.max(0,Math.min(5,index));hideMovie();render(true);
 };
 const schedule=()=>{
  clearTimeout(timer);
  timer=setTimeout(()=>{
   if(!playing)return;
   if(scene===5){stop();render();return;}
   scene++;render();schedule();
  },8000);
 };
 const initialize=async()=>{
  try{
   const response=await fetch(root.dataset.recordingUrl!);
   if(!response.ok)throw new Error("Recording unavailable");
   const recording=await response.json();
   if(!Array.isArray(recording.events)||recording.events.length!==47)throw new Error("Recording unavailable");
   const events=recording.events;
   let action=46;
   const actionSelect=$<HTMLSelectElement>("[data-action-select]");
   const file=$<HTMLSelectElement>("[data-file]");
   const output=()=>{
    const event=events[action];
    $("[data-output]").textContent=file.value?filesAt(recording,action)[file.value]:eventText(event);
    $("[data-source-note]").textContent=file.value?"File at this point in the recording":event.kind==="edit"?"Recorded before / after":event.kind==="write"?"Exact written source":"Actual recorded output";
   };
   const inspect=(index:number)=>{
    action=index;root.dataset.index=String(index);actionSelect.value=String(index);
    const event=events[index],check=checkOf(event);
    $("[data-action-title]").textContent=event.title;
    $("[data-action-status]").textContent=eventStatus(event);
    $("[data-position]").textContent="Action "+(index+1)+" / "+events.length+" · "+elapsed(recording,event);
    file.replaceChildren(new Option("This action",""),...Object.keys(filesAt(recording,index)).map(path=>new Option(path.replace("examples/support-inbox/",""),path)));
    $("[data-receipt]").textContent=check?"Recorded check · "+(check.verified?"Executed by Magistr":"Visual / planning review")+" · "+check.receipt_id:
      event.output?.results?.[0]?.contentHash?"Saved source · SHA-256 "+event.output.results[0].contentHash:"";
    output();
   };
   actionSelect.addEventListener("change",()=>{stop();render();inspect(Number(actionSelect.value));});
   file.addEventListener("change",output);
   root.querySelectorAll<HTMLButtonElement>("[data-inspect]").forEach(button=>{
    button.hidden=false;
    button.addEventListener("click",()=>{
     stop();render();inspect(Number(button.dataset.inspect));details.open=true;
     actionSelect.focus({preventScroll:true});
     $(".recording-inspector").scrollIntoView({block:"center",behavior:"instant"});
    });
   });
   $("[data-next]").addEventListener("click",()=>go(intro||scene===5?0:scene+1));
   $("[data-previous]").addEventListener("click",()=>go(scene-1));
   chapterButtons.forEach(button=>button.addEventListener("click",()=>go(Number(button.dataset.chapter))));
   $("[data-play]").addEventListener("click",()=>{
    if(playing){stop();render();return;}
    hideMovie();if(intro||scene===5)scene=0;intro=false;playing=true;
    $("[data-play]").textContent="Pause chapters";$("[data-play]").setAttribute("aria-pressed","true");
    render(true);schedule();
   });
   $("[data-watch]").addEventListener("click",()=>{
    stop();render();
    const movie=$("[data-movie]");
    if(!movie.hidden){hideMovie();return;}
    panels[5].querySelector<HTMLElement>(".capture-window")!.hidden=true;movie.hidden=false;
    $("[data-watch]").textContent="Back to the capture";originals();
    video.play().catch(()=>{/* Native controls remain available when playback is blocked. */});
   });
   root.querySelectorAll<HTMLButtonElement>("[data-review]").forEach(button=>button.addEventListener("click",()=>{
    stop();render();review=button.dataset.review!;
    panels[4].querySelectorAll<HTMLElement>("[data-capture-version]").forEach(image=>image.hidden=image.dataset.captureVersion!==review);
    panels[4].querySelectorAll<HTMLButtonElement>("[data-review]").forEach(option=>option.setAttribute("aria-pressed",String(option===button)));
    $("[data-review-note]").textContent=review==="before"?"Review failed: small menus and a misleading recovery message.":"Menus enlarged; recovery message corrected.";
    originals();frameCapture();
   }));
   details.addEventListener("toggle",()=>{if(details.open){stop();render();}});
   root.addEventListener("focusin",event=>{if(playing&&event.target!==$("[data-play]")){stop();render();}});
   root.querySelectorAll<HTMLElement>(".capture-window").forEach(frame=>frame.addEventListener("pointerdown",()=>{stop();render();}));
   document.addEventListener("visibilitychange",()=>{if(document.hidden){stop();render();video.pause();}});
   new IntersectionObserver(entries=>{if(!entries[0].isIntersecting){stop();render();video.pause();}}).observe(root);
   for(const selector of [".story-controls",".story-chapters",".review-switch",".recording-inspector"])$(selector).hidden=false;
   root.addEventListener("show-recording",event=>go((event as CustomEvent<number>).detail));
   inspect(46);render();root.dataset.ready="true";
  }catch{
   $("[data-error]").hidden=false;
   $("[data-error]").textContent="The walkthrough couldn’t load. You can still view the finished inbox, watch the app recording or download the project below.";
  }
 };
 initialize();
}
