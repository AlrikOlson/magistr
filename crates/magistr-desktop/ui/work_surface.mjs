import {element} from "./work_dom.mjs";
import {paintSurface} from "./work_surface_paint.mjs";
import {routeSurface} from "./work_surface_routes.mjs";
import {bindSurfaceInput} from "./work_surface_input.mjs";
const action=(label,fn,text=label)=>{const button=element("button",text);button.type="button";button.setAttribute("aria-label",label);button.title=label;button.onclick=fn;return button;};
// The camera and manual placement are desktop view state, never roadmap order or workflow state.
export class WorkSurface{
  constructor(label){
    this.root=element("section","","work-surface");this.root.setAttribute("aria-label",label);
    this.viewport=element("div","","surface-viewport");this.viewport.tabIndex=0;this.viewport.setAttribute("role","region");this.viewport.setAttribute("aria-label",label+" canvas");
    this.canvas=element("canvas");this.canvas.setAttribute("aria-hidden","true");this.world=element("div","","surface-world");
    this.viewport.append(this.canvas,this.world);this.root.append(this.viewport);this.nodes=new Map();this.positions=new Map();this.camera={x:32,y:48,scale:1};this.edges=[];this.groups=[];this.selected=null;
    this.tools=element("div","","surface-tools");this.tools.setAttribute("role","toolbar");this.tools.setAttribute("aria-label",label+" view controls");
    this.zoomLabel=action("Actual size",()=>{this.fit(true);this.zoom(1/this.camera.scale);const anchor=this.nodes.get(this.selected??this.focusId);if(anchor)this.reveal(anchor);},"100%");
    const zoom=element("div","","surface-zoom");zoom.setAttribute("role","group");zoom.setAttribute("aria-label","Zoom");
    zoom.append(action("Zoom out",()=>this.zoom(1/1.2),"−"),this.zoomLabel,action("Zoom in",()=>this.zoom(1.2),"+"));
    this.tools.append(zoom,action("Fit all",()=>this.fit()),action("Auto arrange",()=>{this.positions.clear();for(const node of this.nodes.values())Object.assign(node,node.home);this.fit(true);}));
    const pan=element("details","","surface-pan"),summary=element("summary","Move view"),directions=element("div","","surface-pan-actions");pan.append(summary,directions);
    for(const [label,x,y,mark] of [["left",80,0,"←"],["up",0,80,"↑"],["down",0,-80,"↓"],["right",-80,0,"→"]])directions.append(action("Pan "+label,()=>this.pan(x,y),mark));
    this.tools.append(pan);this.root.append(this.tools);
    this.note=element("p","Drag the background to move · pinch to zoom","","");this.note.className="surface-help";this.root.append(this.note);
    this.observer=new ResizeObserver(()=>{if(!this.viewport.clientWidth||!this.viewport.clientHeight)return;if(this.needsFit||!this.hasNavigated){this.needsFit=false;this.fit(true);}else if(this.nodes.has(this.selected))this.reveal(this.nodes.get(this.selected));else this.render();});this.observer.observe(this.viewport);
    this.theme=new MutationObserver(()=>this.render());this.theme.observe(document.documentElement,{attributes:true,attributeFilter:["data-theme"]});
    bindSurfaceInput(this);
  }
  reset(){this.hasNavigated=false;this.key=null;this.positions.clear();this.selected=null;this.needsFit=true;}
  update({key,nodes,edges,groups=[],selected,focusId,onSelect}){
    if(key!==this.key){this.reset();this.key=key;}
    this.selected=selected;this.focusId=focusId;this.routeKey=null;this.edges=edges;this.groups=groups;this.onSelect=onSelect;this.nodes=new Map(nodes.map(node=>[node.id,{...node,home:{x:node.x,y:node.y},...this.positions.get(node.id)}]));
    for(const id of this.positions.keys())if(!this.nodes.has(id))this.positions.delete(id);
    const elements=[];
    for(const node of this.nodes.values()){
      const wrap=element("div","","surface-node");wrap.dataset.node=node.id;wrap.style.width=node.w+"px";wrap.style.height=node.h+"px";
      const grip=action("Move "+node.label,()=>this.moveMenu(node),"⠿");grip.className="surface-grip";grip.dataset.movePiece=node.id;
      const button=node.element;button.dataset.focusKey="surface:"+node.id;button.setAttribute("aria-pressed",String(node.id===selected));button.onclick=()=>onSelect(node.id);button.onfocus=()=>this.reveal(node);
      button.onkeydown=event=>{if(!event.shiftKey&&["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(event.key)){event.preventDefault();this.focusNeighbor(node,event.key);}};
      wrap.append(button,grip);node.wrap=wrap;elements.push(wrap);
    }
    for(const edge of edges){const control=action(edge.label,()=>{this.selected=edge.to;onSelect(edge.to);},edge.caption??"");control.className="surface-connection";control.dataset.edge=edge.from+":"+edge.to;control.dataset.kind=edge.kind??"dependency";edge.element=control;elements.push(control);}
    for(const group of groups)if(group.element){group.element.style.left=group.x+"px";group.element.style.top=group.y+"px";group.element.style.width=group.w+"px";elements.push(group.element);}
    this.world.replaceChildren(...elements);this.render();queueMicrotask(()=>{if(this.needsFit&&this.root.isConnected){this.needsFit=false;this.fit(true);}else if(this.root.isConnected&&this.focusId&&!this.selected&&!this.hasNavigated){const anchor=this.nodes.get(this.focusId);if(anchor)this.reveal(anchor);}});return this.root;
  }
  moveMenu(node){
    this.root.querySelector(".surface-arrange")?.remove();const menu=element("div","","surface-arrange");menu.setAttribute("role","group");menu.setAttribute("aria-label","Move "+node.label);menu.append(element("span","Move piece"));
    for(const [label,x,y,mark] of [["left",-48,0,"←"],["up",0,-48,"↑"],["down",0,48,"↓"],["right",48,0,"→"]])menu.append(action("Move piece "+label,()=>{node.x+=x;node.y+=y;this.positions.set(node.id,{x:node.x,y:node.y});this.render();},mark));
    menu.append(action("Done arranging",()=>{menu.remove();node.wrap.querySelector("button").focus();},"Done"));this.root.append(menu);menu.querySelector("button").focus();
  }
  pan(x,y){this.hasNavigated=true;this.camera.x+=x;this.camera.y+=y;this.render();}
  zoom(factor,x=this.viewport.clientWidth/2,y=this.viewport.clientHeight/2){
    this.hasNavigated=true;const old=this.camera.scale,next=Math.min(1.8,Math.max(.15,old*factor));this.camera.x=x-(x-this.camera.x)*next/old;this.camera.y=y-(y-this.camera.y)*next/old;this.camera.scale=next;this.render();
  }
  fit(readable=false){
    if(!readable)this.hasNavigated=true;
    const nodes=[...this.nodes.values(),...this.groups.map(g=>({...g,h:g.headingHeight??g.h??16}))];if(!nodes.length)return;
    const minX=Math.min(...nodes.map(n=>n.x)),minY=Math.min(...nodes.map(n=>n.y)),maxX=Math.max(...nodes.map(n=>n.x+n.w)),maxY=Math.max(...nodes.map(n=>n.y+n.h));
    const width=this.viewport.clientWidth,height=this.viewport.clientHeight;if(!width||!height){this.needsFit=true;return;}
    const ideal=Math.min(1,(width-40)/(maxX-minX+24),(height-24)/(maxY-minY+24)),scale=readable&&ideal<.9?1:Math.max(.15,ideal);
    this.camera={scale,x:Math.max(20,(width-(maxX-minX)*scale)/2)-minX*scale,y:Math.max(12,(height-(maxY-minY)*scale)/2)-minY*scale};
    const anchor=this.nodes.get(this.selected??this.focusId);
    if(readable&&scale>ideal&&anchor)this.reveal(anchor);else this.render();
  }
  reveal(node){
    const {x,y,scale}=this.camera,left=x+node.x*scale,top=y+node.y*scale,right=left+node.w*scale,bottom=top+node.h*scale;
    if(left<16)this.camera.x+=16-left;else if(right>this.viewport.clientWidth-16)this.camera.x-=right-this.viewport.clientWidth+16;
    if(top<16)this.camera.y+=16-top;else if(bottom>this.viewport.clientHeight-16)this.camera.y-=bottom-this.viewport.clientHeight+16;this.render();
  }
  focusNeighbor(node,key){
    const horizontal=key==="ArrowLeft"||key==="ArrowRight",sign=key==="ArrowLeft"||key==="ArrowUp"?-1:1;
    const next=[...this.nodes.values()].filter(n=>n!==node&&((horizontal?n.x-node.x:n.y-node.y)*sign)>1).sort((a,b)=>Math.hypot(a.x-node.x,a.y-node.y)-Math.hypot(b.x-node.x,b.y-node.y))[0];next?.wrap.querySelector("button").focus({preventScroll:true});
  }
  render(){
    const {x,y,scale}=this.camera;this.world.style.transform="translate("+x+"px,"+y+"px) scale("+scale+")";this.zoomLabel.textContent=Math.round(scale*100)+"%";this.root.dataset.zoom=String(scale);this.root.dataset.overview=String(scale<.85);this.root.style.setProperty("--view-scale",String(scale));
    for(const node of this.nodes.values()){node.wrap.style.left=node.x+"px";node.wrap.style.top=node.y+"px";node.wrap.dataset.selected=String(node.id===this.selected);node.element.setAttribute("aria-pressed",String(node.id===this.selected));}
    const routeKey=JSON.stringify([...this.nodes.values()].map(n=>[n.id,n.x,n.y,n.w,n.h]));
    if(routeKey!==this.routeKey){routeSurface(this.nodes,this.edges,this.groups);this.routeKey=routeKey;}
    for(const edge of this.edges){
      const p=edge.route;edge.element.hidden=!p;if(!p)continue;
      edge.element.style.left=p.labelX+"px";edge.element.style.top=(p.labelY-10)+"px";edge.element.dataset.related=String(edge.from===this.selected||edge.to===this.selected);
      edge.element.dataset.route=JSON.stringify(p.points);edge.element.dataset.room=String(p.labelRoom>72);
    }
    this.note.textContent=scale<.85?"Overview · select an item to read it":"Drag the background to move · pinch to zoom";
    paintSurface(this);
  }
}