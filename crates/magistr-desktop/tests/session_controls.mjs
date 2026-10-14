import test from "node:test";
import assert from "node:assert/strict";
import { Submissions, shouldSaveDraft, reconcileDraft, conversationOwner, resumeReason } from "../ui/session_controls.mjs";
import { sameDraft } from "../ui/attachments.mjs";
const address = session => ({address:{project:"project",workspace:"copy",session},conversation:session+"-history"});
const input = (text,attachments=[]) => ({text,attachments});
const file = (id,hash="hash",name="note.txt") => ({id,name,content_hash:hash});
test("a click and another session's acceptance never clear a pending draft",()=>{
  const submissions=new Submissions(),a=address("a"),b=address("b"),draft=input("hello");
  submissions.begin(a,draft,10);
  assert.equal(submissions.pending(a),true);
  assert.equal(submissions.observe({target:b,admissions:[{sequence:11,...draft,accepted:true}]}),null);
  assert.equal(submissions.pending(a),true);
  assert.equal(submissions.observe({target:a,admissions:[{sequence:9,...draft,accepted:true}]}),null);
  assert.deepEqual(submissions.observe({target:a,admissions:[{sequence:11,...draft,accepted:true}]}),draft);
  assert.equal(submissions.pending(a),false);
});
test("refusal releases submission without claiming acceptance",()=>{
  const submissions=new Submissions(),a=address("a"),draft=input("keep me",[file("one")]);
  submissions.begin(a,draft,2);
  assert.equal(submissions.observe({target:a,admissions:[{sequence:3,...draft,accepted:false}]}),null);
  assert.equal(submissions.pending(a),false);
});
test("pending input can be retried after delivery failure and never replaces another draft",()=>{
  const submissions=new Submissions(),a=address("a");
  submissions.begin(a,input("first"),0);submissions.fail(a);submissions.begin(a,input("second"),1);
  assert.equal(submissions.observe({target:a,admissions:[{sequence:2,...input("first"),accepted:true}]}),null);
  assert.equal(submissions.pending(a),true);
  assert.deepEqual(submissions.observe({target:a,admissions:[{sequence:3,...input("second"),accepted:true}]}),input("second"));
});
test("acceptance matches ordered file names and bytes even when text is identical",()=>{
  const submissions=new Submissions(),a=address("a"),draft=input("review",[file("one"),file("two","other","other.txt")]);
  submissions.begin(a,draft,5);
  for(const attachments of [[],[file("one","changed"),draft.attachments[1]],[...draft.attachments].reverse()]) {
    assert.equal(submissions.observe({target:a,admissions:[{sequence:6,text:draft.text,attachments,accepted:true}]}),null);
    assert.equal(submissions.pending(a),true);
  }
  const accepted=submissions.observe({target:a,admissions:[{sequence:6,text:draft.text,attachments:draft.attachments,accepted:true}]});
  assert.deepEqual(accepted,draft);
  assert.equal(sameDraft(input("review",[file("re-added"),draft.attachments[1]]),accepted),false);
});
test("a delayed acknowledgement for identical bytes cannot release a newly selected file",()=>{
  const submissions=new Submissions(),a=address("a");
  const old=input("review",[file("original")]),replacement=input("review",[file("selected-again")]);
  submissions.begin(a,old,5);
  submissions.fail(a);
  submissions.begin(a,replacement,7);
  for(const accepted of [true,false]) {
    assert.equal(submissions.observe({target:a,admissions:[{sequence:8,...old,accepted}]}),null);
    assert.equal(submissions.pending(a),true);
    assert.equal(sameDraft(old,replacement),false);
  }
  assert.deepEqual(submissions.observe({target:a,admissions:[{sequence:9,...replacement,accepted:true}]}),replacement);
  assert.equal(submissions.pending(a),false);
});
test("empty owner drafts and ordinary sequence changes never imply acceptance",()=>{
  const submissions=new Submissions(),a=address("a"),draft=input("",[file("one")]);
  submissions.begin(a,draft,5);
  assert.equal(submissions.observe({target:a,sequence:10,draft:"",attachments:[],admissions:[]}),null);
  assert.equal(submissions.pending(a),true);
});
test("coalesced attachment edits and acceptance retain the owner's changed draft",()=>{
  const a=address("a"),draft=input("review these files",[file("one"),file("two","other","other.txt")]);
  const changedAttachments=[
    [draft.attachments[0]],
    [...draft.attachments,file("three","third","third.txt")],
    [file("selected-again"),draft.attachments[1]]
  ];
  for(const attachments of changedAttachments) {
    const submissions=new Submissions();
    submissions.begin(a,draft,5);
    const frame={target:a,sequence:8,draft:draft.text,attachments,
      admissions:[{sequence:8,...draft,accepted:true}]};
    const accepted=submissions.observe(frame);
    assert.equal(submissions.pending(a),false);
    // receiveFrame still has the old visible files until it renders this frame.
    assert.equal(reconcileDraft(frame,draft,accepted,5),draft.text);
  }
});
test("acceptance clears only a matching local input after the owner clears its text",()=>{
  const submissions=new Submissions(),a=address("a"),draft=input("review",[file("one")]);
  submissions.begin(a,draft,5);
  const frame={target:a,sequence:6,draft:"",attachments:[],
    admissions:[{sequence:6,...draft,accepted:true}]};
  const accepted=submissions.observe(frame);
  assert.equal(reconcileDraft(frame,draft,accepted,5),"");
  assert.equal(reconcileDraft(frame,input("new thought",draft.attachments),accepted,5),"new thought");
  assert.equal(reconcileDraft(frame,input(draft.text,[file("selected-again")]),accepted,5),draft.text);
  assert.equal(reconcileDraft({...frame,admissions:[]},draft,null,5),draft.text);
  // New files can be staged after the accepted input was cleared.
  assert.equal(reconcileDraft({...frame,attachments:[file("new-file")]},draft,accepted,5),"");
});
test("coalesced refusal and attachment edits use the current owner draft without replacing local text",()=>{
  const submissions=new Submissions(),a=address("a"),draft=input("sent",[file("one")]);
  submissions.begin(a,draft,5);
  const frame={target:a,sequence:8,draft:"kept with the new file",attachments:[file("selected-again")],
    events:[],admissions:[{sequence:8,...draft,accepted:false}]};
  const accepted=submissions.observe(frame);
  assert.equal(submissions.pending(a),false);
  assert.equal(reconcileDraft(frame,input("new local thought",draft.attachments),accepted,5),"new local thought");
  assert.equal(reconcileDraft(frame,input("",draft.attachments),accepted,5),frame.draft);
  assert.equal(reconcileDraft({...frame,draft:""},input("",draft.attachments),accepted,5),"");
});
test("an explicit refusal restores the owner draft after transcript pruning without replacing new text",()=>{
  const frame={draft:"review these files",events:[],admissions:[{sequence:6,accepted:false}]};
  assert.equal(reconcileDraft(frame,input(""),null,5),"review these files");
  assert.equal(reconcileDraft(frame,input("new thought"),null,5),"new thought");
  assert.equal(reconcileDraft({...frame,admissions:[]},input(""),null,5),"");
  assert.equal(reconcileDraft({...frame,admissions:[{sequence:6,accepted:true}]},input(""),null,5),"");
  assert.equal(reconcileDraft({...frame,admissions:[],events:[{role:"Error"}]},input(""),null,5),"");
});
test("a rejected queued input restores once and a reseed cannot undo a later clear",()=>{
  const submissions=new Submissions(),a=address("a"),draft=input("queued",[file("one")]);
  submissions.begin(a,draft,5);
  const acceptedFrame={target:a,sequence:6,draft:"",attachments:[],
    admissions:[{sequence:6,...draft,accepted:true}]};
  assert.equal(reconcileDraft(acceptedFrame,draft,submissions.observe(acceptedFrame),5),"");
  assert.equal(submissions.pending(a),false);

  const rejectedFrame={target:a,sequence:9,draft:draft.text,attachments:draft.attachments,
    admissions:[{sequence:9,...draft,accepted:false}]};
  assert.equal(reconcileDraft(rejectedFrame,input(""),submissions.observe(rejectedFrame),6),draft.text);
  // The user clears the recovered text, then an unrelated update reseeds history.
  const laterFrame={...rejectedFrame,sequence:12,reseed:true};
  assert.equal(reconcileDraft(laterFrame,input("",draft.attachments),submissions.observe(laterFrame),9),"");
});
test("navigation never writes a draft to a catalog-confirmed closed owner",()=>{
  const submissions=new Submissions(),a=address("a");
  assert.equal(shouldSaveDraft(a,input("draft"),[{address:a.address,target:a,closed:false}],submissions),true);
  assert.equal(shouldSaveDraft(a,input("draft"),[{address:a.address,target:a,closed:true}],submissions),false);
  assert.equal(shouldSaveDraft(a,input("draft"),[],submissions),false);
  submissions.begin(a,input("accepted soon"),0);
  assert.equal(shouldSaveDraft(a,input("accepted soon"),[{address:a.address,target:a,closed:false}],submissions),false);
  assert.equal(shouldSaveDraft(a,input("new draft"),[{address:a.address,target:a,closed:false}],submissions),true);
});

