import test from "node:test";
import assert from "node:assert/strict";
import { Catalog, usageText, capabilityText } from "../ui/supervision.mjs";
const address=(session,workspace="copy")=>({project:"project",workspace,session});
test("catalog accepts only its stream and increasing snapshots, including reseeds",()=>{
 const catalog=new Catalog();catalog.open(4);
 assert.equal(catalog.apply({stream:3,sequence:10,sessions:[]}),false);
 const a=address("a"),b=address("b","other");
 assert.equal(catalog.apply({stream:4,sequence:2,sessions:[{address:a,pending:[]},{address:b,pending:[{id:1,title:"Read"}]}]}),true);
 assert.deepEqual(catalog.waiting()[0],{address:b,id:1,title:"Read"});
 assert.equal(catalog.apply({stream:4,sequence:1,sessions:[]}),false);
 assert.equal(catalog.apply({stream:4,sequence:3,sessions:[{address:b,pending:[]}]}),true);
 assert.equal(catalog.waiting().length,0);
 catalog.open(5);assert.equal(catalog.apply({stream:4,sequence:100,sessions:[]}),false);
 assert.equal(catalog.apply({stream:5,sequence:3,sessions:[]}),true);
});
test("spending and provider facts retain unknown, zero and unavailable values",()=>{
 assert.match(usageText(null),/not reported/i);
 assert.match(usageText({used:null,size:null,cost_cents:0}),/\$0\.00/);
 assert.match(usageText({used:0,size:100,cost_cents:null}),/0 \/ 100/);
 assert.match(usageText({used:8,size:100,cost_cents:25}),/\$0\.25/);
 assert.match(capabilityText(null),/unavailable/i);
 assert.match(capabilityText({can_resume:false,can_send_images:false}),/cannot reopen/i);
 assert.match(capabilityText({can_resume:true,can_send_images:true}),/can reopen/i);
});
