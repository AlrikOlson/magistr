import { Drafts } from "./drafts.mjs";
import { AttachmentList } from "./attachments.mjs";

// Saved and resident attachments share one presentation; writes go to the history owner.
export class SavedDraftEditor {
  constructor(element, invoke, resident) {
    this.$=element;this.invoke=invoke;this.resident=resident;this.current=null;this.generation=0;this.busy=false;this.loading=false;this.operation=null;this.mutationError=null;
    this.writer=new Drafts((target,expectedRevision,text)=>invoke("save_saved_draft",{id:target.conversation,expectedRevision,text}),()=>this.render());
    this.files=new AttachmentList(element("saved-attachments"),id=>this.mutate("remove_saved_attachment",{attachmentId:id}),()=>element("saved-message").focus());
    element("saved-message").addEventListener("input",()=>{
      if(this.current&&!this.busy&&!this.loading&&!this.resident(this.current.conversation))this.writer.edit(this.current,element("saved-message").value);
    });
    element("save-saved-message").onclick=()=>this.retry();
    element("discard-saved-draft").onclick=()=>this.mutate("discard_saved_draft");
  }
  async open(record) {
    const generation=++this.generation;
    this.loading=true;this.render();
    try {
      await this.flush();
      if(generation!==this.generation)return false;
      const draft=await this.invoke("saved_draft",{id:record.id});
      if(generation!==this.generation)return false;
      if(this.current?.conversation!==record.id)this.files.clear();
      this.current={conversation:record.id};this.apply(draft);
      this.$("saved-draft").hidden=false;return true;
    }catch(error){if(generation!==this.generation)return false;throw error;}
    finally{if(generation===this.generation){this.loading=false;this.render();}}
  }
  cancelOpen() {++this.generation;this.loading=false;this.render();}
  observe(target,status) {
    this.writer.accept(target,status);
  }
  apply(draft) {
    this.mutationError=null;this.observe(this.current,draft.status);
    this.$("saved-message").value=draft.text;this.files.apply(draft.attachments,!!this.resident(this.current.conversation));this.render();
  }
  render() {
    if(!this.current)return;
    const owner=this.resident(this.current.conversation),open=!!owner,status=this.writer.status(this.current);
    const openMessage=owner?.closed?"This conversation is still closing. Wait for it to finish before editing this saved message.":
      owner?.target?.conversation===this.current.conversation?"This conversation is open. Resume it to edit the message.":
      "This conversation belongs to an open session. Close that session before editing this saved message.";
    this.$("saved-message").disabled=open||this.busy||this.loading;
    this.$("save-saved-message").disabled=open||this.busy||this.loading;
    this.$("save-saved-message").hidden=!this.mutationError&&!status.error;
    this.$("discard-saved-draft").disabled=open||this.busy||this.loading;
    this.files.apply(this.files.files,open||this.busy||this.loading);
    this.$("saved-draft-status").textContent=open?openMessage:
      (this.mutationError||status.error)?(this.mutationError||status.error):status.saving?"Saving draft…":"Draft saved";
  }
  perform(action) {
    if(!this.current||this.busy||this.loading||this.resident(this.current.conversation))return Promise.resolve();
    const target=this.current;
    this.busy=true;this.mutationError=null;this.render();
    this.operation=Promise.resolve().then(()=>action(target))
      .catch(error=>{if(this.current===target)this.mutationError=error?.message??String(error);})
      .finally(()=>{this.busy=false;this.operation=null;this.render();});
    return this.operation;
  }
  retry() {
    const text=this.$("saved-message").value;
    return this.perform(async target=>{
      // Settle any earlier save before refreshing its revision for an explicit retry.
      try{await this.writer.flush(target);}catch{}
      const latest=await this.invoke("saved_draft",{id:target.conversation});
      this.observe(target,latest.status);
      this.writer.edit(target,text);await this.writer.flush(target);
    });
  }
  mutate(command, extra={}) {
    return this.perform(async target=>{
      await this.writer.flush(target);
      const status=await this.invoke(command,{id:target.conversation,expectedRevision:this.writer.status(target).revision,...extra});
      this.writer.accept(target,status);
      if(status.error)throw new Error(status.error);
      const draft=await this.invoke("saved_draft",{id:target.conversation});
      this.observe(target,draft.status);
      if(this.current===target)this.apply(draft);
    });
  }
  async flush() {await this.operation;if(this.mutationError)throw new Error(this.mutationError);await this.writer.flushAll();}
}
