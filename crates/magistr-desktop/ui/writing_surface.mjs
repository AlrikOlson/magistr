const measured=new WeakMap();
export function renderWritingMeasure(draft,output){
  if(measured.get(draft)===draft.value)return;
  measured.set(draft,draft.value);
  let characters=0,lines=1;
  for(const character of draft.value){characters++;if(character==="\n")lines++;}
  output.textContent=characters.toLocaleString()+" "+(characters===1?"character":"characters")+" · "+lines.toLocaleString()+" "+(lines===1?"line":"lines");
}
// Expand the existing native editor; draft content and persistence retain their single owner.
export function attachWritingSurface(form,draft,button,transcript,latest){
  let composing=false;
  const measure=()=>renderWritingMeasure(draft,form.querySelector("#draft-measure"));
  draft.addEventListener("input",measure);measure();
  const setExpanded=expanded=>{
    form.dataset.expanded=String(expanded);measure();
    button.textContent=expanded?"Done writing":"Expand";
    button.setAttribute("aria-label",expanded?"Close expanded editor":"Expand editor");
    button.setAttribute("aria-expanded",String(expanded));
    transcript.inert=expanded;latest.inert=expanded;
    if(expanded)draft.focus({preventScroll:true});else button.focus({preventScroll:true});
  };
  button.onclick=()=>setExpanded(form.dataset.expanded!=="true");
  form.addEventListener("compositionstart",()=>{composing=true;});
  form.addEventListener("compositionend",()=>{composing=false;});
  form.addEventListener("keydown",event=>{
    if(event.key==="Escape"&&form.dataset.expanded==="true"&&!composing&&!event.isComposing){
      event.preventDefault();event.stopPropagation();setExpanded(false);
    }
  });
}
