// Native button tabs keep keyboard focus independent of refreshed record content.
export class WorkTabs {
  constructor(root,changed){
    this.buttons=[...root.querySelectorAll("[data-work-view]")];this.changed=changed;this.current="work";
    for(const button of this.buttons){
      button.onclick=()=>this.select(button.dataset.workView);
      button.onkeydown=event=>{
        if(event.isComposing)return;
        const index=this.buttons.indexOf(button),keys={ArrowRight:(index+1)%this.buttons.length,ArrowLeft:(index+this.buttons.length-1)%this.buttons.length,Home:0,End:this.buttons.length-1};
        if(keys[event.key]===undefined)return;
        event.preventDefault();const next=this.buttons[keys[event.key]];this.select(next.dataset.workView);next.focus();
      };
    }
  }
  select(view){
    this.current=view;
    for(const button of this.buttons){const active=button.dataset.workView===view;button.setAttribute("aria-selected",String(active));button.tabIndex=active?0:-1;}
    this.changed(view);
  }
}
