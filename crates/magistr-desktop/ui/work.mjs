import { addressKey } from "./context.mjs";

export function workPresentation(frame, pending=false) {
  const run=frame?.work, phase=run?.phase?.kind, active=!!run&&phase!=="ended";
  const stopped=run?.requested==="abort", paused=phase==="paused";
  const requested=run?.requested;
  const status=pending?"Requesting work…":!run?"No work is running.":
    phase==="ended"?"Work ended.":paused?"Work paused.":
    stopped?"Finishing this item, then stopping.":
    requested==="pause"||requested==="step"?"Finishing this item, then pausing.":
    phase==="starting"?"Starting work.":"Work is running.";
  const detail=run?(phase==="ended"?run.phase.reason:
    "Attempt "+run.iteration+" of "+run.limit+". "+(paused?"Continue or run one more item.":
    "Changes to the run take effect after this item. Stop in the message box cancels the current reply.")):"Choose one item or up to ten items from the roadmap.";
  return {status,detail,active,paused,stopped,
    canStart:!!frame?.target&&!frame.closed&&!active&&!pending&&!frame.activity?.active&&!frame.activity?.configuration_in_progress&&!frame.activity?.cancellation_requested,
    error:frame?.work_error??frame?.work_admission?.error??null};
}

export class WorkPanel {
  constructor(element,invoke,onError,present=()=>{}) {
    this.$=element;this.invoke=invoke;this.onError=onError;this.present=present;this.pending=new Map();this.frame=null;
    for(const [id,action] of [["work-next","next"],["work-loop","loop"]])this.$(id).onclick=()=>this.start(action);
    for(const [id,action] of [["work-pause","pause"],["work-resume","resume"],["work-step","step"],["work-abort","abort"]])this.$(id).onclick=()=>this.control(action);
  }
  render(frame) {
    this.frame=frame;
    const key=frame?addressKey(frame.address):null, pending=this.pending.get(key);
    if(pending!==undefined&&(frame.closed||(frame.work_admission?.sequence??0)>pending))this.pending.delete(key);
    const view=workPresentation(frame,this.pending.has(key));
    this.$("work-status").textContent=view.status;this.$("work-detail").textContent=view.detail;
    this.$("work-error").textContent=view.error??"";this.$("work-error").hidden=!view.error;
    for(const id of ["work-next","work-loop"]) {this.$(id).disabled=!view.canStart;this.$(id).hidden=view.active||this.pending.has(key);}
    for(const id of ["work-pause","work-resume","work-step","work-abort"]) {
      this.$(id).hidden=!view.active;
      this.$(id).disabled=!!frame?.closed||view.stopped;
    }
    this.$("work-pause").hidden=!view.active||view.paused;
    this.$("work-pause").disabled||=frame?.work?.requested==="pause"||frame?.work?.requested==="step";
    this.$("work-resume").hidden=!view.active||(!view.paused&&!["pause","step"].includes(frame?.work?.requested));
    this.$("work-step").hidden=!view.active||!view.paused;
    this.$("work-abort").textContent=view.paused?"Stop work":"Finish item and stop";
    this.present(view,frame);
  }
  async start(action) {
    const frame=this.frame;if(!workPresentation(frame,this.pending.has(addressKey(frame?.address))).canStart)return;
    const key=addressKey(frame.address);this.pending.set(key,frame.sequence);this.render(frame);
    try {await this.invoke("session_action",{target:frame.target,action:{action}});}
    catch(error) {this.pending.delete(key);if(addressKey(this.frame?.address)===key)this.onError(error);}
    if(addressKey(this.frame?.address)===key)this.render(this.frame);
  }
  async control(action) {
    const frame=this.frame;if(!frame?.work||frame.work.phase.kind==="ended")return;
    const target={address:frame.address,run_id:frame.work.id};
    try {await this.invoke("control_work",{target,action});}
    catch(error) {if(addressKey(this.frame?.address)===addressKey(target.address))this.onError(error);}
  }
}
