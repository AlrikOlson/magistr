import test from "node:test";
import assert from "node:assert/strict";
import { workPresentation, WorkPanel } from "../ui/work.mjs";
const address={project:"project",workspace:"copy",session:"one"};
const frame=()=>({address,target:{address,conversation:"opening"},sequence:1,closed:false,activity:{active:false}});
const run=(phase,requested=null)=>({id:{project:"project",local:"run-one"},phase:{kind:phase},iteration:1,limit:10,requested});
test("a pending click never claims work started; a refusal remains distinct",()=>{
  const f=frame();
  assert.equal(workPresentation(f,true).status,"Requesting work…");
  assert.equal(workPresentation(f,true).active,false);
  f.work_admission={sequence:2,error:"Already running.",run_id:null};
  assert.equal(workPresentation(f).error,"Already running.");
  assert.equal(workPresentation(f).active,false);
});
test("pause and stop requests describe future boundaries; paused work can step",()=>{
  const f=frame();f.work=run("running","pause");
  assert.match(workPresentation(f).status,/then pausing/);
  assert.equal(workPresentation(f).paused,false);
  f.work=run("paused");assert.equal(workPresentation(f).paused,true);
  f.work=run("running","abort");assert.equal(workPresentation(f).stopped,true);
  f.work={...run("ended"),phase:{kind:"ended",reason:"Finished."}};
  assert.equal(workPresentation(f).detail,"Finished.");
  assert.equal(workPresentation(f).canStart,true);
});
test("delayed controls preserve the captured run across conversation and resident changes",async()=>{
  const elements=new Map(),calls=[];
  const $=id=>{if(!elements.has(id))elements.set(id,{});return elements.get(id);};
  let finish;const delivered=new Promise(resolve=>finish=resolve);
  const panel=new WorkPanel($,async(name,args)=>{calls.push({name,args});await delivered;},()=>{});
  const f=frame();f.work=run("running");panel.render(f);
  const pending=panel.control("pause");
  panel.render({...f,target:{...f.target,conversation:"new provider"},work:{...run("running"),id:{project:"project",local:"run-two"}}});
  finish();await pending;
  assert.equal(calls[0].args.target.run_id.local,"run-one");
  assert.deepEqual(calls[0].args.target.address,address);
});
test("launch admission clears pending state only after the owner's newer result",async()=>{
  const elements=new Map();const $=id=>{if(!elements.has(id))elements.set(id,{});return elements.get(id);};
  const panel=new WorkPanel($,async()=>{},()=>{}),f=frame();panel.render(f);
  await panel.start("next");
  assert.equal($("work-status").textContent,"Requesting work…");
  panel.render({...f,sequence:9});
  assert.equal($("work-next").disabled,true);
  panel.render({...f,sequence:10,work_admission:{sequence:10,error:"Cannot start.",run_id:null}});
  assert.equal($("work-next").disabled,false);
  assert.equal($("work-error").textContent,"Cannot start.");
});
