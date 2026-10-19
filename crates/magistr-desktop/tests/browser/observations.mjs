// Deterministic native observations shared by Storybook and headless WebKit.
import { workSnapshot } from "./work_fixture.mjs";
import { roadmapFixture } from "./roadmap_fixture.mjs";
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
  const history=[
    ["release-notes","Release notes and rollback","magistr"],
    ["sync-review","Review the sync retry","magistr"],
    ["home-layout","Home page content and navigation","website"],
    ["auth-review","Review authorization changes","magistr"],
    ["install-guide","Improve the installation guide","magistr"],
    ["contact-page","Contact page accessibility","website"],
    ["project-search","Find projects more quickly","magistr"],
    ["agent-setup","Choose an agent for this project","magistr"],
    ["release-checks","Verify the release checklist","magistr"],
    ["mobile-layout","Review the narrow layout","website"],
    ["empty-states","Clearer messages for empty projects","magistr"],
    ["copy-review","Review product language","website"],
  ].map(([id,name,project],index)=>({
    id,name,workspace:"/Projects/"+project,provider:"GitHub Copilot",purpose:"conversation",model:"Claude Sonnet",
    updated_at:"2026-10-"+String(14-Math.floor(index/2)).padStart(2,"0")+"T15:30:00Z",ended_at:null,turns:"8",cost:"$0.42",resume_reason:null,
    summary:index===0?"The release is ready for a final review.\n\nCheck the installer on a clean machine, keep the previous build available, and confirm that reopening a project restores the unsent message.\n\n"+Array.from({length:8},(_,n)=>"Review "+(n+1)+": confirm the expected behavior with the person responsible for this part of the release. Keep a short note of what changed and how to recover.").join("\n\n"):"We reviewed the current behavior and agreed on the next change. Return to this conversation to continue with the same context."
  }));
  for(const record of history)record.document={nodes:record.summary.split("\n\n").map((value,index)=>paragraph(value,index*100))};
  const setup={kind:"setup",review:1,directory:"/Projects/new-project",setup_directory:"/Projects/new-project",files:[
    {path:".magistr/project.json",action:"Create file",ownership:"Managed by Magistr",reason:null,selectable:true,before:null,after:'{\n  "name": "New project",\n  "language": "Rust"\n}\n',diff:null},
    {path:"AGENTS.md",action:"Update file",ownership:"Shared file",reason:"Only the Magistr instructions will change. Your project notes stay in place.",selectable:true,before:"# Project notes\n",after:"# Project notes\n\nUse focused checks for this project.\n",diff:"@@ -1 +1,3 @@\n # Project notes\n+\n+Use focused checks for this project.\n"},
    {path:".claude/settings.json",action:"Keep file",ownership:"Your file",reason:"This file has been edited. Its current contents will be kept.",selectable:false,before:'{ "permissions": { "allow": [] } }',after:"",diff:null},
    ...Array.from({length:10},(_,index)=>({path:".agents/skills/"+["craft","next","validate","roadmap","roadmap-refresh","signals","handoff","workspace-routing","init","next-loop"][index]+"/SKILL.md",action:"Create file",ownership:"Managed by Magistr",reason:null,selectable:true,before:null,after:"# Project workflow\n\nRead the current project state before starting work.\n\n"+Array.from({length:12},()=>"- Keep each change focused and verify the affected behavior.").join("\n"),diff:null}))
  ]};
  const roadmap=roadmapFixture();
  return {preferences:{theme:"dark",reduce_motion:true},frames,history,setup,workflow:{...workSnapshot(),roadmap:roadmap.records},roadmap:roadmap.overview,projects:[{id:"magistr",name:"Magistr",directory:"/Projects/magistr"},{id:"website",name:"Website",directory:"/Projects/website"}]};
}
export function install(seed) {
  const frames=new Map(seed.frames.map(frame=>[frame.address.session,frame])),calls=[];
  const saved=new Map(seed.history.map((record,index)=>[record.id,{text:index===0?"Before shipping, confirm the rollback steps and check that the installation guide matches the new release.":"",revision:1}]));
  let stream=1,subscription=null,catalog=null,preferences={revision:1,preferences:structuredClone(seed.preferences)};
  const sessions=()=>[...frames.values()].map(f=>({address:f.address,target:f.target,owned_conversations:[f.conversation.id],
    name:seed.projects.find(p=>p.directory===f.address.workspace)?.name??"Project",conversation:f.conversation,status:f.status,closed:f.closed,
    pending:f.pending.map(({id,title})=>({id,title})),activity:f.activity,usage:f.usage,model:f.model,work:f.work}));
  const emit=()=>{if(subscription){const f=frames.get(subscription.session);subscription.channel.onmessage({...structuredClone(f),stream:subscription.stream,sequence:++f.sequence});}};
  window.__fixture={calls,frames,workflow:seed.workflow,roadmap:seed.roadmap,history:seed.history,saved,handlers:{},publish(session,patch){Object.assign(frames.get(session),patch);if(subscription?.session===session)emit();catalog?.onmessage({stream:1,sequence:++stream,sessions:sessions()});}};
  window.__TAURI__={core:{Channel:class{},async invoke(command,args={}) {
    calls.push({command,args:JSON.parse(JSON.stringify(args))});
    if(window.__fixture.handlers[command])return window.__fixture.handlers[command](args);
    switch(command) {
      case "read_roadmap_item":{
        const state=window.__fixture.workflow.roadmap;
        const wrap=value=>{
          const record=value.chunks.find(record=>record.id===args.id);if(!record)return null;
          const document=source=>({nodes:[{tag:"p",text:source,children:[],start:0}],limited:false});
          const relation=id=>{const item=value.chunks.find(item=>item.id===id);return {id,title:item?.title??null,status:item?.status??null,group:item?.group??null};};
          const overview=window.__fixture.roadmap,frontiers=(overview.state==="current"?overview.data:overview.data?.last)?.value.frontiers??[];
          return {record:structuredClone(record),readiness:frontiers.flatMap(track=>track.items).find(item=>item.id===record.id)?.readiness??{state:"inactive"},
            dependencies:record.deps.map(relation),dependents:value.chunks.filter(item=>item.deps.includes(record.id)).map(item=>relation(item.id)),
            description:document(record.description),notes:document(record.notes),
            requirements:record.acceptance.map(document),sections:(record.content?.sections??[]).map(section=>document(section.prose??""))};
        };
        const item=state.state==="current"?{state:"current",data:{...state.data,value:wrap(state.data.value)}}:
          state.state==="stale"?{state:"stale",data:{...state.data,last:{...state.data.last,value:wrap(state.data.last.value)}}}:state;
        return {address:args.address,id:args.id,revision:frames.get(args.address.session).workflow_revision??0,item};
      }
      case "read_workflow":return {address:args.address,revision:frames.get(args.address.session).workflow_revision??0,snapshot:structuredClone(window.__fixture.workflow),roadmap:structuredClone(window.__fixture.roadmap)};
      case "overview":return {directory:null,reduce_motion:false,sessions:sessions(),projects:seed.projects,attachment_limits:{file_bytes:8388608}};
      case "subscribe_catalog":catalog=args.channel;catalog.onmessage({stream:1,sequence:++stream,sessions:sessions()});return 1;
      case "subscribe_file_drops":case "subscribe_exit":case "acknowledge":case "session_action":return;
      case "agent_choices":return [{id:"copilot",label:"GitHub Copilot"},{id:"api",label:"API Agent"}];
      case "model_choices":return [{id:"sonnet",name:"Claude Sonnet"},{id:"gpt",name:"GPT"}];
      case "preview_project":return {...structuredClone(seed.setup),directory:args.directory,setup_directory:args.directory,review:++stream};
      case "discard_setup":return;
      case "pick_folder":return null;
      case "saved_conversations":return structuredClone(seed.history);
      case "saved_draft":{
        const record=saved.get(args.id);if(!record)throw new Error("Unknown saved conversation.");
        return {text:record.text,attachments:[],status:{conversation:args.id,revision:record.revision,saved_revision:record.revision,save_attempt:record.revision,error:null}};
      }
      case "save_saved_draft":{
        const record=saved.get(args.id);if(record.revision!==args.expectedRevision)throw new Error("Draft revision mismatch.");
        record.text=args.text;record.revision++;
        return {conversation:args.id,revision:record.revision,saved_revision:record.revision,save_attempt:record.revision,error:null};
      }
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
