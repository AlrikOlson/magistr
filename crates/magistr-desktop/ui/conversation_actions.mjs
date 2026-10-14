import { sameTarget } from "./context.mjs";

// The dialog owns only its edit and pending acknowledgement, never the conversation name.
export class ConversationName {
  constructor(element,invoke) {
    this.$=element;this.invoke=invoke;this.frame=null;this.target=null;this.pending=false;
    this.$("rename-conversation").onclick=()=>this.open();
    this.$("rename-cancel").onclick=()=>this.$("rename-picker").close();
    this.$("rename-picker").oncancel=event=>{if(this.pending)event.preventDefault();};
    this.$("rename-picker").onclose=()=>{
      this.target=null;
      const trigger=this.$("conversation-menu-trigger");
      if(!trigger.closest("[hidden]"))trigger.focus();
    };
    this.$("name-form").onsubmit=event=>{event.preventDefault();this.save();};
  }
  open() {
    if(!this.frame?.target||this.frame.closed||document.querySelector("dialog[open]"))return;
    this.target=this.frame.target;
    this.$("conversation-actions").open=false;
    this.$("conversation-name").value=this.frame.conversation?.name??"";
    this.$("rename-error").hidden=true;
    this.$("rename-picker").showModal();this.render(this.frame);
    this.$("conversation-name").focus();this.$("conversation-name").select();
  }
  render(frame) {
    this.frame=frame;
    this.$("rename-conversation").disabled=!frame?.target||frame.closed;
    const changed=this.target&&!sameTarget(this.target,frame?.target);
    this.$("save-name").disabled=this.pending||changed||!frame?.target||frame.closed;
    this.$("conversation-name").disabled=this.pending;
    this.$("rename-cancel").disabled=this.pending;
    this.$("rename-status").hidden=!this.pending;
    this.$("rename-status").textContent=this.pending?"Saving name…":"";
    if(changed){this.$("rename-error").textContent="This conversation changed. Close this dialog and try again.";this.$("rename-error").hidden=false;}
  }
  async save() {
    if(this.pending||!sameTarget(this.target,this.frame?.target)||this.frame?.closed)return;
    const target=this.target,name=this.$("conversation-name").value;
    this.pending=true;this.$("rename-error").hidden=true;this.render(this.frame);
    try {
      await this.invoke("session_action",{target,action:{action:"rename",name}});
      this.$("rename-picker").close();
    } catch(error) {
      this.$("rename-error").textContent=error?.message??String(error);this.$("rename-error").hidden=false;
    } finally {
      this.pending=false;this.render(this.frame);
      if(this.$("rename-picker").open)this.$("conversation-name").focus();
    }
  }
}
