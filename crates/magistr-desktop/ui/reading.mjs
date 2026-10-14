import { targetKey } from "./context.mjs";

// Bounded, transient view coordinates; no message or draft bytes are retained.
export function remember(map,key,value) {
  map.delete(key);map.set(key,value);
  while(map.size>64)map.delete(map.keys().next().value);
}
export function textStamp(value) {
  let first=2166136261,second=5381;
  for(let index=0;index<value.length;index++){const unit=value.charCodeAt(index);first=Math.imul(first^unit,16777619);second=Math.imul(second,33)^unit;}
  return [value.length,first>>>0,second>>>0].join(":");
}
function endpoint(root,node,offset) {
  const block=(node?.nodeType===3?node.parentElement:node)?.closest("[data-reading]");
  if(!block||!root.contains(block))return null;
  const range=root.ownerDocument.createRange();range.selectNodeContents(block);range.setEnd(node,offset);
  return {key:block.dataset.reading,offset:range.toString().length};
}
export function captureSelection(root) {
  const selection=root.ownerDocument.getSelection();
  if(!selection||selection.isCollapsed)return null;
  const anchor=endpoint(root,selection.anchorNode,selection.anchorOffset),focus=endpoint(root,selection.focusNode,selection.focusOffset);
  return anchor&&focus?{anchor,focus,stamp:textStamp(selection.toString())}:null;
}
function point(root,saved) {
  const block=[...root.querySelectorAll("[data-reading]")].find(node=>node.dataset.reading===saved.key);
  if(!block)return null;
  const walker=root.ownerDocument.createTreeWalker(block,4);
  let remaining=saved.offset,node;
  while((node=walker.nextNode())){if(remaining<=node.length)return {node,offset:remaining};remaining-=node.length;}
  return remaining===0?{node:block,offset:block.childNodes.length}:null;
}
export function restoreSelection(root,saved) {
  if(!saved)return false;
  const anchor=point(root,saved.anchor),focus=point(root,saved.focus);
  if(!anchor||!focus)return false;
  const selection=root.ownerDocument.getSelection();
  selection.setBaseAndExtent(anchor.node,anchor.offset,focus.node,focus.offset);
  if(textStamp(selection.toString())===saved.stamp)return true;
  selection.removeAllRanges();return false;
}
export class EditorPlaces {
  constructor(editor){this.editor=editor;this.places=new Map();}
  capture(target,revision) {
    if(!target||(this.targets&&!this.targets.has(targetKey(target))))return;
    const editor=this.editor;
    remember(this.places,targetKey(target),{revision,stamp:textStamp(editor.value),start:editor.selectionStart,end:editor.selectionEnd,direction:editor.selectionDirection,top:editor.scrollTop});
  }
  restore(target,revision) {
    const saved=this.places.get(targetKey(target)),editor=this.editor;
    if(!saved||saved.revision!==revision||saved.stamp!==textStamp(editor.value))return;
    editor.setSelectionRange(saved.start,saved.end,saved.direction);editor.scrollTop=saved.top;
  }
  retain(targets){this.targets=targets;for(const key of this.places.keys())if(!targets.has(key))this.places.delete(key);}
}
