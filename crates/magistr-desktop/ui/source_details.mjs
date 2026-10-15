import { paintSource } from "./source_color.mjs";
const views=new WeakMap();
function structured(document,value,depth,budget){
  if(--budget.remaining<0||depth>5)throw Error("Too much structure");
  const node=document.createElement(value&&typeof value==="object"?"dl":"span");
  if(value&&typeof value==="object"){
    node.className="result-fields";
    for(const [key,item] of Object.entries(value)){
      const term=document.createElement("dt"),definition=document.createElement("dd");
      term.textContent=Array.isArray(value)?String(Number(key)+1):key;
      if(item&&typeof item==="object"){
        const details=document.createElement("details"),summary=document.createElement("summary");
        summary.textContent=Object.keys(item).length+(Array.isArray(item)?" items":" fields");details.open=false;
        details.append(summary,structured(document,item,depth+1,budget));definition.append(details);
      }else definition.append(structured(document,item,depth+1,budget));
      node.append(term,definition);
    }
    if(!node.children.length){const empty=document.createElement("span");empty.className="result-value";empty.textContent=Array.isArray(value)?"[]":"{}";return empty;}
  }else {node.className="result-value result-"+(value===null?"null":typeof value);node.textContent=JSON.stringify(value);}
  return node;
}
export function renderSourceDetail(section,detail,copy){
  section.classList.add("source-details");
  const previous=views.get(section);
  if(previous?.source===detail.source&&previous?.label===detail.label)return;
  const document=section.ownerDocument,header=document.createElement("header"),title=document.createElement("h3");
  title.textContent=detail.label;
  const button=document.createElement("button");button.type="button";button.textContent="Copy";button.setAttribute("aria-label","Copy "+detail.label.toLowerCase());button.disabled=!copy;
  button.onclick=()=>copy?.(detail.source);header.append(title,button);
  const content=document.createElement("div");content.className="source-result";
  let fields=null;
  if(detail.source.length<=65536){
    try{const value=JSON.parse(detail.source);if(value&&typeof value==="object")fields=structured(document,value,0,{remaining:200});}catch{}
  }
  const pre=document.createElement("pre");pre.tabIndex=0;pre.setAttribute("aria-label",detail.label);
  if(fields){
    const original=document.createElement("details"),summary=document.createElement("summary");
    original.className="result-original";summary.textContent="Original text";pre.textContent=detail.source;
    original.append(summary,pre);content.append(fields,original);
  }else {paintSource(pre,detail.source,/command/i.test(detail.label)?"shell":"");content.append(pre);}
  section.replaceChildren(header,content);views.set(section,{source:detail.source,label:detail.label});
}
