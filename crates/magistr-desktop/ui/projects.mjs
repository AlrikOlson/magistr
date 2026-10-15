// Navigation and review lifetime only; Rust owns setup and session admission.
export class ProjectOpening {
  constructor(invoke,{open,opened,changed}) {
    this.invoke=invoke;this.open=open;this.opened=opened;this.changed=changed;
    this.intent=0;this.state={stage:"idle",directory:"",preview:null,result:null,error:null,errorTitle:null};
  }
  emit(next) {this.state={...this.state,...next};this.changed(this.state);}
  fail(error,title) {this.emit({stage:"error",error:error instanceof Error?error.message:String(error),errorTitle:title});}
  async discard(preview) {if(preview?.kind==="setup")await this.invoke("discard_setup",{review:preview.review});}
  async choose(directory,review=false) {
    const intent=++this.intent,previous=this.state.preview;
    this.emit({stage:"reading",directory,preview:null,result:null,error:null});
    try {
      await this.discard(previous);
      if(intent!==this.intent)return;
      const preview=await this.invoke("preview_project",{directory,review});
      if(intent!==this.intent){await this.discard(preview);return;}
      this.emit({directory:preview.directory,preview,stage:preview.kind==="setup"?"review":"opening"});
      if(preview.kind==="ready")await this.admit(intent,preview.directory);
    } catch(error) {if(intent===this.intent)this.fail(error,this.state.stage==="opening"?"Couldn’t start the conversation":"Couldn’t check this folder");}
  }
  async apply(approved) {
    if(this.state.stage!=="review")return;
    const intent=this.intent,preview=this.state.preview;
    this.emit({stage:"applying",error:null});
    try {
      const result=await this.invoke("apply_setup",{review:preview.review,approved});
      if(intent!==this.intent)return;
      this.emit({result,stage:result.complete?"opening":"partial"});
      if(result.complete)await this.admit(intent,result.directory);
    } catch(error) {if(intent===this.intent)this.fail(error,this.state.stage==="opening"?"Couldn’t start the conversation":"Couldn’t finish setup");}
  }
  async admit(intent,directory) {
    const address=await this.open(directory);
    if(intent!==this.intent)return;
    await this.opened(address);
    if(intent===this.intent)this.emit({stage:"opened",preview:null});
  }
  async cancel() {
    const preview=this.state.preview;
    ++this.intent;
    this.emit({stage:"declined",preview:null,error:null});
    await this.discard(preview);
  }
}
