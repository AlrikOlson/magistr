// Bound the rendered neighborhood without discarding the owner's complete record set.
export function roadmapContext(primary,all,selected){
  const included=new Set(primary);if(selected&&all.has(selected))included.add(selected);
  const queue=[...included];
  for(let i=0;i<queue.length&&included.size<100;i++)for(const dep of (all.get(queue[i])?.deps?.length>6?[]:all.get(queue[i])?.deps??[])){
    if(included.size>=100)break;if(!included.has(dep)){included.add(dep);queue.push(dep);}
  }
  if(selected)for(const record of all.values())if(included.size<100&&record.deps?.includes(selected))included.add(record.id);
  const records=new Map(),bundles=new Map();let remaining=160;
  for(const id of [...included]){
    const source=all.get(id)??{id,title:"Required item unavailable",name:"Unavailable",deps:[]},deps=[],hidden=[];
    for(const dep of source.deps??[]){if(source.deps.length<=6&&included.has(dep)&&remaining>0){deps.push(dep);remaining--;}else hidden.push(dep);}
    if(hidden.length){
      const bundle="prerequisites:"+id;deps.push(bundle);
      const more=hidden.length<(source.deps?.length??0),completed=hidden.filter(dep=>all.get(dep)?.status==="done").length;
      bundles.set(bundle,{target:id,count:hidden.length,ids:hidden,more,completed});
      records.set(bundle,{id:bundle,title:hidden.length+(more?" more prerequisites":" prerequisites"),deps:[]});
    }
    records.set(id,{...source,deps});
  }
  const reachable=new Set(primary.filter(id=>records.has(id)));if(records.has(selected))reachable.add(selected);
  for(const [id,record] of records)if(record.deps?.includes(selected))reachable.add(id);
  const pending=[...reachable];for(let i=0;i<pending.length;i++)for(const dep of records.get(pending[i])?.deps??[])if(!reachable.has(dep)){reachable.add(dep);pending.push(dep);}
  return {records:new Map([...records].filter(([id])=>reachable.has(id))),bundles};
}