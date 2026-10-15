// One finite entrance per opened surface. Preferences owns the effective motion policy.
export class SurfaceMotion {
  constructor(document){
    this.document=document;this.active=new Map();
    this.observer=new MutationObserver(records=>{
      if(document.documentElement.dataset.reducedMotion!=="false"){this.cancel();return;}
      for(const {target,attributeName,oldValue} of records){
        if(attributeName==="open"&&target.matches("dialog")){
          if(target.open&&oldValue===null)this.enter(target);else if(!target.open)this.stop(target);
        }else if(attributeName==="data-expanded"&&target.id==="composer"){
          if(target.dataset.expanded==="true"&&oldValue!=="true")this.enter(target.querySelector(".composer-box"));else if(target.dataset.expanded!=="true")this.stop(target.querySelector(".composer-box"));
        }
      }
    });
    this.observer.observe(document.documentElement,{subtree:true,attributes:true,attributeOldValue:true,attributeFilter:["open","data-expanded","data-reduced-motion"]});
  }
  enter(element){
    this.stop(element);
    if(!element?.animate||this.document.hidden)return;
    const animation=element.animate([{opacity:.65,transform:"translateY(8px) scale(.985)"},{opacity:1,transform:"translateY(0) scale(1)"}],{duration:160,easing:"cubic-bezier(.2,.7,.2,1)",iterations:1});
    this.active.set(element,animation);
    const settled=()=>{if(this.active.get(element)===animation)this.active.delete(element);};
    animation.finished.then(settled,settled);
  }
  stop(element){this.active.get(element)?.cancel();this.active.delete(element);}
  cancel(){for(const animation of this.active.values())animation.cancel();this.active.clear();}
  dispose(){this.observer.disconnect();this.cancel();}
}
