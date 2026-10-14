import test from "node:test";
import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import { Conversation } from "../ui/conversation.mjs";
test("production full revisions stay within the response budget with bounded owner retention",()=>{
 const address={project:"p",workspace:"w",session:"s"},view=new Conversation(address),samples=[],retained=[];
 for(let sequence=0;sequence<4096;sequence++){
  const started=performance.now();
  retained.push(sequence);if(retained.length>512)retained.shift();
  assert.equal(view.apply({address,sequence,reseed:sequence===0,dropped:Math.max(0,sequence-511),pending:[],status:"Working",retained:[...retained],events:[
    {id:sequence,revision:sequence,role:"Agent",text:"日本語 ".repeat(100),document:null,tool:null}
  ]}),true);
  samples.push(performance.now()-started);
 }
 samples.sort((a,b)=>a-b);const p99=samples[Math.floor(samples.length*.99)];
 assert.ok(p99<32,"batch p99 must stay within 32 ms, observed "+p99.toFixed(2));
 assert.equal(view.messages.length,512);
 assert.ok(view.messages.reduce((sum,message)=>sum+new TextEncoder().encode(message.text).byteLength,0)<=1048576);
 console.log("production batch p99: "+p99.toFixed(2)+" ms");
});
