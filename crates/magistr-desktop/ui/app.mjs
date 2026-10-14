import { Transcript } from "./transcript.mjs";
import { Submissions, shouldSaveDraft, reconcileDraft } from "./session_controls.mjs";
import { attachComposer } from "./composer.mjs";
import { AttachmentList, clipboardFiles, sizeLabel } from "./attachments.mjs";
import { Selection } from "./selection.mjs";
import { ProjectOpening } from "./projects.mjs";
import { Catalog, usageText, capabilityText } from "./supervision.mjs";
const {invoke,Channel}=window.__TAURI__.core;
const $=id=>document.getElementById(id);
let selected=null,view=null,sessions=[],projects=[],composerFrame=null,attachmentLimits=null;
const attachmentWork=new Set();
const submissions=new Submissions();
const selection=new Selection();
const catalog=new Catalog();
const railNodes=new Map(),attentionNodes=new Map();
let announcementKey="";
const transcriptView=new Transcript($("transcript"),$("latest"),{
  copy:value=>navigator.clipboard.writeText(value).catch(error=>showError("Could not copy the text. "+error)),
  openLink:destination=>invoke("open_link",{destination}).catch(showError)
});
let configurationKey="";
let requestsKey="",railKey="";
const labels=new Map();let issuedLabel=0;
const showError=error=>{$("error").textContent=String(error);$("error").hidden=false;};
const clearError=()=>{$("error").hidden=true;};
const button=(label,click)=>{const element=document.createElement("button");element.textContent=label;element.onclick=click;return element;};
const text=(tag,value)=>{const element=document.createElement(tag);element.textContent=value;return element;};
const attachmentList=new AttachmentList($("pending-attachments"),id=>{
  const address=selected;
  return changeAttachments(address,()=>invoke("remove_attachment",{address,id}));
},()=>$("add-files").focus());

