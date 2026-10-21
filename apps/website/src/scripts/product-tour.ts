for(const region of document.querySelectorAll<HTMLElement>("[data-explore]")){
 const buttons=[...region.querySelectorAll<HTMLButtonElement>(":scope > .tour-workspace [data-record-select], :scope > .check-choices [data-record-select]")];
 const details=[...region.querySelectorAll<HTMLElement>(":scope > .tour-workspace [data-record-detail], :scope > [data-record-detail]")];
 if(!buttons.length)continue;
 buttons.forEach(button=>button.disabled=false);
 for(const button of buttons)button.addEventListener("click",()=>{
  const selected=button.dataset.recordSelect;
  buttons.forEach(b=>b.setAttribute("aria-pressed",String(b===button)));
  details.forEach(panel=>panel.hidden=panel.dataset.recordDetail!==selected);
 });
}
for(const link of document.querySelectorAll<HTMLAnchorElement>("[data-playback-event]")){
 link.addEventListener("click",()=>{
  const root=document.querySelector<HTMLElement>("[data-recording]");
  if(root?.dataset.ready==="true")root.dispatchEvent(new CustomEvent("show-recording",{detail:Number(link.dataset.playbackEvent)}));
  document.querySelector<HTMLElement>("#workflow")?.focus({preventScroll:true});
 });
}