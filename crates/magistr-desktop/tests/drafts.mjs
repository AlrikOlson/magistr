import test from "node:test";
import assert from "node:assert/strict";
import { Drafts } from "../ui/drafts.mjs";
const address = {project:"one",workspace:"/one",session:"first"};
const status = (revision, save_attempt=revision, saved_revision=revision, error=null) => ({revision,saved_revision,save_attempt,error});
const frame = (target, revision, save_attempt=revision, saved_revision=revision, error=null) => ({
  address:target,draft_revision:revision,draft_saved_revision:saved_revision,draft_save_attempt:save_attempt,draft_save_error:error,
});
const deferred = () => {let resolve;const promise=new Promise(done=>{resolve=done;});return {promise,resolve};};

test("rapid edits save the first and newest revisions in order", async () => {
  const gate=deferred(), calls=[];
  const drafts=new Drafts(async(target,revision,text)=>{
    calls.push({target,revision,text});
    if(calls.length===1)await gate.promise;
    return status(revision+1);
  });
  drafts.observe(frame(address,4));
  drafts.edit(address,"first");drafts.edit(address,"middle");drafts.edit(address,"last 文");
  assert.equal(calls.length,1);gate.resolve();await drafts.flush(address);
  assert.deepEqual(calls.map(({revision,text})=>({revision,text})),[{revision:4,text:"first"},{revision:5,text:"last 文"}]);
  assert.equal(drafts.status(address).saving,false);
});

test("saves remain addressed while another working copy is edited", async () => {
  const gate=deferred(), calls=[], other={...address,workspace:"/two"};
  const drafts=new Drafts(async(target,revision,text)=>{calls.push({target,revision,text});if(target===address)await gate.promise;return status(revision+1);});
  drafts.observe(frame(address,8));drafts.observe(frame(other,2));
  drafts.edit(address,"one");drafts.edit(other,"two");await drafts.flush(other);
  gate.resolve();await drafts.flushAll();
  assert.deepEqual(calls.map(({target,revision})=>[target.workspace,revision]),[["/one",8],["/two",2]]);
});

test("failed save keeps the latest editable text for explicit retry", async () => {
  const gate=deferred(), calls=[];let failed=true;
  const drafts=new Drafts(async(target,revision,text)=>{
    calls.push({revision,text});if(failed){await gate.promise;return {...status(revision+1),saved_revision:revision,error:"Disk is full."};}
    return status(revision+1);
  });
  drafts.observe(frame(address,0));drafts.edit(address,"before");drafts.edit(address,"after");
  gate.resolve();await assert.rejects(drafts.flush(address),/Disk is full/);
  assert.equal(drafts.status(address).error,"Disk is full.");
  failed=false;await drafts.flush(address);
  assert.deepEqual(calls.map(call=>call.text),["before","after"]);
  assert.equal(drafts.status(address).saved_revision,2);
});

test("a delayed result cannot lower a newer owner revision", async () => {
  const gate=deferred(), calls=[];
  const drafts=new Drafts(async(target,revision,text)=>{calls.push(revision);if(calls.length===1)await gate.promise;return status(revision+1);});
  drafts.observe(frame(address,3));drafts.edit(address,"earlier");
  drafts.observe(frame(address,9));drafts.edit(address,"newer");
  gate.resolve();await drafts.flush(address);
  assert.deepEqual(calls,[3,9]);assert.equal(drafts.status(address).revision,10);
});

test("a refused stale save stays pending until an explicit retry", async () => {
  const calls=[];let refuse=true;
  const drafts=new Drafts(async(target,revision,text)=>{calls.push({revision,text});if(refuse)throw new Error("The draft changed. Try again.");return status(revision+1);});
  drafts.observe(frame(address,1));drafts.edit(address,"keep this");
  await assert.rejects(drafts.flush(address),/draft changed/);
  assert.equal(calls.length,1);
  drafts.observe(frame(address,4));refuse=false;await drafts.flush(address);
  assert.deepEqual(calls,[{revision:1,text:"keep this"},{revision:4,text:"keep this"}]);
});

test("an older failed frame cannot undo a same-revision retry RPC", async () => {
  const drafts=new Drafts(async()=>status(7,12));
  drafts.observe(frame(address,7,10,6,"Disk is full."));
  drafts.edit(address,"Retry the same text");
  await drafts.flush(address);
  const saved=drafts.status(address);
  drafts.observe(frame(address,7,11,6,"Disk is full."));
  assert.deepEqual(drafts.status(address),saved);
  await drafts.flush(address);
});

test("save attempts order errors when neither editable nor saved revision changes", async () => {
  const drafts=new Drafts(async()=>assert.fail("No edit was requested"));
  drafts.observe(frame(address,7,10));
  drafts.accept(address,status(7,12));
  drafts.observe(frame(address,7,11,7,"A repeated save failed."));
  assert.equal(drafts.status(address).error,null);
  drafts.observe(frame(address,7,13,7,"The latest attempt failed."));
  drafts.accept(address,status(7,12));
  drafts.observe(frame(address,7,12));
  assert.equal(drafts.status(address).save_attempt,13);
  assert.equal(drafts.status(address).error,"The latest attempt failed.");
  await assert.rejects(drafts.flush(address),/latest attempt failed/);
});

test("a delayed failed RPC cannot requeue text after a newer successful frame", async () => {
  const gate=deferred();let calls=0;
  const drafts=new Drafts(async()=>{calls++;await gate.promise;return status(7,10,6,"Old failure");});
  drafts.observe(frame(address,7,9,6));
  drafts.edit(address,"Attempt still awaiting its RPC");
  drafts.observe(frame(address,7,11));
  gate.resolve();
  await drafts.flush(address);
  assert.equal(calls,1);
  assert.deepEqual(drafts.status(address),{...status(7,11),saving:false});
});

test("a rejected RPC error cannot resurrect input superseded by an owner edit", async () => {
  const gate=deferred();let calls=0;
  const drafts=new Drafts(async()=>{calls++;await gate.promise;return status(5,8,4,"Old failure");});
  drafts.observe(frame(address,4,7));
  drafts.edit(address,"Already accepted input");
  drafts.observe(frame(address,6,9));
  gate.resolve();
  await drafts.flush(address);
  assert.equal(calls,1);
  assert.deepEqual(drafts.status(address),{...status(6,9),saving:false});
});

test("a newer editable frame keeps the newest completed save facts", () => {
  const drafts=new Drafts(async()=>assert.fail("No edit was requested"));
  drafts.accept(address,status(7,12));
  drafts.observe(frame(address,8,11,6,"Old failure"));
  assert.deepEqual(drafts.status(address),{...status(8,12,7),saving:true});
  drafts.observe(frame(address,8,13));
  assert.equal(drafts.status(address).saving,false);
});

test("a save error already observed on the Channel still keeps the RPC edit for retry", async () => {
  const gate=deferred();let calls=0;
  const drafts=new Drafts(async()=>{
    calls++;
    if(calls===1){await gate.promise;return status(7,10,6,"Current failure");}
    return status(7,11);
  });
  drafts.observe(frame(address,7,9,6));
  drafts.edit(address,"Keep this failed edit");
  drafts.observe(frame(address,7,10,6,"Current failure"));
  gate.resolve();
  await assert.rejects(drafts.flush(address),/Current failure/);
  await drafts.flush(address);
  assert.equal(calls,2);
  assert.deepEqual(drafts.status(address),{...status(7,11),saving:false});
});