async function refresh() {
  const info=await invoke("overview");if(catalog.sequence<0)sessions=info.sessions;projects=info.projects;
  attachmentLimits=info.attachment_limits;
  for(const session of sessions)if(!labels.has(session.address.session))labels.set(session.address.session,++issuedLabel);
  renderRail();return info;
}
function renderRail() {
  const signature=JSON.stringify([projects,sessions,selected]);if(signature===railKey)return;railKey=signature;
  $("projects").replaceChildren(...projects.map(project=>button(project.name,()=>openDirectory(project.directory))));
  const rows=sessions.map(session=>{
    const id=session.address.session,index=labels.get(id),name=session.conversation?.name??(session.name+" session "+index);
    let row=railNodes.get(id);
    if(!row) {
      row=document.createElement("div");row.className="session-row";
      const item=button("",()=>select(session.address).catch(showError));item.append(text("span",""),text("small",""));
      const path=text("small","Working copy: "+session.address.workspace.split(/[\\/]/).filter(Boolean).at(-1));path.className="path";path.title=session.address.workspace;path.setAttribute("aria-label","Working copy "+session.address.workspace);
      const actions=document.createElement("div");actions.className="rail-actions";
      actions.append(button("Stop",()=>act(session.address,"cancel")),button("Reconnect",()=>act(session.address,"reconnect")),button("Close",()=>closeAddress(session.address)));
      row.append(item,path,actions);railNodes.set(id,row);
    }
    const item=row.children[0];item.children[0].textContent=session.conversation?.name??(session.name+" · "+index);
    const status=session.status+(session.pending.length?" · "+session.pending.length+" waiting":"")+(session.activity?.queued_turns?" · "+session.activity.queued_turns+" queued":"");
    if(item.children[1].textContent!==status)item.children[1].textContent=status;
    item.setAttribute("aria-current",String(selected?.session===id));item.setAttribute("aria-label",name);
    const [stop,reconnect,close]=row.children[2].children;
    stop.disabled=session.closed||session.activity?.cancellation_requested||!["Working","Waiting for you"].includes(session.status);
    reconnect.disabled=session.closed||session.status!=="Connection lost"||session.activity?.can_resume!==true;
    close.disabled=session.closed;
    stop.setAttribute("aria-label","Stop "+name);reconnect.setAttribute("aria-label","Reconnect "+name);close.setAttribute("aria-label","Close "+name);
    return row;
  });
  placeChildren($("sessions"),rows);
  for(const id of railNodes.keys())if(!sessions.some(session=>session.address.session===id))railNodes.delete(id);
  for(const id of labels.keys())if(!sessions.some(session=>session.address.session===id))labels.delete(id);
  const waiting=sessions.flatMap(session=>session.pending.map(request=>({session,request})));
  const waitingRows=waiting.map(({session,request})=>{
    const key=JSON.stringify([session.address,request.id]);
    let item=attentionNodes.get(key);
    if(!item){item=button("",()=>select(session.address,request.id).catch(showError));attentionNodes.set(key,item);}
    item.textContent=(session.conversation?.name??(session.name+" session "+labels.get(session.address.session)))+": "+request.title;
    return item;
  });
  placeChildren($("attention"),waitingRows);
  for(const [key,item] of attentionNodes)if(!waitingRows.includes(item))attentionNodes.delete(key);
  const failed=sessions.filter(session=>session.status==="Connection lost").length;
  const announcement=waiting.length+(waiting.length===1?" request waiting":" requests waiting")+(failed?" · "+failed+(failed===1?" agent disconnected":" agents disconnected"):"");
  if(announcement!==announcementKey){$("attention-count").textContent=announcement;announcementKey=announcement;}
}
function placeChildren(parent,children) {
  children.forEach((element,index)=>{if(parent.children[index]!==element)parent.insertBefore(element,parent.children[index]??null);});
  while(parent.children.length>children.length)parent.lastChild.remove();
}
async function act(address,action) {
  try{await invoke("session_action",{address,action:{action}});}catch(error){showError(error);}
}
async function connectCatalog() {
  const channel=new Channel();let early=null,ready=false;
  const receive=frame=>{
    if(!catalog.apply(frame))return;
    sessions=catalog.sessions;
    for(const session of sessions)if(!labels.has(session.address.session))labels.set(session.address.session,++issuedLabel);
    renderRail();
    invoke("acknowledge",{stream:frame.stream,sequence:frame.sequence}).catch(showError);
  };
  channel.onmessage=frame=>ready?receive(frame):early=frame;
  catalog.open(await invoke("subscribe_catalog",{channel}));ready=true;
  if(early)receive(early);

}
function renderComposer() {
  if(!composerFrame)return;
  const frame=composerFrame,busy=attachmentWork.has(JSON.stringify(frame.address));
  $("send").disabled=busy||submissions.pending(frame.address)||frame.closed||frame.activity?.cancellation_requested;
  $("add-files").disabled=busy||frame.closed;
  attachmentList.apply(frame.attachments, busy||frame.closed);
  const images=attachmentList.files.some(file=>file.mime_type);
  const note=busy?"Updating files…":images&&frame.activity?.can_send_images===false?
    "This agent cannot receive images. Remove the image before sending.":
    images&&frame.activity?.can_send_images!==true?"Image support has not been reported. Your files stay here if the agent cannot receive them.":
    attachmentLimits?"Add text files or PNG, JPEG and WebP images · "+sizeLabel(attachmentLimits.file_bytes)+" per file.":"";
  if($("attachment-note").textContent!==note)$("attachment-note").textContent=note;
}
async function changeAttachments(address,operation) {
  if(!address)return;
  const key=JSON.stringify(address);
  if(attachmentWork.has(key)){showError("Wait for the current file selection to finish.");return;}
  attachmentWork.add(key);renderComposer();clearError();
  try{await operation();}catch(error){showError(error);}
  finally{attachmentWork.delete(key);renderComposer();}
}
async function connectFileDrops() {
  const channel=new Channel();
  channel.onmessage=notice=>{
    if(notice.kind==="error"){showError(notice.message);return;}
    if(!selected||JSON.stringify(selected)!==JSON.stringify(notice.address)||selection.current?.stream!==notice.stream||document.querySelector("dialog[open]")) {
      showError("The selected conversation changed. Drop the files again.");return;
    }
    const address=selected;
    changeAttachments(address,()=>invoke("attach_dropped_files",{address,stream:notice.stream,selection:notice.selection}));
  };
  await invoke("subscribe_file_drops",{channel});
}
async function saveDraft(address=selected,input=attachmentList.input($("draft").value)) {
  if(!shouldSaveDraft(address,input,sessions,submissions))return;
  await invoke("save_draft",{address,text:input.text});
}
async function select(address,requestId=null) {
  const previous=selected,input=attachmentList.input($("draft").value);
  const accepted=await selection.open(address,{
    save:()=>previous?saveDraft(previous,input):Promise.resolve(),
    subscribe:async(address,intent,receive)=>{
      const channel=new Channel();channel.onmessage=receive;
      return invoke("subscribe_session",{address,channel,selection:intent});
    },
    commit:(candidate,initial)=>{
      selected=address;view=candidate;candidate.requestId=requestId;requestsKey="";railKey="";configurationKey="";
      $("welcome").hidden=true;$("workspace").hidden=false;$("view-title").textContent="Conversation";
      $("draft").value="";composerFrame=null;attachmentList.clear();
      $("draft").disabled=true;$("send").disabled=true;$("add-files").disabled=true;$("attachment-note").textContent="";
      $("workspace-path").textContent=address.workspace;
      $("project-name").textContent=sessions.find(session=>session.address.session===address.session)?.name??"Project";
      $("status").textContent="Connecting";renderRail();clearError();
      if(initial)receiveFrame(candidate,initial);
    },
    receive:receiveFrame
  });
  if(!accepted)return;
  const copies=await invoke("working_copies",{directory:address.workspace});
  if(selected?.session!==address.session)return;
  $("working-copy").replaceChildren(...copies.map(copy=>{
    const option=document.createElement("option");option.value=copy.path;
    option.textContent=(copy.state.status==="unversioned"?"Project folder":copy.branch??"Detached working copy")+(copy.is_main?" · Main":"");
    option.selected=copy.path===address.workspace;
    option.disabled=!["available","archived","unversioned"].includes(copy.state.status);return option;
  }));
}
function receiveFrame(candidate,frame) {
  const previousSequence=candidate.sequence;
  if(candidate.apply(frame)) {
    if(previousSequence<0) $("draft").value=frame.draft;
    const accepted=submissions.observe(frame);
    const draft=reconcileDraft(frame,attachmentList.input($("draft").value),accepted,previousSequence);
    if(draft!==$("draft").value)$("draft").value=draft;
    render(frame);
  }
  invoke("acknowledge",{stream:frame.stream,sequence:frame.sequence}).catch(()=>{});
}
function render(frame) {
  transcriptView.render(view);
  if($("status").textContent!==frame.status)$("status").textContent=frame.status;
  $("stop").disabled=!["Working","Waiting for you"].includes(frame.status);
  composerFrame=frame;renderComposer();
  $("draft").disabled=frame.closed;
  $("new-session").disabled=frame.closed;$("close-session").disabled=frame.closed;
  $("copy").disabled=view.messages.length===0;
  $("model").textContent=frame.model??"Model information is unavailable.";
  renderConfiguration(frame);
  $("usage").textContent=usageText(frame.usage);
  $("capabilities").textContent=capabilityText(frame.activity);
  $("queue").textContent=frame.activity?frame.activity.queued_turns+(frame.activity.queued_turns===1?" message queued":" messages queued")+(frame.activity.cancellation_requested?" · Waiting for the agent to stop.":""):"Queue information is unavailable.";
  $("reconnect").disabled=frame.closed||frame.status!=="Connection lost"||frame.activity?.can_resume!==true;
  const pendingKey=JSON.stringify([frame.address,view.pending]);
  if(pendingKey!==requestsKey) {$("requests").replaceChildren(...view.pending.map(request=>{
    const section=document.createElement("section");section.className="request";section.dataset.request=String(request.id);section.tabIndex=-1;section.setAttribute("aria-label","Request: "+request.title);
    section.append(text("h2","Waiting for you"),text("h3",request.title));
    if(request.detail)section.append(text("pre",request.detail));
    for(const choice of request.choices) section.append(button(choice.label,async event=>{
      const address=frame.address;const trigger=event.currentTarget;
      trigger.disabled=true;
      try {await invoke("session_action",{address,action:{action:"answer",id:request.id,option_id:choice.option_id}});}
      catch(error){trigger.disabled=false;showError(error);}
    }));
    return section;
  }));requestsKey=pendingKey;}
  if(view.requestId!==null) {
    const target=$("requests").querySelector('[data-request="'+view.requestId+'"]');
    if(target)target.focus();
    else if(frame.reseed)showError("This request is no longer waiting for an answer.");
    view.requestId=null;
  }
}
async function openSession(directory,fresh=false) {
  const resident=!fresh&&sessions.find(session=>session.address.workspace===directory);
  if(resident)return resident.address;
  return invoke("open_session",{directory,choice:{kind:"launch"}});
}
async function openKnownWorkspace(directory,fresh=false) {
  try {
    clearError();
    const address=await openSession(directory,fresh);
    await refresh();await select(address);$("draft").focus();
  } catch(error){showError(error);}
}
async function openDirectory(directory) {
  const resident=sessions.find(session=>session.address.workspace===directory);
  if(resident)return openKnownWorkspace(directory);
  showPicker(directory);await projectOpening.choose(directory);
}
function showWelcome(note="") {
  selected=null;view=null;composerFrame=null;attachmentList.clear();selection.clear();requestsKey="";railKey="";
  $("workspace").hidden=true;$("welcome").hidden=false;
  $("project-name").textContent="Magistr";$("view-title").textContent="Projects";$("status").textContent="Ready";
  $("draft").value="";$("requests").replaceChildren();transcriptView.clear();
  for(const id of ["draft","send","stop","new-session","close-session","copy","reconnect","add-files"])$(id).disabled=true;
  $("welcome-note").textContent=note;$("welcome-note").hidden=!note;
  renderRail();
}
function showPicker(directory="") {
  $("picker-error").hidden=true;
  if(directory)$("project-folder").value=directory;
  if(!$("project-picker").open)$("project-picker").showModal();
  $("project-folder").focus();
}
let reviewKey="";
const projectOpening=new ProjectOpening(invoke,{
  open:directory=>openSession(directory),
  opened:async address=>{await refresh();await select(address);$("project-picker").close();$("draft").focus();},
  changed:renderProjectState
});
function renderProjectState(state) {
  const busy=["reading","applying","opening"].includes(state.stage);
  for(const id of ["browse-folder","project-folder","picker-submit","picker-cancel","select-files","clear-files"])$(id).disabled=busy;
  $("picker-error").textContent=state.error??"";$("picker-error").hidden=!state.error;
  const status={reading:"Reading folder…",applying:"Applying selected files…",opening:"Opening the agent…",partial:"Setup is incomplete. Review it again to finish.",declined:"Setup review closed."}[state.stage]??"";
  $("picker-status").textContent=status;$("picker-status").hidden=!status;
  $("picker-submit").textContent=state.stage==="review"?"Apply selected files":["partial","error"].includes(state.stage)&&state.preview?.kind==="setup"&&!state.result?.complete?"Review again":"Open";
  if(state.directory)$("project-folder").value=state.directory;
  $("setup-preview").hidden=state.preview?.kind!=="setup"||!!state.result;
  if(state.preview?.kind==="setup"&&String(state.preview.review)!==reviewKey) {
    reviewKey=String(state.preview.review);
    $("setup-root").textContent="Files in "+state.preview.setup_directory;
    $("setup-files").replaceChildren(...state.preview.files.map((file,index)=>{
      const row=document.createElement("article");row.className="setup-file";
      const label=document.createElement("label");
      if(file.selectable) {const check=document.createElement("input");check.type="checkbox";check.checked=true;check.dataset.index=String(index);check.setAttribute("aria-label",file.path);label.append(check);}
      label.append(text("span",file.path));row.append(label,text("small",file.action+" · "+file.ownership));
      if(file.reason)row.append(text("p",file.reason));
      if(file.selectable) {
        const details=document.createElement("details");details.append(text("summary","Review "+file.path));
        if(file.diff)details.append(text("pre",file.diff));
        else {if(file.before!==null)details.append(text("h4","Current contents"),text("pre",file.before));details.append(text("h4",file.action==="Remove file"?"Contents to remove":"Contents after setup"),text("pre",file.action==="Remove file"?file.before:file.after));}
        row.append(details);
      }
      return row;
    }));
  }
  $("setup-result").hidden=!state.result;
  if(["review","partial","error"].includes(state.stage))$("picker-content").scrollTop=0;
  if(state.result) {
    $("setup-result-title").textContent=state.result.complete?"Setup complete":"Setup is incomplete";
    $("setup-record").textContent=state.result.setup_record;
    const outcomes={applied:"Applied",skipped:"Kept",refused:"Cannot change",failed:"Failed"};
    $("setup-outcomes").replaceChildren(...state.result.files.map(file=>text("li",file.path+" · "+outcomes[file.outcome]+(file.detail?" · "+file.detail:""))));
  }
}
async function browseFolder() {
  try {
    const directory=await invoke("pick_folder");
    if(directory){showPicker(directory);await projectOpening.choose(directory);}
  }catch(error){showPicker();$("picker-error").textContent=String(error);$("picker-error").hidden=false;}
}
$("add-files").onclick=()=>{
  const address=selected;
  changeAttachments(address,()=>invoke("pick_attachments",{address}));
};
$("draft").addEventListener("paste",event=>{
  try {
    const files=clipboardFiles(event,attachmentLimits,attachmentList.files);
    if(!files.length||!selected)return;
    const address=selected;
    changeAttachments(address,async()=>{
      for(const file of files) {
        const bytes=new Uint8Array(await file.arrayBuffer());
        await invoke("attach_clipboard_file",{address,name:file.name||"Pasted image",bytes:Array.from(bytes)});
      }
    });
  }catch(error){showError(error);}
});
// The native host owns file drops; prevent a dropped URL from navigating the page.
document.addEventListener("dragover",event=>event.preventDefault());
document.addEventListener("drop",event=>event.preventDefault());
attachComposer($("draft"),$("composer"));
$("composer").onsubmit=async event=>{
  event.preventDefault();if(!selected||submissions.pending(selected)||attachmentWork.has(JSON.stringify(selected)))return;
  const address=selected,input=attachmentList.input($("draft").value);
  if(!input.text.trim()&&!input.attachments.length)return;
  submissions.begin(address,input,view.sequence);$("send").disabled=true;clearError();
  try {
    await invoke("save_draft",{address,text:input.text});
    await invoke("session_action",{address,action:{action:"send",text:input.text,attachment_ids:input.attachments.map(file=>file.id)}});
  } catch(error){submissions.fail(address);showError(error);if(selected?.session===address.session)renderComposer();}
};
$("stop").onclick=()=>selected&&act(selected,"cancel");
$("reconnect").onclick=()=>selected&&act(selected,"reconnect");
$("draft").onblur=()=>saveDraft().catch(showError);

