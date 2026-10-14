// Pending sends are released only by an addressed owner observation or delivery failure.
export class Submissions {
  constructor(){this.items=new Map();}
  key(address){return JSON.stringify(address);}
  hasText(address,text){return this.items.get(this.key(address))?.text===text;}
  pending(address){return !!address&&this.items.has(this.key(address));}
  begin(address,text,sequence){this.items.set(this.key(address),{text,sequence});}
  fail(address){this.items.delete(this.key(address));}
  observe(frame){
    const item=this.items.get(this.key(frame.address));if(!item)return null;
    const admission=frame.admissions?.find(entry=>entry.sequence>item.sequence&&entry.text===item.text);
    if(admission){this.fail(frame.address);return admission.accepted?item.text:null;}
    if(frame.closed||frame.status==="Connection lost")this.fail(frame.address);
    return null;
  }
}
export function shouldSaveDraft(address,value,sessions,submissions){
  return !!address&&sessions.some(session=>JSON.stringify(session.address)===JSON.stringify(address)&&!session.closed)&&!submissions.hasText(address,value);
}

