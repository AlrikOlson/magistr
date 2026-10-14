import test from "node:test";
import assert from "node:assert/strict";
import { Submissions, shouldSaveDraft } from "../ui/session_controls.mjs";
const address = session => ({project:"project",workspace:"copy",session});
test("a click and another session's acceptance never clear a pending draft",()=>{
  const submissions=new Submissions(),a=address("a"),b=address("b");
  submissions.begin(a,"hello",10);
  assert.equal(submissions.pending(a),true);
  assert.equal(submissions.observe({address:b,admissions:[{sequence:11,text:"hello",accepted:true}]}),null);
  assert.equal(submissions.pending(a),true);
  assert.equal(submissions.observe({address:a,admissions:[{sequence:9,text:"hello",accepted:true}]}),null);
  assert.equal(submissions.observe({address:a,admissions:[{sequence:11,text:"hello",accepted:true}]}),"hello");
  assert.equal(submissions.pending(a),false);
});
test("refusal releases submission without claiming acceptance",()=>{
  const submissions=new Submissions(),a=address("a");
  submissions.begin(a,"keep me",2);
  assert.equal(submissions.observe({address:a,admissions:[{sequence:3,text:"keep me",accepted:false}]}),null);
  assert.equal(submissions.pending(a),false);
});
test("pending input can be retried after delivery failure and never replaces another draft",()=>{
  const submissions=new Submissions(),a=address("a");
  submissions.begin(a,"first",0);submissions.fail(a);submissions.begin(a,"second",1);
  assert.equal(submissions.observe({address:a,admissions:[{sequence:2,text:"first",accepted:true}]}),null);
  assert.equal(submissions.pending(a),true);
  assert.equal(submissions.observe({address:a,admissions:[{sequence:3,text:"second",accepted:true}]}),"second");
});
test("navigation never writes a draft to a catalog-confirmed closed owner",()=>{
  const submissions=new Submissions(),a=address("a");
  assert.equal(shouldSaveDraft(a,"draft",[{address:a,closed:false}],submissions),true);
  assert.equal(shouldSaveDraft(a,"draft",[{address:a,closed:true}],submissions),false);
  assert.equal(shouldSaveDraft(a,"draft",[],submissions),false);
  submissions.begin(a,"accepted soon",0);
  assert.equal(shouldSaveDraft(a,"accepted soon",[{address:a,closed:false}],submissions),false);
  assert.equal(shouldSaveDraft(a,"new draft",[{address:a,closed:false}],submissions),true);
});
