import { ConversationSwitcher } from "./switcher.mjs";
import { KeyboardNavigation } from "./keyboard.mjs";
import { EditorPlaces } from "./reading.mjs";
import { ConversationPicker } from "./conversation_picker.mjs";
import { HistoryBrowser } from "./history.mjs";
import { ConversationName } from "./conversation_actions.mjs";
import { ProjectNavigation } from "./navigation.mjs";
import { WorkspaceLayout } from "./layout.mjs";
import { WorkPanel } from "./work.mjs";
import { addressKey, targetKey, sameTarget } from "./context.mjs";
import { Transcript } from "./transcript.mjs";
import { Drafts } from "./drafts.mjs";
import { Preferences, effectiveMotion } from "./preferences.mjs";
import { SavedDraftEditor } from "./saved_drafts.mjs";
import { Submissions, shouldSaveDraft, reconcileDraft, conversationOwner } from "./session_controls.mjs";
import { attachComposer } from "./composer.mjs";
import { AttachmentList, clipboardFiles, sizeLabel } from "./attachments.mjs";
import { Selection } from "./selection.mjs";
import { ProjectPicker } from "./project_picker.mjs";
import { Catalog, usagePresentation, capabilityText } from "./supervision.mjs";
const {invoke,Channel}=window.__TAURI__.core;
const $=id=>document.getElementById(id);
let selected=null,view=null,sessions=[],projects=[],composerFrame=null,attachmentLimits=null;
const attachmentWork=new Map(),draftMutations=new Set();
const submissions=new Submissions();
const drafts=new Drafts((target,expectedRevision,text)=>invoke("save_draft",{target,expectedRevision,text}),()=>renderDraftStatus());
const savedDraftEditor=new SavedDraftEditor($,invoke,id=>conversationOwner(id,sessions));
const history=new HistoryBrowser($,invoke,{drafts:savedDraftEditor,sessions:()=>sessions,resume:async record=>{
  const resident=conversationOwner(record.id,sessions);
  const address=resident?.address??await invoke("open_session",{directory:record.workspace,choice:{kind:"resume",id:record.id}});
  await refresh();await select(address);
}});
let forcedMotion=false;
const systemMotion=matchMedia("(prefers-reduced-motion: reduce)");
const preferences=new Preferences(invoke,()=>renderPreferences());
systemMotion.addEventListener("change",()=>renderPreferences());
const selection=new Selection();
const editorPlaces=new EditorPlaces($("draft"));
const layout=new WorkspaceLayout({root:$("workspace"),conversation:$("conversation"),detail:$("session-detail"),toggle:$("toggle-details"),heading:$("detail-title"),compact:matchMedia("(max-width:1100px)")});
const catalog=new Catalog();
const attentionNodes=new Map();
let announcementKey="";
const transcriptView=new Transcript($("transcript"),$("latest"),{
  copy:value=>navigator.clipboard.writeText(value).catch(error=>showError("Could not copy the text. "+error)),
  openLink:destination=>invoke("open_link",{destination}).catch(showError)
});
let configurationKey="";
let requestsKey="",railKey="";
const labels=new Map();let issuedLabel=0;
const showError=error=>{$("error").textContent=error?.message??String(error);$("error").hidden=false;};
const workPanel=new WorkPanel($,invoke,showError,renderWorkSummary);
const conversationName=new ConversationName($,invoke);
const clearError=()=>{$("error").hidden=true;};
const button=(label,click)=>{const element=document.createElement("button");element.textContent=label;element.onclick=click;return element;};
const text=(tag,value)=>{const element=document.createElement(tag);element.textContent=value;return element;};
const attachmentList=new AttachmentList($("pending-attachments"),id=>{
  const target=currentTarget();
  return changeAttachments(target,()=>invoke("remove_attachment",{target,id}));
},()=>$("add-files").focus());

const navigation=new ProjectNavigation($("projects"),{
  select:address=>select(address).catch(showError),open:openDirectory,act,close:closeAddress,
  label:session=>session.conversation?.name??("Conversation "+labels.get(session.address.session))
});
const switcher=new ConversationSwitcher($,{sessions:()=>sessions,selected:()=>selected,ready:()=>catalog.sequence>=0,select,
  label:session=>session.conversation?.name??("Conversation "+labels.get(session.address.session))});
