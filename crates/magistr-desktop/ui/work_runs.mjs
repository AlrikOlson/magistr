import { element,statusLabel } from "./work_records.mjs";
export function workDate(value){
  if(!value)return null;
  const date=new Date(value);
  return Number.isNaN(date.getTime())?null:date.toLocaleString(undefined,{year:"numeric",month:"short",day:"numeric",hour:"numeric",minute:"2-digit"});
}
export class WorkRuns {
  constructor(select,changed){this.select=select;this.choice="current";this.keys="";select.onchange=()=>{this.choice=select.value;changed();};}
  reset(){this.choice="current";this.keys="";}
  update(data){
    const retained=[...(data?.retained??[])].reverse();
    const options=[["current","Current work"],...retained.map(run=>["run:"+run.run_id,
      run.objective.description+" · "+statusLabel(run.objective.status)+(workDate(run.objective.created_at)?" · "+workDate(run.objective.created_at):"")])];
    if(!options.some(([key])=>key===this.choice))options.push([this.choice,"Saved work is no longer available"]);
    const keys=JSON.stringify(options);
    if(keys!==this.keys){
      this.select.replaceChildren(...options.map(([key,label])=>{const option=element("option",label);option.value=key;return option;}));
      this.keys=keys;this.select.value=this.choice;
    }
    this.select.disabled=!data;
    return this.choice==="current"?data?.current:retained.find(run=>"run:"+run.run_id===this.choice);
  }
}
