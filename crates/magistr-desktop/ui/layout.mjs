// Layout owns only which native document surface is visible, never session lifetime.
export class WorkspaceLayout {
  constructor({root, conversation, detail, toggle, heading, compact}) {
    Object.assign(this,{root,conversation,detail,toggle,heading,compact});
    this.enabled=false;this.wideOpen=false;this.narrowOpen=false;
    toggle.onclick=()=>this.show(!this.open,true);
    compact.addEventListener("change",()=>{
      const focus=detail.ownerDocument.activeElement;
      if(compact.matches)this.narrowOpen=detail.contains(focus);
      this.render();
    });
    detail.addEventListener("keydown",event=>{
      if(event.key==="Escape"&&!event.isComposing&&!event.defaultPrevented) {
        event.preventDefault();this.show(false);toggle.focus();
      }
    });
    this.render();
  }
  get open() {return this.compact.matches?this.narrowOpen:this.wideOpen;}
  enable(value) {this.enabled=value;this.render();}
  show(value,focus=false) {
    if(this.compact.matches)this.narrowOpen=value;else this.wideOpen=value;
    this.render();
    if(value&&focus)this.heading.focus();
  }
  render() {
    const open=this.enabled&&this.open;
    const focus=this.detail.ownerDocument.activeElement;
    this.root.dataset.details=String(open);
    this.detail.hidden=!open;
    this.conversation.hidden=this.enabled&&this.compact.matches&&open;
    this.toggle.hidden=!this.enabled;
    this.toggle.setAttribute("aria-expanded",String(open));
    if(!open&&this.detail.contains(focus)&&this.enabled)this.toggle.focus();
    if(open&&this.compact.matches&&this.conversation.contains(focus))this.heading.focus();
  }
}
