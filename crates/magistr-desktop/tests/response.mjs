import test from "node:test";
import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import { Conversation } from "../ui/conversation.mjs";
test("production batches remain bounded and apply within the response budget",()=>{
 const address={project:"p",workspace:"w",session:"s"},view=new Conversation(address),samples=[];
 for(let sequence=0;sequence<4096;sequence++){
  const started=performance.now();
  assert.equal(view.apply({address,sequence,reseed:sequence===0,dropped:0,pending:[],status:"Working",events:[
    {kind:"user",data:"message "+sequence},
    {kind:"text",data:"日本語 ".repeat(200)},
    {kind:"ended",data:"Finished"}
  ]}),true);
  samples.push(performance.now()-started);
 }
 samples.sort((a,b)=>a-b);
 const p99=samples[Math.floor(samples.length*.99)];
 assert.ok(p99<32,"batch p99 must stay within 32 ms, observed "+p99.toFixed(2));
 assert.ok(view.messages.length<=512);
 assert.ok(view.messages.reduce((sum,m)=>sum+new TextEncoder().encode(m.text).byteLength,0)<=1048576);
 console.log("production batch p99: "+p99.toFixed(2)+" ms");
});
