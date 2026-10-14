import { inputIdentity, sameDraft } from "./attachments.mjs";
// Pending sends are released only by an addressed owner observation or delivery failure.
export class Submissions {
  constructor(){this.items=new Map();}
  key(address){return JSON.stringify(address);}
  hasInput(address,input){const pending=this.items.get(this.key(address));return !!pending&&sameDraft(pending.input,input);}
  pending(address){return !!address&&this.items.has(this.key(address));}
  begin(address,input,sequence){this.items.set(this.key(address),{input:{text:input.text,attachments:input.attachments.map(file=>({...file}))},sequence});}
  fail(address){this.items.delete(this.key(address));}
  observe(frame){
    const item=this.items.get(this.key(frame.address));if(!item)return null;
    const admission=frame.admissions?.find(entry=>entry.sequence>item.sequence&&inputIdentity(entry)===inputIdentity(item.input));
    if(admission){this.fail(frame.address);return admission.accepted?item.input:null;}
    if(frame.closed||frame.status==="Connection lost")this.fail(frame.address);
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
export function shouldSaveDraft(address,input,sessions,submissions){
  return !!address&&sessions.some(session=>JSON.stringify(session.address)===JSON.stringify(address)&&!session.closed)&&!submissions.hasInput(address,input);
}