$("new-session").onclick=()=>selected&&chooseAgent(selected.workspace).catch(showError);
$("working-copy").onchange=event=>openKnownWorkspace(event.target.value);
$("open-project").onclick=()=>showPicker();
$("welcome-path").onclick=()=>showPicker();
$("welcome-open").onclick=()=>{showPicker();browseFolder();};
$("browse-folder").onclick=browseFolder;
async function closePicker() {
  await projectOpening.cancel();$("project-picker").close();
  if(!selected)showWelcome("Open the folder again whenever you want to continue setup.");
}
$("picker-cancel").onclick=()=>closePicker().catch(showError);
$("project-picker").oncancel=event=>{
  event.preventDefault();
  if(!["applying","opening"].includes(projectOpening.state.stage))closePicker().catch(showError);
};
for(const [id,value] of [["select-files",true],["clear-files",false]])$(id).onclick=()=>{
  $("setup-files").querySelectorAll("input[type=checkbox]").forEach(input=>{input.checked=value;});
};
$("project-form").onsubmit=async event=>{
  event.preventDefault();
  const directory=$("project-folder").value,state=projectOpening.state;
  if(state.stage==="review"&&directory===state.directory) {
    const approved=Array.from($("setup-files").querySelectorAll("input:checked"),input=>Number(input.dataset.index));
    await projectOpening.apply(approved);
  } else {
    const review=directory===state.directory&&state.preview?.kind==="setup"&&!state.result?.complete;
    await projectOpening.choose(directory,!!review);
  }
};
$("theme").onchange=event=>{if(event.target.value==="auto")delete document.documentElement.dataset.theme;else document.documentElement.dataset.theme=event.target.value;};
$("copy").onclick=async()=>{
  const value=view?.copy()??"";
  try{await navigator.clipboard.writeText(value);}catch(error){showError("Could not copy the conversation. "+error);}
};
async function closeAddress(address) {
  try {
    await invoke("close_session",{address});
    const info=await refresh();await connectCatalog();sessions=info.sessions;renderRail();
    if(selected?.session!==address.session)return;
    const next=sessions.find(session=>!session.closed);
    if(next)await select(next.address);
    else showWelcome("Your session has closed. Open a project to start another.");
  }catch(error){showError(error);}
}
$("close-session").onclick=()=>selected&&closeAddress(selected);

