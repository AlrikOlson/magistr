// An addressed presentation of the application owner's ordered observations.
export class Conversation {
  constructor(address) { this.address=address; this.sequence=-1; this.messages=[]; this.pending=[]; this.dropped=0; this.status="Ready"; this.active=false; }
  apply(frame) {
    if (JSON.stringify(frame.address)!==JSON.stringify(this.address) || frame.sequence<=this.sequence) return false;
    if (frame.reseed) { this.messages=[]; this.active=false; this.dropped=frame.dropped; }
    for (const event of frame.events) {
      const {kind,data}=event;
      if (kind==="user") { this.messages.push({role:"You",text:data}); this.active=false; }
      if (kind==="text" || kind==="thought") {
        const role=kind==="text"?"Agent":"Thinking";
        const tail=this.messages.at(-1);
        if(this.active && tail?.role===role) tail.text+=data;
        else this.messages.push({role,text:data});
        this.active=true;
      }
      if (kind==="ended") this.active=false;
      if (["notice","error","tool"].includes(kind)) {
        this.messages.push({role:kind==="tool"?"Command":"Magistr",text:data});
        this.active=false;
      }
    }
    let bytes=this.messages.reduce((n,m)=>n+new TextEncoder().encode(m.text).byteLength,0);
    while(this.messages.length>512 || bytes>1048576) {
      const removed=this.messages.shift();
      bytes-=new TextEncoder().encode(removed.text).byteLength; this.dropped++;
    }
    this.sequence=frame.sequence; this.pending=frame.pending; this.status=frame.status;
    return true;
  }
}
