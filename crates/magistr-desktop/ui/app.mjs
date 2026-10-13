import { attachComposer } from "./composer.mjs";
import { Selection } from "./selection.mjs";
const {invoke,Channel}=window.__TAURI__.core;
const $=id=>document.getElementById(id);
let selected=null,view=null,sessions=[],projects=[],sending=false;
const selection=new Selection();
const messageNodes=new WeakMap();
let requestsKey="",railKey="";
const labels=new Map();let issuedLabel=0;
const showError=error=>{$("error").textContent=String(error);$("error").hidden=false;};
const clearError=()=>{$("error").hidden=true;};
const button=(label,click)=>{const element=document.createElement("button");element.textContent=label;element.onclick=click;return element;};
const text=(tag,value)=>{const element=document.createElement(tag);element.textContent=value;return element;};

async function refresh() {
  const info=await invoke("overview");sessions=info.sessions;projects=info.projects;
  for(const session of sessions)if(!labels.has(session.address.session))labels.set(session.address.session,++issuedLabel);
  renderRail();return info;
}
function renderRail() {
  const signature=JSON.stringify([projects,sessions,selected]);if(signature===railKey)return;railKey=signature;
  $("projects").replaceChildren(...projects.map(project=>button(project.name,()=>openDirectory(project.directory,false))));
  $("sessions").replaceChildren(...sessions.map(session=>{const index=labels.get(session.address.session)-1;
    const item=button(session.name+" · "+(index+1),()=>select(session.address).catch(showError));
    item.append(text("small",session.status));
    item.setAttribute("aria-current",String(selected?.session===session.address.session));
    item.setAttribute("aria-label",session.name+" session "+(index+1));return item;
  }));
}
async function saveDraft(address=selected,value=$("draft").value) {
  if(!address)return;
  await invoke("save_draft",{address,text:value});
}
async function select(address) {
  const previous=selected,value=$("draft").value;
  const accepted=await selection.open(address,{
    save:()=>previous?saveDraft(previous,value):Promise.resolve(),
    subscribe:async(address,intent,receive)=>{
      const channel=new Channel();channel.onmessage=receive;
      return invoke("subscribe_session",{address,channel,selection:intent});
    },
    commit:(candidate,initial)=>{
      selected=address;view=candidate;requestsKey="";railKey="";
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
    option.textContent=(copy.branch??"Detached working copy")+(copy.is_main?" · Main":"");
    option.selected=copy.path===address.workspace;
    option.disabled=!["available","archived"].includes(copy.state.status);return option;
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
  $("status").textContent=frame.status;
  $("stop").disabled=!["Working","Waiting for you"].includes(frame.status);
  $("send").disabled=sending || frame.closed;
  $("draft").disabled=frame.closed;
  $("new-session").disabled=frame.closed;$("close-session").disabled=frame.closed;
  $("copy").disabled=view.messages.length===0;
  $("model").textContent=frame.model??"Model information is unavailable.";
  $("usage").textContent=frame.usage?(frame.usage.size?frame.usage.used.toLocaleString()+" / "+frame.usage.size.toLocaleString()+" context tokens":"")+(frame.usage.cost_cents===null?"":" · $"+(frame.usage.cost_cents/100).toFixed(2)):"";
  const pendingKey=JSON.stringify([frame.address,view.pending]);
  if(pendingKey!==requestsKey) {$("requests").replaceChildren(...view.pending.map(request=>{
    const section=document.createElement("section");section.className="request";
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
  const session=sessions.find(session=>session.address.session===frame.address.session);
  if(session){session.status=frame.status;renderRail();}
}
async function openDirectory(directory,fresh) {
  try {
    clearError();
    const resident=!fresh && sessions.find(session=>session.address.workspace===directory);
    const address=resident?.address??await invoke("open_session",{directory});
    await refresh();await select(address);$("draft").focus();
  } catch(error){showError(error);}
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
$("stop").onclick=async()=>{try{await invoke("session_action",{address:selected,action:{action:"cancel"}});}catch(error){showError(error);}};
$("draft").onblur=()=>saveDraft().catch(showError);
$("latest").onclick=()=>{$("transcript").scrollTop=$("transcript").scrollHeight;$("latest").hidden=true;};
$("new-session").onclick=()=>selected&&openDirectory(selected.workspace,true);
$("working-copy").onchange=event=>openDirectory(event.target.value,false);
$("open-project").onclick=()=>{$("picker-error").hidden=true;$("project-picker").showModal();$("project-folder").focus();};
$("picker-cancel").onclick=()=>$("project-picker").close();
$("project-form").onsubmit=async event=>{
  event.preventDefault();const control=event.submitter;control.disabled=true;
  try {
    const address=await invoke("open_session",{directory:$("project-folder").value});
    await refresh();await select(address);$("project-picker").close();$("draft").focus();
  } catch(error){$("picker-error").textContent=String(error);$("picker-error").hidden=false;}
  finally{control.disabled=false;}
};
$("theme").onchange=event=>{if(event.target.value==="auto")delete document.documentElement.dataset.theme;else document.documentElement.dataset.theme=event.target.value;};
$("copy").onclick=async()=>{
  const value=view?.messages.map(message=>message.role+"\n"+message.text).join("\n\n")??"";
  try{await navigator.clipboard.writeText(value);}catch(error){showError("Could not copy the conversation. "+error);}
};
$("close-session").onclick=async()=>{
  if(!selected)return;
  const address=selected;
  try{
    await invoke("close_session",{address});labels.delete(address.session);await refresh();
    const next=sessions[0];
    if(next)await select(next.address);
    else {selected=null;selection.current=null;$("status").textContent="Closed";$("draft").disabled=true;$("send").disabled=true;$("stop").disabled=true;$("new-session").disabled=true;$("close-session").disabled=true;}
  }catch(error){showError(error);}
};
document.addEventListener("keydown",event=>{
  if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==="o"){event.preventDefault();$("open-project").click();}
});
try {
  const info=await refresh();
  document.documentElement.dataset.reducedMotion=String(info.reduce_motion||matchMedia("(prefers-reduced-motion: reduce)").matches);
  await select(info.initial);
} catch(error){showError(error);}