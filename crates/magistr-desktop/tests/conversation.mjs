import assert from "node:assert/strict";
import test from "node:test";
import { Conversation } from "../ui/conversation.mjs";

const address = {project:"p",workspace:"/p",session:"s"};
const frame = (sequence, events, extra={}) => ({address,sequence,events,reseed:false,pending:[],draft:"",status:"Ready",...extra});
test("duplicate and late observations never alter a conversation",()=>{
  const view=new Conversation(address);
  view.apply(frame(2,[{kind:"user",data:"hello"},{kind:"text",data:"reply"}]));
  assert.equal(view.apply(frame(1,[{kind:"text",data:"old"}])),false);
  assert.equal(view.apply(frame(2,[{kind:"text",data:"duplicate"}])),false);
  assert.deepEqual(view.messages.map(m=>m.text),["hello","reply"]);
});
test("a reseed clears incomplete retained text and exposes the gap",()=>{
  const view=new Conversation(address);
  view.apply(frame(1,[{kind:"text",data:"old"}]));
  view.apply(frame(9,[{kind:"text",data:"retained"}],{reseed:true,dropped:4}));
  assert.deepEqual(view.messages.map(m=>m.text),["retained"]);
  assert.equal(view.dropped,4);
});
test("another session cannot change this view or its permission request",()=>{
  const view=new Conversation(address);
  view.apply(frame(1,[],{pending:[{id:7,title:"read",choices:[]}]}));
  assert.equal(view.apply(frame(8,[],{address:{...address,session:"other"},pending:[]})),false);
  assert.equal(view.pending[0].id,7);
});
test("turn boundaries keep separate replies and user-authored bytes",()=>{
  const view=new Conversation(address);
  view.apply(frame(1,[{kind:"user",data:"日本\nhello"},{kind:"text",data:"one "},{kind:"text",data:"two"},{kind:"ended",data:"Finished"},{kind:"user",data:"next"},{kind:"text",data:"three"}]));
  assert.deepEqual(view.messages.map(m=>m.text),["日本\nhello","one two","next","three"]);
});
test("retained presentation is bounded and says when older messages were removed",()=>{
  const view=new Conversation(address);
  for(let i=1;i<=700;i++) view.apply(frame(i,[{kind:"user",data:"x".repeat(4000)}]));
  assert.ok(view.messages.length<=512);
  assert.ok(view.messages.reduce((n,m)=>n+m.text.length,0)<=1048576);
  assert.ok(view.dropped>0);
});
