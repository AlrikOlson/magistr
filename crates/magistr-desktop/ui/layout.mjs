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

// Compact project browsing keeps the same rail, conversation and draft nodes mounted.
export class ProjectRailLayout {
  constructor({root,panel,content,toggle,fallback,compact}) {
    Object.assign(this,{root,panel,content,toggle,fallback,compact});this.open=false;this.returnFocus=null;
    toggle.onclick=()=>this.show(!this.open,true);
    compact.addEventListener("change",()=>{
      const focus=panel.ownerDocument.activeElement;
      if(compact.matches)this.open=panel.contains(focus);
      this.render();
      if(!compact.matches&&focus===toggle)this.focusProjects();
    });
    root.addEventListener("keydown",event=>{
      if(event.key==="Escape"&&!event.isComposing&&!event.defaultPrevented&&compact.matches&&this.open){
        event.preventDefault();this.show(false,true);
      }
    });
    this.render();
  }
  focusProjects() {
    const selected=this.panel.querySelector('[aria-current=true]');
    const target=selected?.getClientRects().length?selected:this.panel.querySelector(".project-heading")??this.fallback;
    target.focus({preventScroll:true});
  }
  show(value,focus=false) {
    if(value&&!this.open)this.returnFocus=this.panel.ownerDocument.activeElement;
    this.open=value;this.render();
    if(!focus)return;
    if(value)this.focusProjects();
    else {
      const previous=this.returnFocus;
      (previous?.isConnected&&previous.getClientRects().length&&!previous.disabled?previous:this.toggle).focus({preventScroll:true});
    }
  }
  render() {
    const compact=this.compact.matches,open=compact&&this.open;
    this.root.dataset.projects=String(open);this.toggle.hidden=!compact;
    this.toggle.setAttribute("aria-expanded",String(open));
    this.panel.hidden=compact&&!open;this.content.hidden=open;
  }
}

