import { addressKey, targetKey } from "./context.mjs";
import { inputIdentity, sameDraft } from "./attachments.mjs";
// Pending sends are released only by an addressed owner observation or delivery failure.
export class Submissions {
  constructor(){this.items=new Map();}
  key(target){return targetKey(target);}
  hasInput(target,input){const pending=this.items.get(this.key(target));return !!pending&&sameDraft(pending.input,input);}
  pending(target){return !!target&&this.items.has(this.key(target));}
  begin(target,input,sequence){this.items.set(this.key(target),{target,input:{text:input.text,attachments:input.attachments.map(file=>({...file}))},sequence});}
  fail(target){this.items.delete(this.key(target));}
  forgetSession(address){
    for(const [key,item] of this.items)
      if(addressKey(item.target.address)===addressKey(address))this.items.delete(key);
  }
  observe(frame){
    const item=this.items.get(this.key(frame.target));if(!item)return null;
    const admission=frame.admissions?.find(entry=>entry.sequence>item.sequence&&inputIdentity(entry)===inputIdentity(item.input));
    if(admission){this.fail(frame.target);return admission.accepted?item.input:null;}
    if(frame.closed||frame.status==="Connection lost")this.fail(frame.target);
    return null;
  }
}
export function reconcileDraft(frame,input,accepted,previousSequence) {
  // A frame can combine an attachment edit with an older acceptance. The owner
  // retains that changed draft even when the visible attachments still match.
  if(accepted!==null&&frame.draft===""&&sameDraft(input,accepted))return "";
  const rejected=frame.admissions.some(admission=>!admission.accepted&&admission.sequence>previousSequence);
  return !input.text&&rejected?frame.draft:input.text;
}
export function shouldSaveDraft(target,input,sessions,submissions){
  return !!target&&sessions.some(session=>addressKey(session.address)===addressKey(target.address)&&!session.closed)&&!submissions.hasInput(target,input);
}

export function conversationOwner(id,sessions) {
  return sessions.find(session=>session.owned_conversations.includes(id));
}
export function resumeReason(record,sessions) {
  const owner=conversationOwner(record.id,sessions);
  if(owner?.closed)return "This conversation is still closing. Wait for it to finish, then try again.";
  if(owner&&owner.target?.conversation!==record.id)return "This conversation is paused in an open session. Close that session before resuming it here.";
  return owner?null:record.resume_reason;
}
