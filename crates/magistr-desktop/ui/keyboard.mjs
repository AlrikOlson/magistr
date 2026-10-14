// Window navigation keys only; native editing and composition keep their own keys.
export class KeyboardNavigation {
  constructor(element,layout) {
    this.$=element;this.layout=layout;this.mac=/Mac|iPhone|iPad/.test(navigator.platform);
    const modifier=this.mac?"⌘":"Ctrl",aria=this.mac?"Meta":"Control";
    const shortcuts=[
      ["find-conversation","K",false],["open-project","O",false],["new-session","N",true],["open-settings",",",false]
    ];
    for(const [id,key,shift] of shortcuts) {
      const display=modifier+" "+(shift?(this.mac?"⇧ ":"Shift "):"")+key,button=element(id);
      button.setAttribute("aria-keyshortcuts",aria+"+"+(shift?"Shift+":"")+key.toLowerCase());button.title=button.textContent.trim()+" · "+display;
      element(id+"-shortcut").textContent=display;
      if(id==="find-conversation")element("settings-find-shortcut").textContent=display;
    }
    document.addEventListener("keydown",event=>{
      if(event.defaultPrevented||event.isComposing||document.querySelector("dialog[open]"))return;
      if(event.key==="F6"&&!event.metaKey&&!event.ctrlKey&&!event.altKey){event.preventDefault();this.cycle(event.shiftKey?-1:1);return;}
      const modified=this.mac?event.metaKey&&!event.ctrlKey:event.ctrlKey&&!event.metaKey;
      if(!modified||event.altKey)return;
      const match=shortcuts.find(([,key,shift])=>event.key.toLowerCase()===key.toLowerCase()&&event.shiftKey===shift);
      if(match){event.preventDefault();const button=element(match[0]);if(!button.disabled)button.click();}
    });
  }
  cycle(direction) {
    const $=this.$,areas=[{root:$("project-rail"),focus:()=>{const row=$("projects").querySelector('[aria-current=true]');(row?.closest(".project-group")?.open&&row.getClientRects().length?row:$("find-conversation")).focus();}}];
    if(this.layout.enabled)areas.push(
      {root:$("transcript"),focus:()=>{if(this.layout.compact.matches)this.layout.show(false);$("transcript").focus({preventScroll:true});}},
      {root:$("composer"),focus:()=>{if(this.layout.compact.matches)this.layout.show(false);($("draft").disabled?$("composer"):$("draft")).focus({preventScroll:true});}},
      {root:$("session-detail"),focus:()=>{this.layout.show(true);$("detail-title").focus();}}
    );
    else areas.push({root:$("welcome"),focus:()=>$("welcome-open").focus()});
    const index=areas.findIndex(area=>area.root.contains(document.activeElement));
    const next=index<0?(direction>0?0:areas.length-1):(index+direction+areas.length)%areas.length;
    areas[next].focus();
  }
}
