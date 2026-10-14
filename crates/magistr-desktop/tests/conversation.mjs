import assert from "node:assert/strict";
import test from "node:test";
import { Conversation } from "../ui/conversation.mjs";
const address={project:"p",workspace:"/p",session:"s"};
const message=(id,revision,text,role="Agent")=>({id,revision,role,text,document:null,tool:null});
const frame=(sequence,events,retained=events.map(value=>value.id),extra={})=>({address,sequence,events,retained,reseed:false,pending:[],draft:"",dropped:0,status:"Ready",...extra});
test("late and wrongly addressed frames cannot alter retained source",()=>{
 const view=new Conversation(address);view.apply(frame(2,[message(1,2,"reply")]));
 assert.equal(view.apply(frame(1,[message(1,1,"old")])),false);
 assert.equal(view.apply(frame(2,[message(1,2,"duplicate")])),false);
 assert.equal(view.apply(frame(3,[],[],{address:{...address,session:"other"}})),false);
 assert.equal(view.messages[0].text,"reply");
});
test("stream updates and reseeds retain source object identity and exact original copy",()=>{
 const view=new Conversation(address);
 view.apply(frame(1,[message(1,1,"# 日本\r\n")]));
 const first=view.messages[0];
 view.apply(frame(5,[message(1,5,"# 日本\r\n**café**\n")]));
 assert.equal(view.messages[0],first);
 assert.equal(first.text,"# 日本\r\n**café**\n");
 view.apply(frame(9,[message(1,5,first.text),message(8,8,"next")],[1,8],{reseed:true}));
 assert.equal(view.messages[0],first);
 assert.equal(view.copy(),"Agent\n# 日本\r\n**café**\n\n\nAgent\nnext");
});
test("owner pruning drops unavailable source on every frame and keeps surviving identity",()=>{
 const view=new Conversation(address);view.apply(frame(2,[message(1,1,"old"),message(2,2,"keep")]));
 const keep=view.messages[1];
 view.apply(frame(8,[],[2],{dropped:6}));
 assert.deepEqual(view.messages,[keep]);assert.equal(view.dropped,6);
 view.apply(frame(12,[message(9,9,"remaining")],[9],{reseed:true,dropped:9}));
 assert.deepEqual(view.messages.map(value=>value.text),["remaining"]);
});
test("late tool updates replace one entry without changing transcript order",()=>{
 const view=new Conversation(address);view.apply(frame(3,[message(1,1,"Tool","Command"),message(2,2,"below")]));
 const tool=view.messages[0],below=view.messages[1];
 view.apply(frame(5,[{...message(1,5,"Tool\nStatus: Failed\n\nOutput\nerror","Command"),tool:{title:"Tool",status:"Failed"}}],[1,2]));
 assert.equal(view.messages[0],tool);assert.equal(view.messages[1],below);
 assert.equal(view.messages.length,2);assert.equal(tool.tool.status,"Failed");
});
test("current requests remain authoritative with source pruning",()=>{
 const view=new Conversation(address);
 view.apply(frame(1,[],[],{pending:[{id:7,title:"Read",choices:[]}]}));
 assert.equal(view.pending[0].id,7);
 view.apply(frame(2,[],[],{pending:[],dropped:3}));
 assert.equal(view.pending.length,0);
});
