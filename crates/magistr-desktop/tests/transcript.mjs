import test from "node:test";
import assert from "node:assert/strict";
import {Transcript,allowedDestination,nearBottom,selectedWithin} from "../ui/transcript.mjs";
test("link activation accepts only credential-free explicit web addresses",()=>{
 for(const value of ["https://example.com/日本?q=a#part","http://localhost:8080/"])assert.equal(allowedDestination(value),true);
 for(const value of ["javascript:alert(1)","file:///tmp/x","data:text/html,test","//example.com","/relative","https://u:p@example.com","https://example.com\n"])assert.equal(allowedDestination(value),false,value);
});
test("any selected range intersecting a message freezes that message",()=>{
 const selected={},other={},selection={isCollapsed:false,rangeCount:2,getRangeAt:index=>({intersectsNode:node=>index===1&&node===selected})};
 assert.equal(selectedWithin(selection,selected),true);
 assert.equal(selectedWithin(selection,other),false);
 assert.equal(selectedWithin({...selection,isCollapsed:true},selected),false);
});
test("Latest ends a held selection and flushes the current view before choosing the bottom",()=>{
 const selection={isCollapsed:false,removeAllRanges(){this.isCollapsed=true;}},view={revision:2};
 const reader={document:{getSelection:()=>selection},view,pending:true,following:false,
  root:{scrollTop:100,scrollHeight:1000},latest:{hidden:false},
  render(next){assert.equal(selection.isCollapsed,true);assert.equal(next,view);assert.equal(this.pending,false);this.root.scrollHeight=1500;}
 };
 Transcript.prototype.resumeLatest.call(reader);
 assert.equal(reader.root.scrollTop,1500);
 assert.equal(reader.following,true);
 assert.equal(reader.latest.hidden,true);
});
test("follow-bottom has a narrow threshold and does not treat old reading position as latest",()=>{
 assert.equal(nearBottom({scrollHeight:1000,scrollTop:700,clientHeight:300}),true);
 assert.equal(nearBottom({scrollHeight:1000,scrollTop:650,clientHeight:300}),false);
});
