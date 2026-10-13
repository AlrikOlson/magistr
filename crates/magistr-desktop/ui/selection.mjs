import { Conversation } from "./conversation.mjs";
// Navigation intentions begin before any asynchronous draft or IPC work.
export class Selection {
  constructor() { this.generation=0; this.current=null; }
  async open(address,{save,subscribe,commit,receive}) {
    const generation=++this.generation;
    await save();
    if(generation!==this.generation)return false;
    const view=new Conversation(address);
    let admitted=false,initial=null;
    const stream=await subscribe(address,generation,frame=>{
      if(admitted) {if(this.current?.view===view)receive(view,frame);}
      else if(generation===this.generation)initial=frame;
    });
    if(generation!==this.generation)return false;
    this.current={address,view,stream};
    admitted=true;commit(view,initial);
    return true;
  }
}