new KeyboardNavigation($,layout);
$("open-settings").onclick=()=>{if(!document.querySelector("dialog[open]"))$("settings").showModal();};
$("settings-close").onclick=()=>$("settings").close();
$("settings").onclose=()=>$("open-settings").focus();
$("agent-quick-settings").onclick=()=>{layout.show(true,true);$("agent-configuration").querySelector("select")?.focus();};
$("close-details").onclick=()=>{layout.show(false);$("toggle-details").focus();};
$("copy-summary").onclick=()=>{layout.show(true);$("working-copy").focus();};
$("work-summary").onclick=()=>{layout.show(true);$("work-title").focus();};
$("review-requests").onclick=()=>{layout.show(true);$("requests").querySelector(".request")?.focus();};
$("conversation-actions").addEventListener("click",event=>{if(event.target.closest("button"))$("conversation-actions").open=false;});
$("conversation-actions").addEventListener("keydown",event=>{
  if(event.key==="Escape"&&!event.isComposing){event.preventDefault();$("conversation-actions").open=false;$("conversation-menu-trigger").focus();}
});
document.addEventListener("pointerdown",event=>{
  if(!$("conversation-actions").contains(event.target))$("conversation-actions").open=false;
});
function renderWorkSummary(presentation,frame) {
  const summary=$("work-summary");
  summary.disabled=!frame?.target;
  summary.textContent=presentation.error?"Work needs attention":frame?.work||presentation.status==="Requesting work…"?presentation.status.replace(/[.]$/,""):"Planned work";
  summary.title=presentation.detail;
  summary.dataset.active=String(presentation.active||presentation.status==="Requesting work…");
}

