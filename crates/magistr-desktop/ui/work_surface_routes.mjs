// Geometry is view state. Edges always retain their recorded source and destination.
const middle=(a,b)=>({x:(a.x+b.x)/2,y:(a.y+b.y)/2});
const length=(a,b)=>Math.abs(a.x-b.x)+Math.abs(a.y-b.y);
const simplify=points=>points.filter((p,i)=>!i||p.x!==points[i-1].x||p.y!==points[i-1].y).filter((p,i,a)=>!i||i===a.length-1||!((a[i-1].x===p.x&&p.x===a[i+1].x)||(a[i-1].y===p.y&&p.y===a[i+1].y)));
export function crossesBox(a,b,r){
  return a.x===b.x?a.x>r.x&&a.x<r.x+r.w&&Math.max(a.y,b.y)>r.y&&Math.min(a.y,b.y)<r.y+r.h:
    a.y>r.y&&a.y<r.y+r.h&&Math.max(a.x,b.x)>r.x&&Math.min(a.x,b.x)<r.x+r.w;
}
function clear(points,obstacles){return points.slice(1).every((b,i)=>!obstacles.some(r=>crossesBox(points[i],b,r)));}
function interference(points,routes){
  let cost=0;
  for(let i=1;i<points.length;i++)for(const route of routes)for(let j=1;j<route.points.length;j++){
    const a=points[i-1],b=points[i],c=route.points[j-1],d=route.points[j],h=a.y===b.y,v=c.x===d.x;
    if(h&&v&&c.x>Math.min(a.x,b.x)&&c.x<Math.max(a.x,b.x)&&a.y>Math.min(c.y,d.y)&&a.y<Math.max(c.y,d.y))cost+=36;
    if(!h&&!v&&a.x>Math.min(c.x,d.x)&&a.x<Math.max(c.x,d.x)&&c.y>Math.min(a.y,b.y)&&c.y<Math.max(a.y,b.y))cost+=36;
    if(h&&!v&&a.y===c.y)cost+=Math.max(0,Math.min(Math.max(a.x,b.x),Math.max(c.x,d.x))-Math.max(Math.min(a.x,b.x),Math.min(c.x,d.x)))*4;
    if(!h&&v&&a.x===c.x)cost+=Math.max(0,Math.min(Math.max(a.y,b.y),Math.max(c.y,d.y))-Math.max(Math.min(a.y,b.y),Math.min(c.y,d.y)))*4;
  }
  return cost;
}
// Rectilinear visibility search is needed when a manually placed item obstructs all simple corridors.
function detour(start,end,obstacles){
  const xs=[...new Set([start.x,end.x,...obstacles.flatMap(r=>[r.x,r.x+r.w])])].sort((a,b)=>a-b);
  const ys=[...new Set([start.y,end.y,...obstacles.flatMap(r=>[r.y,r.y+r.h])])].sort((a,b)=>a-b);
  const point=(x,y)=>({x:xs[x],y:ys[y]}),key=(x,y,d)=>x+":"+y+":"+d;
  const queue=[],best=new Map(),parents=new Map();
  const push=n=>{queue.push(n);let i=queue.length-1;while(i){const p=(i-1)>>1;if(queue[p].score<=n.score)break;queue[i]=queue[p];i=p;}queue[i]=n;};
  const pop=()=>{const first=queue[0],last=queue.pop();if(queue.length){let i=0;while(i*2+1<queue.length){let c=i*2+1;if(c+1<queue.length&&queue[c+1].score<queue[c].score)c++;if(queue[c].score>=last.score)break;queue[i]=queue[c];i=c;}queue[i]=last;}return first;};
  const sx=xs.indexOf(start.x),sy=ys.indexOf(start.y),initial={x:sx,y:sy,d:0,g:0,score:0,key:key(sx,sy,0)};push(initial);best.set(initial.key,0);
  while(queue.length){
    const n=pop();if(n.g!==best.get(n.key))continue;const a=point(n.x,n.y);
    if(a.x===end.x&&a.y===end.y){const result=[a];let k=n.key;while(parents.has(k)){const parent=parents.get(k);result.push(point(parent.x,parent.y));k=parent.key;}return simplify(result.reverse());}
    for(const [dx,dy,d] of [[1,0,1],[-1,0,1],[0,1,2],[0,-1,2]]){
      const x=n.x+dx,y=n.y+dy;if(x<0||y<0||x>=xs.length||y>=ys.length)continue;
      const b=point(x,y);if(!clear([a,b],obstacles))continue;
      const g=n.g+length(a,b)+(n.d&&n.d!==d?24:0),k=key(x,y,d);if(g>=(best.get(k)??Infinity))continue;
      best.set(k,g);parents.set(k,n);push({x,y,d,g,key:k,score:g+length(b,end)});
    }
  }
  return null;
}
export function routeSurface(nodes,edges,groups=[]){
  const obstacles=[...nodes.values()].map(n=>({x:n.x-8,y:n.y-8,w:n.w+16,h:n.h+16}));
  for(const g of groups)if(g.element)obstacles.push({x:g.x-8,y:g.y-4,w:g.w+16,h:(g.headingHeight??64)+8});
  const faces=new Map(),routes=[],ports=new Map();
  const face=(id,side,edge,end,other)=>{const k=id+":"+side;if(!faces.has(k))faces.set(k,[]);faces.get(k).push({edge,end,other,side});};
  edges.forEach((edge,i)=>{
    const a=nodes.get(edge.from),b=nodes.get(edge.to);if(!a||!b)return;
    const vertical=a.x<b.x+b.w&&b.x<a.x+a.w&&(a.y+a.h<=b.y||b.y+b.h<=a.y);
    const side=vertical?(b.y>a.y?"bottom":"top"):(b.x+b.w/2>a.x+a.w/2?"right":"left"),target={right:"left",left:"right",top:"bottom",bottom:"top"}[side];
    face(edge.from,side,i,"from",b);face(edge.to,target,i,"to",a);
  });
  for(const [k,list] of faces){
    const id=k.slice(0,k.lastIndexOf(":")),node=nodes.get(id);
    const vertical=["top","bottom"].includes(list[0].side);
    list.sort((a,b)=>(vertical?a.other.x-b.other.x:a.other.y-b.other.y)||a.edge-b.edge);
    list.forEach((entry,i)=>{const t=list.length===1?.5:.28+.44*i/(list.length-1),dx=vertical?0:entry.side==="right"?1:-1,dy=vertical?(entry.side==="bottom"?1:-1):0;
      ports.set(entry.edge+":"+entry.end,{x:node.x+node.w*(vertical?t:dx===1?1:0),y:node.y+node.h*(vertical?(dy===1?1:0):t),dx,dy});});
  }
  edges.forEach((edge,i)=>{
    const from=ports.get(i+":from"),to=ports.get(i+":to");if(!from||!to)return;
    const gap=(to.x-from.x)*from.dx+(to.y-from.y)*from.dy,stub=gap>0?Math.min(20,gap/2):20;
    const start={x:from.x+from.dx*stub,y:from.y+from.dy*stub},end={x:to.x+to.dx*stub,y:to.y+to.dy*stub};
    const center=middle(start,end),candidates=[];
    for(const x of new Set([center.x,start.x,end.x,...obstacles.flatMap(r=>[r.x,r.x+r.w])]))candidates.push(simplify([start,{x,y:start.y},{x,y:end.y},end]));
    for(const y of new Set([center.y,...obstacles.flatMap(r=>[r.y,r.y+r.h])]))candidates.push(simplify([start,{x:start.x,y},{x:end.x,y},end]));
    const available=candidates.filter(p=>clear(p,obstacles));
    const score=p=>p.slice(1).reduce((n,b,j)=>n+length(p[j],b),0)+p.length*24+interference(p,routes);
    const ranked=available.map(points=>({points,score:score(points)})).sort((a,b)=>a.score-b.score);
    const path=ranked[0]?.points??detour(start,end,obstacles);if(!path){edge.route=null;return;}
    const points=simplify([from,...path,to]),segments=points.slice(1).map((b,j)=>({a:points[j],b,length:length(points[j],b)}));
    const horizontal=segments.filter(s=>s.a.y===s.b.y).sort((a,b)=>b.length-a.length)[0],label=middle(horizontal?.a??start,horizontal?.b??end);
    const route={points,from,to,labelX:label.x+(horizontal?0:42),labelY:label.y+(horizontal?0:10),labelRoom:horizontal?.length??0};edge.route=route;routes.push(route);
  });
  return routes;
}