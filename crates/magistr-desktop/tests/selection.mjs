import test from "node:test";
import assert from "node:assert/strict";
import { Selection } from "../ui/selection.mjs";
const deferred=()=>{let resolve;const promise=new Promise(r=>resolve=r);return {promise,resolve};};
const address=session=>({project:"p",workspace:"w",session});
test("a newer navigation wins while an older draft save is waiting",async()=>{
 const selection=new Selection(),saved=deferred(),opens=[];
 const old=selection.open(address("old"),{save:()=>saved.promise,subscribe:async(a,n)=>{opens.push(a.session);return n;},commit:()=>assert.fail("stale selection committed")});
 await selection.open(address("new"),{save:async()=>{},subscribe:async(a,n)=>{opens.push(a.session);return n;},commit:()=>{}});
 saved.resolve();assert.equal(await old,false);assert.deepEqual(opens,["new"]);
 assert.equal(selection.current.address.session,"new");
});
test("late subscription replies and frames cannot replace the current session",async()=>{
 const selection=new Selection(),subscribed=deferred(),frames=[],commits=[];
 const callbacks={};
 const subscribe=(a,n,receive)=>{callbacks[a.session]=receive;return a.session==="old"?subscribed.promise:Promise.resolve(n);};
 const options={save:async()=>{},subscribe,commit:(view,frame)=>{commits.push(view.address.session);if(frame)frames.push(frame.address.session);},receive:(view,frame)=>frames.push(frame.address.session)};
 const old=selection.open(address("old"),options);await Promise.resolve();
 await selection.open(address("new"),options);
 callbacks.old({address:address("old")});callbacks.new({address:address("new")});
 subscribed.resolve(1);assert.equal(await old,false);
 assert.deepEqual(commits,["new"]);assert.deepEqual(frames,["new"]);
});
test("an initial frame arriving before subscribe returns is applied after admission",async()=>{
 const selection=new Selection(),events=[];
 await selection.open(address("a"),{save:async()=>{},subscribe:async(a,n,receive)=>{receive({address:a});return n;},commit:(view,frame)=>events.push(["commit",view.address.session,frame.address.session]),receive:()=>assert.fail("frame applied before admission")});
 assert.deepEqual(events,[["commit","a","a"]]);
});
