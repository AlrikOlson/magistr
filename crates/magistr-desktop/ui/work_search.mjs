import {element} from "./work_dom.mjs";
export const normalized=value=>String(value??"").normalize("NFKC").toLocaleLowerCase();
export function matchesText(value,query){const text=normalized(value);return normalized(query).trim().split(/\s+/u).every(word=>text.includes(word));}
export function pageControls(total,page,change,size=50){
  const pages=Math.max(1,Math.ceil(total/size)),current=Math.min(Math.max(0,page),pages-1),nav=element("nav","","work-pages");
  nav.setAttribute("aria-label","Result pages");
  for(const [label,to,disabled] of [["Previous page",current-1,current===0],["Next page",current+1,current===pages-1]]){
    const button=element("button",label);button.disabled=disabled;button.dataset.focusKey=label;button.onclick=()=>change(to);nav.append(button);
  }
  const state=element("span","Page "+(current+1)+" of "+pages+" · "+total+" results");state.setAttribute("role","status");nav.insertBefore(state,nav.lastChild);
  return {nav,start:current*size,end:(current+1)*size,page:current};
}
