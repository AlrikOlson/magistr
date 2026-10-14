// The native configuration service owns persistence; this is the editable view.
export class Preferences {
  constructor(invoke, changed = () => {}) {
    this.invoke=invoke;this.changed=changed;this.snapshot=null;this.value=null;this.pending=null;this.running=null;this.error=null;
  }
  async load() {
    try {this.snapshot=await this.invoke("preferences");this.value={...this.snapshot.preferences};this.error=null;}
    catch(error){this.error=error?.message??String(error);}
    this.changed();
  }
  change(value) {
    this.value={...value};this.pending=this.value;this.error=null;this.start();this.changed();
  }
  start() {
    if(this.running)return;
    this.running=this.save().finally(()=>{this.running=null;this.changed();});
  }
  async save() {
    while(this.pending) {
      const value=this.pending;this.pending=null;
      try {
        this.snapshot=await this.invoke("save_preferences",{expectedRevision:this.snapshot.revision,preferences:value});
        this.error=null;
      } catch(error) {if(!this.pending)this.pending=value;this.error=error?.message??String(error);this.changed();return;}
      this.changed();
    }
  }
  async retry() {
    try {
      this.snapshot=await this.invoke("preferences");this.error=null;
      if(this.pending){this.start();await this.flush();}
      else {this.value={...this.snapshot.preferences};this.changed();}
    }catch(error){this.error=error?.message??String(error);this.changed();}
  }
  async flush() {
    if(this.running)await this.running;
    if(this.pending)throw new Error(this.error||"Your preferences have not been saved.");
  }
}
export function effectiveMotion(forced, saved, system) { return Boolean(forced||saved||system); }
