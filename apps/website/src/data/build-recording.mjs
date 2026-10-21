export const kindLabels={message:"Conversation",roadmap:"Roadmap",reasoning:"Reasoning",change:"File change",check:"Check",work:"Work"};
export const statusLabels={pending:"Planned",in_progress:"In progress",done:"Completed",planned:"Planned",active:"In progress",completed:"Completed",backlog:"Later",blocked:"Blocked"};
export function elapsed(recording,index){
 const seconds=Math.max(0,Math.floor((Date.parse(recording.events[index].at)-Date.parse(recording.events[0].at))/1000));
 return Math.floor(seconds/60)+":"+String(seconds%60).padStart(2,"0");
}
export function stateAt(recording,index){
 const chunks=new Map(),steps=new Map(),tasks=new Map(),branches=new Map(),files=new Map(),checks=[];
 for(const event of recording.events.slice(0,index+1)){
  if(event.chunk)chunks.set(event.chunk.id,event.chunk);
  if(event.step){
   const step=event.step;steps.set(step.step_number,step);
   if(step.branch_id&&!branches.has(step.branch_id))branches.set(step.branch_id,{id:step.branch_id,name:step.branch_name,from_step:step.branch_from,status:"active"});
  }
  if(event.branch){const b=event.branch;branches.set(b.branch_id,{...branches.get(b.branch_id),status:b.status,merged_into:b.merged_into});}
  if(event.task&&typeof event.task==="object")tasks.set(event.task.id,event.task);
  if(event.file)files.set(event.file.path,event.file);
  if(event.check)checks.push({...event.check,eventId:event.id,task:typeof event.task==="string"?event.task:undefined});
 }
 return {chunks:[...chunks.values()],steps:[...steps.values()],branches:[...branches.values()],tasks:[...tasks.values()],files:[...files.values()],checks};
}
export function chapters(recording){
 const events=recording.events;
 const find=(fn,fallback=events.length-1)=>{const index=events.findIndex(fn);return index<0?fallback:index;};
 const opening=find(e=>e.kind==="message"&&e.role==="user",0);
 return [
  {label:"Plan the game",index:opening,note:"You ask for a game. Codex agrees on the scope, then saves a plan for building it."},
  {label:"Check the rules",index:find(e=>e.operation==="roadmap_complete_chunk"&&e.chunk?.id==="example-ttt-rules"),note:"Occupied squares, alternating turns, all eight winning lines, and a full-board draw."},
  {label:"Choose the opponent",index:find(e=>e.branch?.merged_into===1752),note:"Try a simple opponent, find its losing game, then check an opponent that looks ahead."},
  {label:"Build the board",index:find(e=>e.operation==="roadmap_complete_chunk"&&e.chunk?.id==="example-ttt-board"),note:"The same Rust rules now drive a real Bevy 0.20.0 board, with pointer and keyboard input."},
  {label:"Play another round",index:find(e=>e.operation==="roadmap_complete_chunk"&&e.chunk?.id==="example-ttt-rematch"),note:"Keep the score, clear the board, and let the other player start."},
  {label:"Finish & ship",index:find(e=>e.id==="message-11"),note:"Follow the rest of the conversation through the final checks, push, and handover."},
 ];
}
export function preview(event){
 const lines=event.output.split("\n");
 return {text:lines.slice(0,event.kind==="change"?20:18).join("\n"),omitted:Math.max(0,lines.length-(event.kind==="change"?20:18))};
}
