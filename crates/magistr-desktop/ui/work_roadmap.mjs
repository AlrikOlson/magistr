import { element } from "./work_dom.mjs";
export class RoadmapBrowser {
  constructor(select,changed){this.select=select;this.choice=undefined;this.keys="";select.onchange=()=>{this.choice=select.value;changed();};}
  reset(){this.choice=undefined;this.keys="";}
  update(data){
    const frontiers=data?.frontiers??[];
    if(this.choice===undefined&&frontiers.length)this.choice=JSON.stringify((frontiers.find(f=>f.items.length)??frontiers[0]).group);
    const choices=frontiers.map(f=>[JSON.stringify(f.group),(f.group??"Other work")+" · "+(f.activity.counts.total?f.activity.counts.total+" open":"Past work")]);
    if(this.choice!==undefined&&!choices.some(([id])=>id===this.choice)&&frontiers.length)choices.push([this.choice,"Track no longer available"]);
    const key=JSON.stringify(choices);
    if(key!==this.keys){this.select.replaceChildren(...choices.map(([id,label])=>{const option=element("option",label);option.value=id;return option;}));this.keys=key;}
    this.select.value=this.choice??"";this.select.disabled=!frontiers.length;
    return frontiers.find(f=>JSON.stringify(f.group)===this.choice);
  }
}
