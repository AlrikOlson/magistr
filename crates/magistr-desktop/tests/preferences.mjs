import test from "node:test";
import assert from "node:assert/strict";
import { Preferences, effectiveMotion } from "../ui/preferences.mjs";
const snapshot=(revision,theme="system",reduce_motion=false)=>({revision,preferences:{theme,reduce_motion}});
const deferred=()=>{let resolve;const promise=new Promise(done=>{resolve=done;});return {promise,resolve};};
test("preferences load and rapid changes persist the newest selection in order",async()=>{
  const gate=deferred(),calls=[];
  const preferences=new Preferences(async(command,args)=>{
    if(command==="preferences")return snapshot(4);
    calls.push(args);if(calls.length===1)await gate.promise;
    return {revision:args.expectedRevision+1,preferences:args.preferences};
  });
  await preferences.load();preferences.change({theme:"dark",reduce_motion:false});
  preferences.change({theme:"light",reduce_motion:true});gate.resolve();await preferences.flush();
  assert.deepEqual(calls.map(call=>call.expectedRevision),[4,5]);
  assert.deepEqual(preferences.value,{theme:"light",reduce_motion:true});
  assert.equal(preferences.snapshot.revision,6);
});
test("save refusal retains editable preferences and retries against refreshed owner state",async()=>{
  let failed=true;const calls=[];
  const preferences=new Preferences(async(command,args)=>{
    if(command==="preferences")return snapshot(failed?2:8);
    calls.push(args);if(failed)throw new Error("Settings changed.");
    return {revision:args.expectedRevision+1,preferences:args.preferences};
  });
  await preferences.load();preferences.change({theme:"dark",reduce_motion:true});
  await assert.rejects(preferences.flush(),/Settings changed/);
  assert.deepEqual(preferences.value,{theme:"dark",reduce_motion:true});
  failed=false;await preferences.retry();await preferences.flush();
  assert.deepEqual(calls.map(call=>call.expectedRevision),[2,8]);
  assert.equal(preferences.error,null);
});
test("an unreadable preference file remains a reported load failure",async()=>{
  const preferences=new Preferences(async()=>{throw new Error("Cannot read settings.");});
  await preferences.load();assert.equal(preferences.value,null);assert.match(preferences.error,/Cannot read/);
});
test("system or launch reduced motion cannot be disabled by a saved false",()=>{
  assert.equal(effectiveMotion(false,false,true),true);
  assert.equal(effectiveMotion(true,false,false),true);
  assert.equal(effectiveMotion(false,true,false),true);
  assert.equal(effectiveMotion(false,false,false),false);
});