async function refresh() {
  const info=await invoke("overview");if(catalog.sequence<0)sessions=info.sessions;projects=info.projects;
  attachmentLimits=info.attachment_limits;
  for(const session of sessions)if(!labels.has(session.address.session))labels.set(session.address.session,++issuedLabel);
  renderRail();
  $("welcome-projects").replaceChildren(...projects.slice(0,4).map(project=>{
    const item=button("",()=>openDirectory(project.directory));
    item.append(text("strong",project.name),text("span",project.directory),text("small","Open project →"));return item;
  }));
  return info;
}
function renderRail() {
  const signature=JSON.stringify([projects,sessions,selected]);if(signature===railKey)return;railKey=signature;
  navigation.render(projects,sessions,selected);
  $("sessions-empty").hidden=projects.length>0||sessions.length>0;
  if(selected)$("project-name").textContent=sessions.find(session=>session.address.session===selected.session)?.name??"Project";
  for(const id of labels.keys())if(!sessions.some(session=>session.address.session===id))labels.delete(id);
  const waiting=sessions.flatMap(session=>session.pending.map(request=>({session,request})));
  const waitingRows=waiting.map(({session,request})=>{
    const key=JSON.stringify([session.target,request.id]);
    let item=attentionNodes.get(key);
    if(!item){
      item=button("",()=>select(session.address,request.id,session.target).catch(showError));
      item.append(text("strong",""),text("span",""));attentionNodes.set(key,item);
    }
    const conversation=session.conversation?.name??(session.name+" session "+labels.get(session.address.session));
    item.children[0].textContent=conversation;item.children[1].textContent=request.title;
    item.setAttribute("aria-label",conversation+": "+request.title);
    return item;
  });
  placeChildren($("attention"),waitingRows);
  for(const [key,item] of attentionNodes)if(!waitingRows.includes(item))attentionNodes.delete(key);
  const failed=sessions.filter(session=>session.status==="Connection lost").length;
  $("attention-dock").hidden=waiting.length===0&&failed===0;
  $("attention-total").textContent=String(waiting.length+failed);
  $("attention-disconnected").hidden=!failed;
  $("attention-disconnected").textContent=failed+(failed===1?" agent disconnected":" agents disconnected");
  const announcement=waiting.length+(waiting.length===1?" request waiting":" requests waiting")+(failed?" · "+failed+(failed===1?" agent disconnected":" agents disconnected"):"");
  if(announcement!==announcementKey){$("attention-count").textContent=announcement;announcementKey=announcement;}
}
function placeChildren(parent,children) {
  children.forEach((element,index)=>{if(parent.children[index]!==element)parent.insertBefore(element,parent.children[index]??null);});
  while(parent.children.length>children.length)parent.lastChild.remove();
}
const currentTarget=()=>composerFrame?.target??null;
const currentIs=target=>sameTarget(currentTarget(),target);
function contextError(target,error) {if(currentIs(target))showError(error);}
async function act(target,action) {
  if(!target)return;
  try{await invoke("session_action",{target,action:{action}});}catch(error){showError(error);}
}
async function connectCatalog() {
  const channel=new Channel();let early=null,ready=false;
  const receive=frame=>{
    if(!catalog.apply(frame))return;
    sessions=catalog.sessions;
    const targets=new Set(sessions.filter(session=>!session.closed).flatMap(session=>session.owned_conversations.map(conversation=>targetKey({address:session.address,conversation}))));
    transcriptView.retain(targets);editorPlaces.retain(targets);
    for(const session of sessions)if(!labels.has(session.address.session))labels.set(session.address.session,++issuedLabel);
    renderRail();savedDraftEditor.render();history.update();switcher.update();
    invoke("acknowledge",{stream:frame.stream,sequence:frame.sequence}).catch(showError);
  };
  channel.onmessage=frame=>ready?receive(frame):early=frame;
  catalog.open(await invoke("subscribe_catalog",{channel}));ready=true;
  if(early)receive(early);

}
function renderComposer() {
  if(!composerFrame)return;
  const frame=composerFrame,key=targetKey(frame.target),fileBusy=attachmentWork.has(key),busy=fileBusy||draftMutations.has(key)||draftMutations.has(addressKey(frame.address));
  const closing=draftMutations.has(addressKey(frame.address));
  $("close-session").disabled=frame.closed||closing;
  $("close-session").textContent=closing?"Closing…":"Close conversation";
  $("draft").disabled=!frame.target||frame.closed||draftMutations.has(key)||closing;
  $("send").disabled=!frame.target||busy||submissions.pending(frame.target)||frame.closed||frame.activity?.cancellation_requested;
  $("add-files").disabled=!frame.target||busy||frame.closed;
  attachmentList.apply(frame.attachments, !frame.target||busy||frame.closed);
  renderDraftStatus();
  const images=attachmentList.files.some(file=>file.mime_type);
  const note=fileBusy?"Updating files…":images&&frame.activity?.can_send_images===false?
    "This agent cannot receive images. Remove the image before sending.":
    images&&frame.activity?.can_send_images!==true?"Image support has not been reported. Your files stay here if the agent cannot receive them.":
    "";
  if($("attachment-note").textContent!==note)$("attachment-note").textContent=note;
  $("add-files").title="Add text files or PNG, JPEG and WebP images"+(attachmentLimits?" · "+sizeLabel(attachmentLimits.file_bytes)+" per file.":"");
}
async function changeAttachments(target,operation) {
  if(!target)return;
  const key=targetKey(target);
  if(draftMutations.has(key)||draftMutations.has(addressKey(target.address)))return;
  if(attachmentWork.has(key)){showError("Wait for the current file selection to finish.");return;}
  const work=Promise.resolve().then(operation);
  attachmentWork.set(key,{target,promise:work});renderComposer();clearError();
  try{await work;}catch(error){contextError(target,error);}
  finally{attachmentWork.delete(key);renderComposer();}
}
async function connectFileDrops() {
  const channel=new Channel();
  channel.onmessage=notice=>{
    if(notice.kind==="error"){showError(notice.message);return;}
    if(!currentIs(notice.target)||selection.current?.stream!==notice.stream||document.querySelector("dialog[open]")) {
      showError("The selected conversation changed. Drop the files again.");return;
    }
    const target=notice.target;
    changeAttachments(target,()=>invoke("attach_dropped_files",{target,stream:notice.stream,selection:notice.selection}));
  };
  await invoke("subscribe_file_drops",{channel});
}
function renderPreferences() {
  const value=preferences.value;
  $("theme").disabled=!value;$("reduce-motion").disabled=!value;
  if(value) {
    $("theme").value=value.theme;$("reduce-motion").checked=value.reduce_motion;
    if(value.theme==="system")delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme=value.theme;
  }
  document.documentElement.dataset.reducedMotion=String(effectiveMotion(forcedMotion,value?.reduce_motion,systemMotion.matches));
  $("preferences-note").textContent=preferences.error?preferences.error:
    preferences.running?"Saving preferences…":forcedMotion?"Reduced motion is enabled for this launch.":"";
  $("retry-preferences").hidden=!preferences.error;
}
function renderDraftStatus() {
  const target=currentTarget();
  if(!target){$("draft-status").textContent="";$("retry-draft").hidden=true;$("discard-draft").disabled=true;return;}
  const status=drafts.status(target);
  $("draft-status").textContent=status.error?status.error:status.saving?"Saving draft…":"Draft saved";
  $("retry-draft").hidden=!status.error;
  $("discard-draft").disabled=!composerFrame||composerFrame.closed||submissions.pending(target)||attachmentWork.has(targetKey(target))||draftMutations.has(targetKey(target))||draftMutations.has(addressKey(selected))||(!$("draft").value&&!attachmentList.files.length);
}
$("retry-draft").onclick=()=>{clearError();saveDraft().catch(showError);};
$("discard-draft").onclick=async()=>{
  const target=currentTarget();if(!target)return;
  const key=targetKey(target);draftMutations.add(key);renderComposer();
  try {
    await drafts.flush(target);
    const status=await invoke("discard_draft",{target,expectedRevision:drafts.status(target).revision});
    const applied=drafts.accept(target,status,"");
    if(status.error)throw new Error(status.error);
    if(applied&&currentIs(target))$("draft").value="";
  }catch(error){contextError(target,error);}
  finally{draftMutations.delete(key);if(currentIs(target))renderComposer();}
};
async function connectExit() {
  const channel=new Channel();let exiting=false;
  channel.onmessage=async()=>{
    if(exiting)return;exiting=true;
    const frozen=[document.body,...document.querySelectorAll("dialog[open]")].map(element=>({element,inert:element.inert}));
    for(const {element} of frozen)element.inert=true;
    try {
      await Promise.all([...attachmentWork.values()].map(entry=>entry.promise));
      await saveDraft();await drafts.flushAll();await savedDraftEditor.flush();await preferences.flush();
      await invoke("finish_exit");
    }catch(error){for(const {element,inert} of frozen)element.inert=inert;exiting=false;showError("Could not close because changes are not saved. "+(error?.message??String(error)));}
  };
  await invoke("subscribe_exit",{channel});
}
async function saveDraft(target=currentTarget(),input=attachmentList.input($("draft").value)) {
  if(!shouldSaveDraft(target,input,sessions,submissions))return;
  drafts.edit(target,input.text);
  await drafts.flush(target);
}
async function select(address,requestId=null,requestTarget=null) {
  const previous=currentTarget(),input=attachmentList.input($("draft").value);
  const accepted=await selection.open(address,{
    save:()=>previous?saveDraft(previous,input):Promise.resolve(),
    subscribe:async(address,intent,receive)=>{
      const channel=new Channel();channel.onmessage=receive;
      return invoke("subscribe_session",{address,channel,selection:intent});
    },
    commit:(candidate,initial)=>{
      const previous=currentTarget();
      if(previous)editorPlaces.capture(previous,drafts.status(previous).revision);
      transcriptView.clear();
      selected=address;view=candidate;candidate.requestId=requestId;candidate.requestTarget=requestTarget;requestsKey="";railKey="";configurationKey="";
      $("welcome").hidden=true;$("workspace").hidden=false;$("view-title").textContent="Conversation";
      layout.enable(true);if(layout.compact.matches)layout.show(false);
      $("draft").value="";composerFrame=null;workPanel.render(null);attachmentList.clear();
      $("draft").disabled=true;$("send").disabled=true;$("add-files").disabled=true;$("attachment-note").textContent="";
      $("workspace-path").textContent=address.workspace;
      $("copy-summary").textContent="Working copy";
      $("copy-summary").title=address.workspace;
      $("conversation-toolbar").hidden=false;$("conversation-actions").hidden=false;$("conversation-actions").open=false;
      $("project-name").textContent=sessions.find(session=>session.address.session===address.session)?.name??"Project";
      $("status").textContent="Connecting";renderRail();navigation.reveal(address);clearError();
      if(initial)receiveFrame(candidate,initial);
    },
    receive:receiveFrame
  });
  if(!accepted)return;
  const copies=await invoke("working_copies",{directory:address.workspace});
  if(selected?.session!==address.session)return;
  const activeCopy=copies.find(copy=>copy.path===address.workspace);
  $("copy-summary").textContent=activeCopy?.state.status==="unversioned"?"Project folder":activeCopy?.branch??"Detached working copy";
  $("copy-summary").setAttribute("aria-label","Working copy: "+$("copy-summary").textContent);
  $("working-copy").replaceChildren(...copies.map(copy=>{
    const option=document.createElement("option");option.value=copy.path;
    option.textContent=(copy.state.status==="unversioned"?"Project folder":copy.branch??"Detached working copy")+(copy.is_main?" · Main":"");
    option.selected=copy.path===address.workspace;
    option.disabled=!["available","archived","unversioned"].includes(copy.state.status);return option;
  }));
}
function receiveFrame(candidate,frame) {
  const previousSequence=candidate.sequence,previousTarget=candidate.target;
  const changing=previousSequence<0||targetKey(previousTarget)!==targetKey(frame.target);
  if(changing&&currentTarget())editorPlaces.capture(currentTarget(),drafts.status(currentTarget()).revision);
  if(candidate.apply(frame)) {
    drafts.observe(frame);
    if(previousSequence<0||targetKey(previousTarget)!==targetKey(frame.target)) {
      $("draft").value=drafts.restore(frame);
      attachmentList.clear();requestsKey="";configurationKey="";
      clearError();
    }
    const accepted=submissions.observe(frame);
    const draft=reconcileDraft(frame,attachmentList.input($("draft").value),accepted,previousSequence);
    if(draft!==$("draft").value)$("draft").value=draft;
    render(frame);
    if(changing)editorPlaces.restore(frame.target,drafts.status(frame.target).revision);
  }
  invoke("acknowledge",{stream:frame.stream,sequence:frame.sequence}).catch(()=>{});
}
function render(frame) {
  transcriptView.render(view);
  if($("status").textContent!==frame.status)$("status").textContent=frame.status;
  $("status").dataset.state=frame.status==="Connection lost"?"disconnected":view.pending.length?"waiting":frame.activity?.active?"running":"ready";
  $("stop").disabled=frame.closed||frame.activity?.cancellation_requested||!frame.activity?.active;
  $("stop").hidden=!frame.activity?.active;
  $("agent-quick-settings").disabled=frame.closed;
  $("agent-quick-settings").textContent=frame.model??frame.conversation?.provider_label??"Agent settings";
  composerFrame=frame;renderComposer();workPanel.render(frame);
  $("new-session").disabled=frame.closed;
  $("copy").disabled=view.messages.length===0;
  $("model").textContent=frame.model??"Model information is unavailable.";
  renderConfiguration(frame);
  const usage=usagePresentation(frame.usage);
  $("usage").textContent=usage.context;$("context-value").textContent=usage.percent;$("spending-value").textContent=usage.spending;
  const meter=$("context-meter"),known=frame.usage?.used!=null&&frame.usage?.size>0;
  meter.hidden=!known;
  if(known){meter.max=frame.usage.size;meter.value=frame.usage.used;meter.setAttribute("aria-valuetext",frame.usage.used.toLocaleString()+" of "+frame.usage.size.toLocaleString()+" context tokens");}
  $("model").hidden=(frame.configuration??[]).some(option=>option.id==="model");
  $("queue").hidden=!frame.activity?.queued_turns&&!frame.activity?.cancellation_requested;
  $("review-requests").hidden=!view.pending.length;
  $("review-requests").textContent=view.pending.length+(view.pending.length===1?" request needs your answer":" requests need your answer")+" →";
  $("capabilities").textContent=capabilityText(frame.activity);
  $("queue").textContent=frame.activity?frame.activity.queued_turns+(frame.activity.queued_turns===1?" message queued":" messages queued")+(frame.activity.cancellation_requested?" · Waiting for the agent to stop.":""):"Queue information is unavailable.";
  $("reconnect").disabled=frame.closed||frame.status!=="Connection lost"||frame.activity?.can_resume!==true;
  $("reconnect").hidden=$("reconnect").disabled;
  const pendingKey=JSON.stringify([frame.target,view.pending]);
  if(pendingKey!==requestsKey) {$("requests").replaceChildren(...view.pending.map(request=>{
    const section=document.createElement("section");section.className="request";section.dataset.request=String(request.id);section.tabIndex=-1;section.setAttribute("aria-label","Request: "+request.title);
    const heading=text("h2","Waiting for you");heading.dataset.leadingIcon="shield";
    section.append(heading,text("h3",request.title));
    if(request.detail)section.append(text("pre",request.detail));
    const actions=text("div","");actions.className="request-actions";
    for(const choice of request.choices) actions.append(button(choice.label,async event=>{
      const target=frame.target;const trigger=event.currentTarget;
      trigger.disabled=true;
      try {await invoke("session_action",{target,action:{action:"answer",id:request.id,option_id:choice.option_id}});}
      catch(error){trigger.disabled=false;contextError(target,error);}
    }));
    section.append(actions);return section;
  }));requestsKey=pendingKey;}
  if(view.requestId!==null) {
    const target=sameTarget(frame.target,view.requestTarget)&&$("requests").querySelector('[data-request="'+view.requestId+'"]');
    if(target){layout.show(true);target.focus();}
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
  $("workspace").hidden=true;$("welcome").hidden=false;layout.enable(false);
  $("conversation-toolbar").hidden=true;$("conversation-actions").hidden=true;$("conversation-actions").open=false;conversationName.render(null);workPanel.render(null);
  $("project-name").textContent="Magistr";$("view-title").textContent="Projects";$("status").textContent="Ready";$("status").dataset.state="ready";
  $("draft").value="";$("requests").replaceChildren();transcriptView.clear();
  for(const id of ["draft","send","stop","new-session","close-session","copy","reconnect","add-files"])$(id).disabled=true;
  $("welcome-note").textContent=note;$("welcome-note").hidden=!note;
  renderRail();
}
const projectPicker=new ProjectPicker(invoke,$,{
  open:directory=>openSession(directory),
  opened:async address=>{await refresh();await select(address);},
  closed:()=>{if(!selected)showWelcome();}
});
const showPicker=directory=>projectPicker.show(directory);
const projectOpening=projectPicker.flow;
$("add-files").onclick=()=>{
  const target=currentTarget();
  changeAttachments(target,()=>invoke("pick_attachments",{target}));
};
$("draft").addEventListener("paste",event=>{
  try {
    const files=clipboardFiles(event,attachmentLimits,attachmentList.files);
    if(!files.length||!currentTarget())return;
    const target=currentTarget();
    changeAttachments(target,async()=>{
      for(const file of files) {
        const bytes=new Uint8Array(await file.arrayBuffer());
        await invoke("attach_clipboard_file",{target,name:file.name||"Pasted image",bytes:Array.from(bytes)});
      }
    });
  }catch(error){showError(error);}
});
// The native host owns file drops; prevent a dropped URL from navigating the page.
document.addEventListener("dragover",event=>event.preventDefault());
document.addEventListener("drop",event=>event.preventDefault());
attachComposer($("draft"),$("composer"));
$("composer").onsubmit=async event=>{
  event.preventDefault();const target=currentTarget();if(!target||submissions.pending(target)||attachmentWork.has(targetKey(target))||draftMutations.has(targetKey(target))||draftMutations.has(addressKey(selected)))return;
  const input=attachmentList.input($("draft").value);
  if(!input.text.trim()&&!input.attachments.length)return;
  submissions.begin(target,input,view.sequence);$("send").disabled=true;clearError();
  try {
    drafts.edit(target,input.text);
    await drafts.flush(target);
    await invoke("session_action",{target,action:{action:"send",text:input.text,attachment_ids:input.attachments.map(file=>file.id)}});
  } catch(error){submissions.fail(target);contextError(target,error);if(currentIs(target))renderComposer();}
};
$("stop").onclick=()=>act(currentTarget(),"cancel");
$("reconnect").onclick=()=>act(currentTarget(),"reconnect");
$("draft").oninput=()=>{const target=currentTarget();if(shouldSaveDraft(target,attachmentList.input($("draft").value),sessions,submissions))drafts.edit(target,$("draft").value);};

$("new-session").onclick=()=>selected&&chooseAgent(selected.workspace).catch(showError);
$("working-copy").onchange=event=>openKnownWorkspace(event.target.value);
$("open-project").onclick=()=>showPicker();
$("welcome-path").onclick=()=>showPicker();
$("welcome-open").onclick=()=>{showPicker();projectPicker.browse();};
$("theme").onchange=$("reduce-motion").onchange=()=>preferences.change({theme:$("theme").value,reduce_motion:$("reduce-motion").checked});
$("retry-preferences").onclick=()=>preferences.retry();
$("copy").onclick=async()=>{
  const value=view?.copy()??"";
  try{await navigator.clipboard.writeText(value);}catch(error){showError("Could not copy the conversation. "+error);}
};
async function closeAddress(address) {
  const key=addressKey(address);if(draftMutations.has(key))return;
  draftMutations.add(key);if(selected?.session===address.session)renderComposer();
  try {
    const target=currentTarget(),input=attachmentList.input($("draft").value),wasSelected=addressKey(selected)===key;
    await Promise.all([...attachmentWork.values()].filter(entry=>addressKey(entry.target.address)===key).map(entry=>entry.promise));
    if(wasSelected)await saveDraft(target,input);
    await drafts.flushSession(address);
    await invoke("close_session",{address});
    drafts.forgetSession(address);submissions.forgetSession(address);
    const info=await refresh();await connectCatalog();sessions=info.sessions;renderRail();
    if(selected?.session!==address.session)return;
    const next=sessions.find(session=>!session.closed);
    if(next)await select(next.address);
    else showWelcome("Your session has closed. Open a project to start another.");
  }catch(error){showError(error);}
  finally{draftMutations.delete(key);if(selected?.session===address.session)renderComposer();}
}
$("close-session").onclick=()=>selected&&closeAddress(selected);

function renderConfiguration(frame) {
  $("view-title").textContent=frame.conversation?.name??(frame.conversation?.purpose==="roadmap"?"Planning":"Conversation");
  conversationName.render(frame);
  $("agent-provider").textContent=frame.conversation?.provider_label??"Agent information is unavailable.";
  const key=JSON.stringify([frame.target,frame.configuration]);
  if(key!==configurationKey) {
    $("agent-configuration").replaceChildren(...(frame.configuration??[]).map(option=>{
      const label=document.createElement("label");label.append(text("span",option.name));
      const select=document.createElement("select");select.setAttribute("aria-label",option.name);
      for(const [value,name] of option.values){const choice=document.createElement("option");choice.value=value;choice.textContent=name;choice.selected=value===option.current_value;select.append(choice);}
      if(!option.values.some(([value])=>value===option.current_value)){const unavailable=document.createElement("option");unavailable.textContent="Current choice unavailable";unavailable.disabled=true;unavailable.selected=true;select.append(unavailable);}
      select.onchange=async()=>{
        const value=select.value;select.value=option.current_value;select.disabled=true;
        try{await invoke("session_action",{target:frame.target,action:{action:"configure",id:option.id,value}});}
        catch(error){contextError(frame.target,error);select.disabled=false;}
      };
      label.append(select);return label;
    }));configurationKey=key;
  }
  for(const select of $("agent-configuration").querySelectorAll("select"))select.disabled=frame.closed||frame.activity?.active||!!frame.activity?.queued_turns||frame.activity?.cancellation_requested||frame.activity?.configuration_in_progress;
}
const agentLabels=new Map();
const conversationPicker=new ConversationPicker(invoke,$,{
  opened:async address=>{await refresh();await select(address);},
  rememberAgents:choices=>{for(const choice of choices)agentLabels.set(choice.id,choice.label);}
});
const chooseAgent=directory=>conversationPicker.show({
  directory,name:sessions.find(session=>addressKey(session.address)===addressKey(selected))?.name??"Project",
  provider:sessions.find(session=>addressKey(session.address)===addressKey(selected))?.conversation?.provider
});
try {
  const info=await refresh();await connectCatalog();await connectFileDrops();
  forcedMotion=info.reduce_motion;await preferences.load();
  await connectExit();
  if(info.directory){showPicker(info.directory);await projectOpening.choose(info.directory);}
  else {showWelcome();$("welcome-open").focus();}
} catch(error){showError(error);}
