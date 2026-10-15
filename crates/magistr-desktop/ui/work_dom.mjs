// Small DOM helpers for complete workflow records.
import { captureSelection,restoreSelection } from "./reading.mjs";
export function element(tag,value="",className=""){
  const node=document.createElement(tag);node.textContent=value;
  if(className)node.className=className;return node;
}
export function saveReading(root){
  return {selection:captureSelection(root),open:[...root.querySelectorAll("details[open][data-reading-key]")].map(node=>node.dataset.readingKey),
    focus:root.contains(document.activeElement)?document.activeElement.dataset.focusKey:null,scroll:root.scrollTop};
}
export function restoreReading(root,saved){
  for(const details of root.querySelectorAll("details[data-reading-key]"))details.open=saved.open.includes(details.dataset.readingKey);
  if(saved.focus){const target=[...root.querySelectorAll("[data-focus-key]")].find(node=>node.dataset.focusKey===saved.focus);(target??root).focus({preventScroll:true});}
  restoreSelection(root,saved.selection);root.scrollTop=saved.scroll;
}
export function retainReading(root,replace){const saved=saveReading(root);replace();restoreReading(root,saved);}
