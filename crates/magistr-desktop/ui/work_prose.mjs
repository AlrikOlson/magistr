import {element} from "./work_dom.mjs";
export function prose(label,value,key,copy){
  const section=element("section","","reasoning-prose"),header=element("header");header.append(element("h4",label));
  const button=element("button","Copy");button.setAttribute("aria-label","Copy "+label.toLowerCase());button.onclick=()=>copy(value);header.append(button);section.append(header);
  const body=element("div",value,"reasoning-text");body.dataset.reading=key;section.append(body);return section;
}