function renderConfiguration(frame) {
  $("view-title").textContent=frame.conversation?.name??"Conversation";
  if(document.activeElement!==$("conversation-name"))$("conversation-name").value=frame.conversation?.name??"";
  $("conversation-name").disabled=frame.closed||!frame.conversation;
  $("save-name").disabled=frame.closed||!frame.conversation;
  $("agent-provider").textContent=frame.conversation?.provider_label??"Agent information is unavailable.";
  const key=JSON.stringify([frame.address,frame.configuration]);
  if(key!==configurationKey) {
    $("agent-configuration").replaceChildren(...(frame.configuration??[]).map(option=>{
      const label=document.createElement("label");label.append(text("span",option.name));
      const select=document.createElement("select");select.setAttribute("aria-label",option.name);
      for(const [value,name] of option.values){const choice=document.createElement("option");choice.value=value;choice.textContent=name;choice.selected=value===option.current_value;select.append(choice);}
      if(!option.values.some(([value])=>value===option.current_value)){const unavailable=document.createElement("option");unavailable.textContent="Current choice unavailable";unavailable.disabled=true;unavailable.selected=true;select.append(unavailable);}
      select.onchange=async()=>{
        const value=select.value;select.value=option.current_value;select.disabled=true;
        try{await invoke("session_action",{address:frame.address,action:{action:"configure",id:option.id,value}});}
        catch(error){showError(error);select.disabled=false;}
      };
      label.append(select);return label;
    }));configurationKey=key;
  }
  for(const select of $("agent-configuration").querySelectorAll("select"))select.disabled=frame.closed||frame.activity?.active||!!frame.activity?.queued_turns||frame.activity?.cancellation_requested||frame.activity?.configuration_in_progress;
}
$("name-form").onsubmit=async event=>{
  event.preventDefault();if(!selected)return;
  try{await invoke("session_action",{address:selected,action:{action:"rename",name:$("conversation-name").value}});}
  catch(error){showError(error);}
};
const agentLabels=new Map();
let newWorkspace=null,chosenHistory=null,openingGeneration=0;
async function chooseAgent(directory) {
  const generation=++openingGeneration;newWorkspace=directory;$("initial-model-control").hidden=true;$("initial-model").replaceChildren();$("choose-initial-model").disabled=false;$("session-picker-error").hidden=true;$("agent-choice").replaceChildren();
  $("session-picker-open").disabled=true;$("session-picker").showModal();
  try {
    const choices=await invoke("agent_choices");if(generation!==openingGeneration)return;for(const choice of choices)agentLabels.set(choice.id,choice.label);
    $("agent-choice").replaceChildren(...choices.map(choice=>{const option=document.createElement("option");option.value=choice.id;option.textContent=choice.label;return option;}));
    const current=sessions.find(session=>session.address.session===selected?.session)?.conversation?.provider;
    if(choices.some(choice=>choice.id===current))$("agent-choice").value=current;
    $("session-picker-open").disabled=choices.length===0;
    if(!choices.length)throw new Error("No agents are available. Set up an agent and try again.");
    $("agent-choice").focus();
  }catch(error){$("session-picker-error").textContent=String(error);$("session-picker-error").hidden=false;}
}
$("session-picker-cancel").onclick=()=>{++openingGeneration;$("session-picker").close();};
$("session-picker").oncancel=()=>{++openingGeneration;};
$("agent-choice").onchange=()=>{$("initial-model-control").hidden=true;$("initial-model").replaceChildren();};
$("choose-initial-model").onclick=async()=>{
  const generation=openingGeneration,provider=$("agent-choice").value;
  $("choose-initial-model").disabled=true;$("session-picker-open").disabled=true;$("session-picker-error").hidden=true;
  try {
    const models=await invoke("model_choices",{directory:newWorkspace,provider});
    if(generation!==openingGeneration||provider!==$("agent-choice").value)return;
    $("initial-model").replaceChildren(...models.map(model=>{const option=document.createElement("option");option.value=model.id;option.textContent=model.name;return option;}));
    $("initial-model-control").hidden=!models.length;
    if(!models.length)throw new Error("This agent offers no available model choices for this project.");
    $("initial-model").focus();
  }catch(error){if(generation===openingGeneration){$("session-picker-error").textContent=String(error);$("session-picker-error").hidden=false;}}
  finally{if(generation===openingGeneration){$("choose-initial-model").disabled=false;$("session-picker-open").disabled=false;if($("initial-model-control").hidden)$("choose-initial-model").focus();}}
};
$("session-form").onsubmit=async event=>{
  event.preventDefault();$("session-picker-open").disabled=true;
  try {
    const address=await invoke("open_session",{directory:newWorkspace,choice:{kind:"new",provider:$("agent-choice").value,model:$("initial-model-control").hidden?null:$("initial-model").value}});
    await refresh();await select(address);$("session-picker").close();$("draft").focus();
  }catch(error){$("session-picker-error").textContent=String(error);$("session-picker-error").hidden=false;}
  finally{$("session-picker-open").disabled=false;if($("session-picker").open)$("session-picker-open").focus();}
};
async function showHistory() {
  $("history-error").hidden=true;$("history-summary").hidden=true;$("history-list").replaceChildren(text("p","Reading saved conversations…"));
  $("history-picker").showModal();
  try {
    const saved=await invoke("saved_conversations");
    $("history-list").replaceChildren(...saved.map(record=>{
      const name=record.name??("Conversation in "+record.workspace.split(/[\\/]/).filter(Boolean).at(-1));
      const item=button(name,()=>{
        chosenHistory=record;$("history-summary").hidden=false;$("history-name").textContent=name;
        const turns=record.turns==="unknown"?"Turns not reported":record.turns+(record.turns==="1"?" turn":" turns");
        const cost=record.cost==="unknown"?"Spending not reported":record.cost;
        $("history-details").textContent=[record.provider,new Date(record.ended_at).toLocaleString(),turns,cost].join(" · ")+"\n"+record.workspace;
        $("history-text").textContent=record.summary??"No reply summary was saved.";
        $("history-resume-reason").textContent=record.resume_reason??"Resume with the original agent and conversation.";
        $("history-resume").disabled=!!record.resume_reason;
        for(const row of $("history-list").children)row.setAttribute("aria-current",String(row===item));
        $("history-summary").focus();
      });
      item.setAttribute("aria-label",name);item.append(text("small",record.provider+" · "+new Date(record.ended_at).toLocaleDateString()));return item;
    }));
    if(!saved.length)$("history-list").append(text("p","No saved conversations yet."));
  }catch(error){$("history-error").textContent=String(error);$("history-error").hidden=false;$("history-list").replaceChildren();}
}
$("saved-conversations").onclick=()=>showHistory().catch(showError);
$("history-close").onclick=()=>$("history-picker").close();
$("history-resume").onclick=async()=>{
  if(!chosenHistory)return;const record=chosenHistory;$("history-resume").disabled=true;$("history-error").hidden=true;
  try {
    const resident=sessions.find(session=>session.conversation?.id===record.id);
    const address=resident?.address??await invoke("open_session",{directory:record.workspace,choice:{kind:"resume",id:record.id}});
    await refresh();await select(address);$("history-picker").close();$("draft").focus();
  }catch(error){$("history-error").textContent=String(error);$("history-error").hidden=false;$("history-resume").disabled=!!record.resume_reason;$("history-resume").focus();}
};
document.addEventListener("keydown",event=>{
  if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==="o"){event.preventDefault();$("open-project").click();}
});
try {
  const info=await refresh();await connectCatalog();await connectFileDrops();
  document.documentElement.dataset.reducedMotion=String(info.reduce_motion||matchMedia("(prefers-reduced-motion: reduce)").matches);
  if(info.directory){showPicker(info.directory);await projectOpening.choose(info.directory);}
  else {showWelcome();$("welcome-open").focus();}
} catch(error){showError(error);}
