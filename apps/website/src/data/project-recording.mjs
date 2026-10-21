export const chapters = [
  {id:"plan", task:"demo-plan", label:"Scope", summary:"Set the boundaries and decide who owns the data."},
  {id:"store", task:"demo-store", label:"Mail storage", summary:"Persist messages, join replies and handle duplicate deliveries."},
  {id:"api", task:"demo-api", label:"API", summary:"Connect intake, search, assignment and conflict-safe updates."},
  {id:"ui", task:"demo-ui", label:"Inbox", summary:"Build the interface, use the app, review it and fix what the review finds."},
  {id:"verify", task:"demo-verify", label:"Verify", summary:"Check the finished implementation and the app recording."}
];
export const checkOf = event => event.output?.check?.value;
export function eventStatus(event) {
  const check = checkOf(event);
  if (check) return check.passed ? "Passed" : "Failed";
  if (event.output?.ok === false) return "Rejected";
  return ({write:"Written",edit:"Edited",finish:"Completed",start:"Started",plan:"Saved",decision:"Recorded"})[event.kind] || "Recorded";
}
export function elapsed(recording, event) {
  const seconds=Math.max(0,Math.round((Date.parse(event.at)-Date.parse(recording.startedAt))/1000));
  return Math.floor(seconds/60)+":"+String(seconds%60).padStart(2,"0");
}
export function eventText(event) {
  const check=checkOf(event);
  if (check) return check.details;
  if (event.kind==="write") return event.input.content;
  if (event.kind==="edit") return "--- Before\n"+event.input.old+"\n\n+++ After\n"+event.input.new;
  if (event.kind==="decision") return event.input.outcome+"\n\n"+event.input.thought+"\n\nNext: "+event.input.next_action;
  if (event.output?.ok===false) return event.output.receipts?.[0]?.result?.message || JSON.stringify(event.output,null,2);
  if (event.kind==="plan") return event.input.title || event.input.reason;
  return event.output.title+"\nStatus: "+event.output.status;
}
export function filesAt(recording, index) {
  const files={};
  for(const event of recording.events.slice(0,index+1)) {
    if(event.output?.ok===false)continue;
    if(event.kind==="write")files[event.input.path]=event.input.content;
    if(event.kind==="edit"){
      const current=files[event.input.file];
      if(current===undefined || !current.includes(event.input.old))throw new Error("Recorded edit has no matching source: "+event.id);
      files[event.input.file]=current.split(event.input.old).join(event.input.new);
    }
  }
  return files;
}
export function milestonesAt(recording,index) {
  return chapters.map(chapter=>{
    const events=recording.events.slice(0,index+1).filter(event=>event.input.task_id===chapter.task && event.output?.ok!==false);
    const last=[...events].reverse().find(event=>event.kind==="start" || event.kind==="finish");
    return {...chapter,status:last?.kind==="finish"?"Completed":last?.kind==="start"?"In progress":"Planned"};
  });
}
