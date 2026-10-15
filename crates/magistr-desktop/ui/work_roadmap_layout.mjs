// Align prerequisites with their consumers. Independent work shares the frontier column.
export function roadmapPositions(ids,records,primary,next){
  const depth=new Map(),visiting=new Set(),consumers=new Map(ids.map(id=>[id,[]]));
  for(const id of ids)for(const dep of records.get(id)?.deps??[])consumers.get(dep)?.push(id);
  const level=id=>{if(depth.has(id))return depth.get(id);if(visiting.has(id))return 0;visiting.add(id);const deps=(records.get(id)?.deps??[]).filter(d=>consumers.has(d));const n=deps.length?1+Math.max(...deps.map(level)):0;visiting.delete(id);depth.set(id,n);return n;};
  const last=Math.max(0,...ids.map(level)),columns=new Map(ids.map(id=>[id,consumers.get(id).length?depth.get(id):last])),positions=new Map(),occupied=new Set(),placing=new Set();
  const place=(id,row)=>{
    if(positions.has(id))return positions.get(id).row;if(placing.has(id))return row;placing.add(id);
    const parents=(records.get(id)?.deps??[]).filter(d=>consumers.has(d)),rows=parents.map((dep,i)=>place(dep,row+i));
    let chosen=rows.length?Math.round(rows.reduce((a,b)=>a+b,0)/rows.length):row;
    const col=columns.get(id);while(occupied.has(col+":"+chosen))chosen++;
    occupied.add(col+":"+chosen);positions.set(id,{x:col*344,y:chosen*92+28,row:chosen});placing.delete(id);return chosen;
  };
  const roots=[...new Set([...primary.filter(id=>consumers.has(id)&&!consumers.get(id).length),...ids.filter(id=>!consumers.get(id).length)])];
  roots.sort((a,b)=>a===next?-1:b===next?1:0);
  let row=0;for(const id of roots){place(id,row);row=1+Math.max(...[...positions.values()].map(p=>p.row));}
  for(const id of ids)if(!positions.has(id))place(id,row++);
  return {positions,columns:[...new Set(columns.values())].sort((a,b)=>a-b)};
}