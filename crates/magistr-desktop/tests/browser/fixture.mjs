// Production UI in an isolated headless browser. This fixture does not verify Rust owners or native input.
import { test as base, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { resolve, extname } from "node:path";
import { fileURLToPath } from "node:url";
export const root=fileURLToPath(new URL("../../../..",import.meta.url));
export const ui=resolve(root,"crates/magistr-desktop/ui");
const text=(tag,value,start)=>({tag,text:value,start,children:[]});
const paragraph=(value,start)=>text("p",value,start);
function frame(session,name,project="Magistr",directory="/Projects/magistr") {
  const address={project:project.toLowerCase(),workspace:directory,session};
  const conversation={id:session+"-conversation",name,provider:"copilot",provider_label:"GitHub Copilot",purpose:"conversation"};
  return {stream:1,address,target:{address,conversation:conversation.id},sequence:1,reseed:true,dropped:0,
    events:[],retained:[],pending:[],draft:"",draft_revision:0,draft_saved_revision:0,draft_save_attempt:0,draft_save_error:null,
    attachments:[],status:"Ready",model:"Claude Sonnet",closed:false,conversation,
    activity:{active:false,queued_turns:0,configuration_in_progress:false,cancellation_requested:false,can_resume:true,can_send_images:true},
    configuration:[{id:"model",name:"Model",current_value:"sonnet",values:[["sonnet","Claude Sonnet"],["gpt","GPT"]]}],
    usage:{used:18400,size:200000,cost_cents:42},admissions:[],work:null,work_admission:null,work_error:null};
}
export function workspaceSeed() {
  const first=frame("release","Review the release plan"),second=frame("sync","Investigate sync failure"),
    third=frame("website","Update the home page","Website","/Projects/website");
  first.events=[
    {id:1,revision:1,role:"You",text:"Review the desktop release plan. Focus on what someone needs during a long working session.",attachments:[]},
    {id:2,revision:1,role:"Agent",text:"A clearer place to work",attachments:[],document:{nodes:[
      text("h2","A clearer place to work",0),
      paragraph("The main task is staying oriented while several conversations and pieces of work are active. Keep the conversation central, with project navigation close by and details available when needed.",26),
      text("h3","What I found",220),
      paragraph("The release plan needs three decisions: how projects organize conversations, where ongoing work appears, and how someone returns to the paragraph or draft they left.",236),
      text("ul","",420),
      paragraph("I recommend reviewing navigation first, then testing a long conversation with unsent text, several files, and a request waiting in another session.",440),
      text("h3","Next step",590),
      paragraph("Try switching to the sync investigation and back. Your unsent message should stay with this conversation, and the background agent should keep working.",600),
    ]}},
  ];
  first.events[1].document.nodes[4].children=[
    {tag:"li",start:420,children:[paragraph("Give each project a stable place and show its conversations together.",420)]},
    {tag:"li",start:430,children:[paragraph("Make running, waiting, and finished work easy to distinguish.",430)]},
  ];
  first.retained=first.events.map(m=>m.id);
  first.draft="Keep the project list visible, and let me return to the same paragraph.";
  first.draft_revision=first.draft_saved_revision=1;
  second.events=[{id:1,revision:1,role:"Agent",text:"I found the failed request. The retry can resume after you review the proposed change.",attachments:[]}];
  second.retained=[1];second.status="Waiting";
  second.pending=[{id:7,title:"Run the focused sync checks",detail:"cargo nextest run -p sync --lib",choices:[{option_id:"allow",label:"Allow once"},{option_id:"deny",label:"Decline"}]}];
  const frames=[first,second,third];
  return {frames,projects:[{id:"magistr",name:"Magistr",directory:"/Projects/magistr"},{id:"website",name:"Website",directory:"/Projects/website"}]};
}
function install(seed) {
  const frames=new Map(seed.frames.map(frame=>[frame.address.session,frame])),calls=[];
  let stream=1,subscription=null,catalog=null,preferences={revision:1,preferences:{theme:"dark",reduce_motion:true}};
  const sessions=()=>[...frames.values()].map(f=>({address:f.address,target:f.target,owned_conversations:[f.conversation.id],
    name:seed.projects.find(p=>p.directory===f.address.workspace)?.name??"Project",conversation:f.conversation,status:f.status,closed:f.closed,
    pending:f.pending.map(({id,title})=>({id,title})),activity:f.activity,usage:f.usage,model:f.model,work:f.work}));
  const emit=()=>{if(subscription){const f=frames.get(subscription.session);subscription.channel.onmessage({...structuredClone(f),stream:subscription.stream,sequence:++f.sequence});}};
  window.__fixture={calls,frames,publish(session,patch){Object.assign(frames.get(session),patch);if(subscription?.session===session)emit();catalog?.onmessage({stream:1,sequence:++stream,sessions:sessions()});}};
  window.__TAURI__={core:{Channel:class{},async invoke(command,args={}) {
    calls.push({command,args:JSON.parse(JSON.stringify(args))});
    switch(command) {
      case "overview":return {directory:null,reduce_motion:false,sessions:sessions(),projects:seed.projects,attachment_limits:{file_bytes:8388608}};
      case "subscribe_catalog":catalog=args.channel;catalog.onmessage({stream:1,sequence:++stream,sessions:sessions()});return 1;
      case "subscribe_file_drops":case "subscribe_exit":case "acknowledge":case "session_action":return;
      case "preferences":return structuredClone(preferences);
      case "save_preferences":preferences={revision:preferences.revision+1,preferences:args.preferences};return structuredClone(preferences);
      case "subscribe_session":subscription={channel:args.channel,session:args.address.session,stream:++stream};emit();return subscription.stream;
      case "working_copies":return [{path:args.directory,branch:"main",is_main:true,state:{status:"available"}}];
      case "save_draft":{
        const f=frames.get(args.target.address.session);
        if(f.draft!==args.text){f.draft=args.text;f.draft_revision++;f.draft_saved_revision=f.draft_revision;}
        return {conversation:f.target.conversation,revision:f.draft_revision,saved_revision:f.draft_saved_revision,save_attempt:++f.draft_save_attempt,error:null};
      }
      default:throw new Error("Unimplemented fixture command: "+command);
    }
  }}};
}
export const test=base.extend({
  app:async({page},use)=>{
    const errors=[];page.on("pageerror",error=>errors.push(error.message));
    await page.route("**/*",async route=>{
      const url=new URL(route.request().url());
      if(url.origin!=="http://magistr.test"){await route.abort();return;}
      const file=resolve(ui,url.pathname==="/"?"index.html":"."+url.pathname);
      if(!file.startsWith(ui+"/")){await route.abort();return;}
      const types={".html":"text/html",".css":"text/css",".mjs":"text/javascript",".png":"image/png"};
      await route.fulfill({body:await readFile(file),contentType:types[extname(file)]??"text/plain"});
    });
    await page.addInitScript(install,workspaceSeed());
    await page.goto("http://magistr.test");
    await expect(page.locator("html")).toHaveAttribute("data-theme","dark");
    await use(page);
    expect(errors).toEqual([]);
  },
});
export { expect };