test("identical submissions and reused file IDs remain isolated across a purpose switch",()=>{
 const submissions=new Submissions(),a=address("resident"),b={...a,conversation:"planning"},draft=input("same",[file("one")]);
 submissions.begin(a,draft,2);submissions.begin(b,draft,3);
 assert.deepEqual(submissions.observe({target:a,admissions:[{sequence:4,...draft,accepted:true}]}),draft);
 assert.equal(submissions.pending(a),false);
 assert.equal(submissions.pending(b),true);
 assert.equal(shouldSaveDraft(a,input("late"),[{address:b.address,target:b,closed:false}],submissions),true);
 assert.equal(shouldSaveDraft(b,input("new"),[{address:b.address,target:b,closed:false}],submissions),true);
});

test("saved history reflects active, parked, closing and released conversation claims",()=>{
 const target=address("resident"),session={address:target.address,target,owned_conversations:[target.conversation,"planning"],closed:false};
 const sessions=[session],record={id:"planning",resume_reason:null};
 assert.equal(conversationOwner(record.id,sessions),session);
 assert.match(resumeReason(record,sessions),/Close that session/);
 assert.equal(resumeReason({id:target.conversation,resume_reason:"Agent cannot resume"},sessions),null);
 session.closed=true;
 assert.match(resumeReason(record,sessions),/still closing/);
 session.owned_conversations=[];
 assert.equal(conversationOwner(record.id,sessions),undefined);
 assert.equal(resumeReason(record,sessions),null);
 assert.equal(resumeReason({...record,resume_reason:"Agent cannot resume"},sessions),"Agent cannot resume");
});
