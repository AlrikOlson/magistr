// The window's catalog cursor contains presentation facts, never provider ownership.
export class Catalog {
  constructor(){this.stream=null;this.sequence=-1;this.sessions=[];}
  open(stream){this.stream=stream;this.sequence=-1;}
  apply(frame){
    if(frame.stream!==this.stream||frame.sequence<=this.sequence)return false;
    this.sequence=frame.sequence;this.sessions=frame.sessions;return true;
  }
  waiting(){return this.sessions.flatMap(session=>session.pending.map(request=>({address:session.address,...request})));}
}
export function usageText(usage) {
  const context=usage?.used!=null&&usage?.size!=null?usage.used.toLocaleString()+" / "+usage.size.toLocaleString()+" context tokens":"Context use not reported";
  return context+" · "+(usage?.cost_cents==null?"Spending not reported":"$"+(usage.cost_cents/100).toFixed(2));
}
export function capabilityText(activity) {
  if(activity?.can_resume==null||activity?.can_send_images==null)return "Agent features are unavailable.";
  return (activity.can_resume?"This agent can reopen conversations.":"This agent cannot reopen conversations.")+" "+(activity.can_send_images?"It accepts images.":"It accepts text.");
}
