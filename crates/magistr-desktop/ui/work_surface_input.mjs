// A drag only changes the view. Cancellation restores the camera or piece's original position.
export function bindSurfaceInput(surface){
  const view=surface.viewport;let drag=null,suppress=false;
  view.addEventListener("pointerdown",event=>{
    if(event.button!==0)return;
    const grip=event.target.closest("[data-move-piece]");if(!grip&&event.target.closest("button,input,select"))return;
    const node=grip?surface.nodes.get(grip.dataset.movePiece):null;
    drag={id:event.pointerId,x:event.clientX,y:event.clientY,node,start:node?{x:node.x,y:node.y}:{...surface.camera},moved:false};
    view.focus({preventScroll:true});view.setPointerCapture(event.pointerId);view.dataset.dragging="true";event.preventDefault();
  });
  view.addEventListener("pointermove",event=>{
    if(!drag||event.pointerId!==drag.id)return;const dx=event.clientX-drag.x,dy=event.clientY-drag.y;
    if(Math.hypot(dx,dy)>3)drag.moved=true;if(!drag.moved)return;surface.hasNavigated=true;
    if(drag.node){drag.node.x=drag.start.x+dx/surface.camera.scale;drag.node.y=drag.start.y+dy/surface.camera.scale;}
    else{surface.camera.x=drag.start.x+dx;surface.camera.y=drag.start.y+dy;}
    surface.render();
  });
  const finish=(event,cancel)=>{
    if(!drag||event.pointerId!==drag.id)return;
    if(cancel){if(drag.node)Object.assign(drag.node,drag.start);else Object.assign(surface.camera,drag.start);}
    else if(drag.node&&drag.moved)surface.positions.set(drag.node.id,{x:drag.node.x,y:drag.node.y});
    if(!cancel&&drag.node&&!drag.moved)surface.moveMenu(drag.node);
    suppress=!cancel&&drag.moved;drag=null;delete view.dataset.dragging;surface.render();
  };
  view.addEventListener("pointerup",event=>finish(event,false));view.addEventListener("pointercancel",event=>finish(event,true));view.addEventListener("lostpointercapture",event=>finish(event,true));
  view.addEventListener("click",event=>{if(suppress){suppress=false;event.preventDefault();event.stopPropagation();}},true);
  view.addEventListener("wheel",event=>{
    event.preventDefault();const bounds=view.getBoundingClientRect();
    if(event.ctrlKey||event.metaKey)surface.zoom(Math.exp(-event.deltaY*.008),event.clientX-bounds.left,event.clientY-bounds.top);
    else{surface.hasNavigated=true;surface.camera.x-=event.deltaX;surface.camera.y-=event.deltaY;surface.render();}
  },{passive:false});
  view.addEventListener("keydown",event=>{
    if(event.isComposing||event.altKey||event.metaKey||event.ctrlKey)return;
    if(event.key==="Escape"&&drag){finish({pointerId:drag.id},true);event.preventDefault();event.stopPropagation();return;}
    if(event.target!==view)return;
    const direction={ArrowLeft:[80,0],ArrowRight:[-80,0],ArrowUp:[0,80],ArrowDown:[0,-80]}[event.key];
    if(direction){surface.pan(...direction);event.preventDefault();}
    else if(["+","=","-","0"].includes(event.key)){event.preventDefault();event.key==="0"?surface.fit():surface.zoom(event.key==="-"?1/1.2:1.2);}
  });
}