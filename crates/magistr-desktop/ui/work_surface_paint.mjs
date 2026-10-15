// Canvas draws relationships; actual records and controls remain semantic DOM elements.
function trace(ctx,points){
  ctx.beginPath();ctx.moveTo(points[0].x,points[0].y);
  for(let i=1;i<points.length-1;i++){
    const a=points[i-1],b=points[i],c=points[i+1],before=Math.hypot(b.x-a.x,b.y-a.y),after=Math.hypot(c.x-b.x,c.y-b.y),r=Math.min(7,before/2,after/2);
    ctx.lineTo(b.x-(b.x-a.x)/before*r,b.y-(b.y-a.y)/before*r);
    ctx.quadraticCurveTo(b.x,b.y,b.x+(c.x-b.x)/after*r,b.y+(c.y-b.y)/after*r);
  }
  ctx.lineTo(points.at(-1).x,points.at(-1).y);
}
export function paintSurface(surface){
  const {canvas,viewport,camera,edges,groups,selected}=surface,w=viewport.clientWidth,h=viewport.clientHeight;
  if(!w||!h)return;const ratio=window.devicePixelRatio||1;
  if(canvas.width!==Math.round(w*ratio)||canvas.height!==Math.round(h*ratio)){canvas.width=Math.round(w*ratio);canvas.height=Math.round(h*ratio);}
  const ctx=canvas.getContext("2d"),style=getComputedStyle(viewport),color=name=>style.getPropertyValue(name).trim();
  ctx.setTransform(ratio,0,0,ratio,0,0);ctx.clearRect(0,0,w,h);ctx.fillStyle=color("--surface-grid");
  let gap=24*camera.scale;while(gap<16)gap*=2;
  for(let x=camera.x%gap;x<w;x+=gap)for(let y=camera.y%gap;y<h;y+=gap){ctx.beginPath();ctx.arc(x,y,.55,0,Math.PI*2);ctx.fill();}
  ctx.translate(camera.x,camera.y);ctx.scale(camera.scale,camera.scale);
  for(const group of groups??[]){
    if(group.element)continue;
    ctx.fillStyle=color("--muted");ctx.font="600 10px "+style.getPropertyValue("--font-ui");ctx.fillText(group.label,group.x,group.y+11);
  }
  const ordered=[...edges].sort((a,b)=>Number(a.from===selected||a.to===selected)-Number(b.from===selected||b.to===selected));
  for(const edge of ordered){
    const p=edge.route;if(!p)continue;const related=!selected||edge.from===selected||edge.to===selected;
    ctx.globalAlpha=related?1:.3;ctx.setLineDash([]);ctx.lineWidth=5/camera.scale;ctx.strokeStyle=color("--base");trace(ctx,p.points);ctx.stroke();
    ctx.strokeStyle=color(selected&&related?"--accent":"--surface-wire");ctx.lineWidth=(selected&&related?1.8:1.3)/camera.scale;
    ctx.setLineDash(edge.missing||edge.kind==="revision"?[3,4]:[]);trace(ctx,p.points);ctx.stroke();ctx.setLineDash([]);
    ctx.fillStyle=ctx.strokeStyle;ctx.beginPath();ctx.arc(p.from.x,p.from.y,2.5/camera.scale,0,Math.PI*2);ctx.fill();
    if(edge.kind==="membership")continue;
    const previous=p.points.at(-2),distance=Math.hypot(p.to.x-previous.x,p.to.y-previous.y),dx=(p.to.x-previous.x)/distance,dy=(p.to.y-previous.y)/distance;
    ctx.beginPath();ctx.moveTo(p.to.x-6*dx+3.5*dy,p.to.y-6*dy-3.5*dx);ctx.lineTo(p.to.x,p.to.y);ctx.lineTo(p.to.x-6*dx-3.5*dy,p.to.y-6*dy+3.5*dx);ctx.stroke();
  }
  ctx.globalAlpha=1;
}