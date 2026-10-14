import { attachComposer } from "./composer.mjs";
import { Selection } from "./selection.mjs";
import { ProjectOpening } from "./projects.mjs";
import { Catalog, usageText, capabilityText } from "./supervision.mjs";
const {invoke,Channel}=window.__TAURI__.core;
const $=id=>document.getElementById(id);
let selected=null,view=null,sessions=[],projects=[],sending=false;
const selection=new Selection();
const catalog=new Catalog();
const railNodes=new Map(),attentionNodes=new Map();
let announcementKey="";
const messageNodes=new WeakMap();
let requestsKey="",railKey="";
const labels=new Map();let issuedLabel=0;
const showError=error=>{$("error").textContent=String(error);$("error").hidden=false;};
const clearError=()=>{$("error").hidden=true;};
const button=(label,click)=>{const element=document.createElement("button");element.textContent=label;element.onclick=click;return element;};
const text=(tag,value)=>{const element=document.createElement(tag);element.textContent=value;return element;};

async function refresh() {
  const info=await invoke("overview");if(catalog.sequence<0)sessions=info.sessions;projects=info.projects;
  for(const session of sessions)if(!labels.has(session.address.session))labels.set(session.address.session,++issuedLabel);
  renderRail();return info;
}
function renderRail() {
  const signature=JSON.stringify([projects,sessions,selected]);if(signature===railKey)return;railKey=signature;
  $("projects").replaceChildren(...projects.map(project=>button(project.name,()=>openDirectory(project.directory))));
  const rows=sessions.map(session=>{
    const id=session.address.session,index=labels.get(id),name=session.name+" session "+index;
    let row=railNodes.get(id);
    if(!row) {
      row=document.createElement("div");row.className="session-row";
      const item=button("",()=>select(session.address).catch(showError));item.append(text("span",""),text("small",""));
      const path=text("small","Working copy: "+session.address.workspace.split(/[\\/]/).filter(Boolean).at(-1));path.className="path";path.title=session.address.workspace;path.setAttribute("aria-label","Working copy "+session.address.workspace);
      const actions=document.createElement("div");actions.className="rail-actions";
      actions.append(button("Stop",()=>act(session.address,"cancel")),button("Reconnect",()=>act(session.address,"reconnect")),button("Close",()=>closeAddress(session.address)));
      row.append(item,path,actions);railNodes.set(id,row);
    }
    const item=row.children[0];item.children[0].textContent=session.name+" · "+index;
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
    item.textContent=session.name+" session "+labels.get(session.address.session)+": "+request.title;
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
async function saveDraft(address=selected,value=$("draft").value) {
  if(!address)return;
  await invoke("save_draft",{address,text:value});
}
async function select(address,requestId=null) {
  const previous=selected,value=$("draft").value;
  const accepted=await selection.open(address,{
    save:()=>previous?saveDraft(previous,value):Promise.resolve(),
    subscribe:async(address,intent,receive)=>{
      const channel=new Channel();channel.onmessage=receive;
      return invoke("subscribe_session",{address,channel,selection:intent});
    },
    commit:(candidate,initial)=>{
      selected=address;view=candidate;candidate.requestId=requestId;requestsKey="";railKey="";
      $("welcome").hidden=true;$("workspace").hidden=false;$("view-title").textContent="Conversation";
      $("draft").value="";
      $("draft").disabled=true;$("send").disabled=true;
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
  const initial=candidate.sequence<0;
  if(candidate.apply(frame)) {
    if(initial) $("draft").value=frame.draft;
    if(frame.draft && !$("draft").value && frame.events.some(event=>event.kind==="error")) $("draft").value=frame.draft;
    render(frame);
  }
  invoke("acknowledge",{stream:frame.stream,sequence:frame.sequence}).catch(()=>{});
}
function render(frame) {
  const transcript=$("transcript");
  const follow=transcript.scrollHeight-transcript.scrollTop-transcript.clientHeight<100;
  const elements=[];
  if(view.dropped) {const gap=text("p","Some earlier messages are unavailable.");gap.className="gap";elements.push(gap);}
  if(view.messages.length===0) {const empty=document.createElement("div");empty.className="empty";empty.append(text("h2","What would you like to work on?"),text("p","Send a message to start a conversation in this working copy."));elements.push(empty);}
  for(const message of view.messages) {
    let article=messageNodes.get(message);
    if(!article) {article=document.createElement("article");article.className=message.role.toLowerCase();article.append(text("h2",message.role),text("p",message.text));messageNodes.set(message,article);}
    else if(article.lastChild.textContent!==message.text)article.lastChild.textContent=message.text;
    elements.push(article);
  }
  elements.forEach((element,index)=>{if(transcript.children[index]!==element)transcript.insertBefore(element,transcript.children[index]??null);});
  while(transcript.children.length>elements.length)transcript.lastChild.remove();
  if(follow) transcript.scrollTop=transcript.scrollHeight;
  $("latest").hidden=follow;
  if($("status").textContent!==frame.status)$("status").textContent=frame.status;
  $("stop").disabled=!["Working","Waiting for you"].includes(frame.status);
  $("send").disabled=sending || frame.closed;
  $("draft").disabled=frame.closed;
  $("new-session").disabled=frame.closed;$("close-session").disabled=frame.closed;
  $("copy").disabled=view.messages.length===0;
  $("model").textContent=frame.model??"Model information is unavailable.";
  $("usage").textContent=usageText(frame.usage);
  $("capabilities").textContent=capabilityText(frame.activity);
  $("queue").textContent=frame.activity?frame.activity.queued_turns+" messages queued"+(frame.activity.cancellation_requested?" · Waiting for the agent to stop.":""):"Queue information is unavailable.";
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
  return invoke("open_session",{directory});
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
  selected=null;view=null;selection.clear();requestsKey="";railKey="";
  $("workspace").hidden=true;$("welcome").hidden=false;
  $("project-name").textContent="Magistr";$("view-title").textContent="Projects";$("status").textContent="Ready";
  $("draft").value="";$("requests").replaceChildren();$("transcript").replaceChildren();
  for(const id of ["draft","send","stop","new-session","close-session","copy","reconnect"])$(id).disabled=true;
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
attachComposer($("draft"),$("composer"));
$("composer").onsubmit=async event=>{
  event.preventDefault();if(sending||!selected)return;
  const address=selected,value=$("draft").value;if(!value.trim())return;
  sending=true;$("send").disabled=true;clearError();
  try {
    await saveDraft(address,value);
    await invoke("session_action",{address,action:{action:"send",text:value}});
    if(selected?.session===address.session && $("draft").value===value) {$("draft").value="";await saveDraft(address,"");}
  } catch(error){showError(error);}
  finally {sending=false;$("send").disabled=false;}
};
$("stop").onclick=()=>selected&&act(selected,"cancel");
$("reconnect").onclick=()=>selected&&act(selected,"reconnect");
$("draft").onblur=()=>saveDraft().catch(showError);
$("latest").onclick=()=>{$("transcript").scrollTop=$("transcript").scrollHeight;$("latest").hidden=true;};
$("new-session").onclick=()=>selected&&openKnownWorkspace(selected.workspace,true);
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
  const value=view?.messages.map(message=>message.role+"\n"+message.text).join("\n\n")??"";
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
document.addEventListener("keydown",event=>{
  if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==="o"){event.preventDefault();$("open-project").click();}
});
try {
  const info=await refresh();await connectCatalog();
  document.documentElement.dataset.reducedMotion=String(info.reduce_motion||matchMedia("(prefers-reduced-motion: reduce)").matches);
  if(info.directory){showPicker(info.directory);await projectOpening.choose(info.directory);}
  else {showWelcome();$("welcome-open").focus();}
} catch(error){showError(error);}
