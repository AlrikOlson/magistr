// A presentation of the owner's complete, identified source revisions.
export class Conversation {
  constructor(address) { this.address=address; this.sequence=-1; this.messages=[]; this.pending=[]; this.dropped=0; this.status="Ready"; }
  apply(frame) {
    if (JSON.stringify(frame.address)!==JSON.stringify(this.address) || frame.sequence<=this.sequence) return false;
    const retained=new Set(frame.retained), existing=new Map(this.messages.map(message=>[message.id,message]));
    for(const incoming of frame.events) {
      const held=existing.get(incoming.id);
      if(held) Object.assign(held,incoming);
      else existing.set(incoming.id,{...incoming});
    }
    this.messages=frame.retained.map(id=>existing.get(id)).filter(Boolean);
    for(const id of existing.keys()) if(!retained.has(id))existing.delete(id);
    this.sequence=frame.sequence; this.pending=frame.pending; this.status=frame.status;this.dropped=frame.dropped;
    return true;
  }
  copy() { return this.messages.map(message=>message.role+"\n"+message.text).join("\n\n"); }
}
