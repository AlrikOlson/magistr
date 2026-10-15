// Small DOM helpers for complete workflow records.
import { captureSelection,restoreSelection } from "./reading.mjs";
export function element(tag,value="",className=""){
  const node=document.createElement(tag);node.textContent=value;
  if(className)node.className=className;return node;
}
export function retainReading(root,replace){
  const selection=captureSelection(root);
  const open=[...root.querySelectorAll("details[open][data-reading-key]")].map(node=>node.dataset.readingKey);
  const focus=root.contains(document.activeElement)?document.activeElement.dataset.focusKey:null,scroll=root.scrollTop;
  replace();
  for(const details of root.querySelectorAll("details[data-reading-key]"))details.open=open.includes(details.dataset.readingKey);
  if(focus){const target=[...root.querySelectorAll("[data-focus-key]")].find(node=>node.dataset.focusKey===focus);(target??root).focus({preventScroll:true});}
  restoreSelection(root,selection);root.scrollTop=scroll;
}
