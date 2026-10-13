import test from "node:test";
import assert from "node:assert/strict";
import { ProjectOpening } from "../ui/projects.mjs";

const preview={kind:"setup",review:4,directory:"/new",setup_directory:"/new",files:[{path:"config",selectable:true}]};
function harness(respond) {
  const calls=[],opened=[],states=[];
  const picker=new ProjectOpening(async(name,args)=>{calls.push([name,args]);return respond(name,args);},{
    open:async directory=>{calls.push(["open_session",{directory}]);return {workspace:directory,session:"one"};},
    opened:async address=>{opened.push(address);},
    changed:state=>states.push(state)
  });
  return {picker,calls,opened,states};
}
test("a clean chooser invokes no provider and only a reviewed plan is applied",async()=>{
  const h=harness(name=>name==="preview_project"?preview:{complete:true,directory:"/new",files:[]});
  assert.deepEqual(h.calls,[]);
  await h.picker.choose("/new");
  assert.equal(h.picker.state.stage,"review");assert.deepEqual(h.opened,[]);
  await h.picker.apply([0]);
  assert.deepEqual(h.calls.find(([name])=>name==="apply_setup"),["apply_setup",{review:4,approved:[0]}]);
  assert.equal(h.opened.length,1);
});
test("declined, partial and refused setup never opens a session",async()=>{
  const h=harness(name=>name==="preview_project"?preview:{complete:false,directory:"/new",files:[{outcome:"failed"}]});
  await h.picker.choose("/new");await h.picker.apply([]);
  assert.equal(h.picker.state.stage,"partial");assert.equal(h.opened.length,0);
  await h.picker.cancel();
  assert.equal(h.picker.state.stage,"declined");assert.equal(h.opened.length,0);
});
test("cancellation revokes pending review and late replies cannot replace a later folder",async()=>{
  let deliver;
  const pending=new Promise(resolve=>{deliver=resolve;});
  const h=harness((name,args)=>name==="preview_project"?(args.directory==="/old"?pending:{...preview,review:5,directory:"/new"}):null);
  const old=h.picker.choose("/old");
  await Promise.resolve();
  await h.picker.choose("/new");deliver({...preview,review:4,directory:"/old"});await old;
  assert.equal(h.picker.state.preview.review,5);
  assert.ok(h.calls.some(([name,args])=>name==="discard_setup"&&args.review===4));
  await h.picker.cancel();
  assert.ok(h.calls.some(([name,args])=>name==="discard_setup"&&args.review===5));
});
test("a partial batch requires a new preview and duplicate apply is ignored",async()=>{
  let complete;
  const batch=new Promise(resolve=>{complete=resolve;});
  const h=harness(name=>name==="preview_project"?preview:batch);
  await h.picker.choose("/new");const applying=h.picker.apply([0]);await h.picker.apply([0]);
  assert.equal(h.calls.filter(([name])=>name==="apply_setup").length,1);
  complete({complete:false,directory:"/new",files:[]});await applying;
  await h.picker.choose("/new",true);
  assert.equal(h.calls.filter(([name])=>name==="preview_project").at(-1)[1].review,true);
});
test("unavailable agents and invalid folders remain recoverable",async()=>{
  const h=harness(()=>{throw Error("Invalid folder");});
  await h.picker.choose("/missing");assert.match(h.picker.state.error,/Invalid folder/);
  const ready=harness(()=>({kind:"ready",directory:"/project"}));
  ready.picker.open=async()=>{throw Error("Agent unavailable");};
  await ready.picker.choose("/project");assert.equal(ready.picker.state.stage,"error");
  assert.match(ready.picker.state.error,/Agent unavailable/);
  ready.picker.open=async directory=>({workspace:directory,session:"later"});
  await ready.picker.choose("/project");assert.equal(ready.opened.length,1);
});
