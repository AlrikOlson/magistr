const $ = (selector) => document.querySelector(selector);
let selected = null;
let listRequest = 0;
let selectionRequest = 0;
const sampleMail = [
  {external_id:"sample-brakes",thread_id:"brakes",sender:"Nora Ellis <nora@example.test>",subject:"Can you look at my brakes?",body:"Hi! My rear brake has started rubbing after the weekend ride. Could I bring the bike in on Friday? I use it to get to work, so a same-day repair would be ideal.\n\nThanks,\nNora"},
  {external_id:"sample-order",thread_id:"order",sender:"Owen Park <owen@example.test>",subject:"Checking on the wheel order",body:"Hello, I ordered a replacement rear wheel last Tuesday. Has it arrived yet? The order reference is WS-1048.\n\nOwen"},
  {external_id:"sample-workshop",thread_id:"workshop",sender:"Alex Chen <alex@example.test>",subject:"Two places at the maintenance class",body:"I'd like to book two places for Saturday's beginner maintenance class. Do we need to bring our own tools?\n\nAlex"}
];
function node(tag, text, className) {
  const element = document.createElement(tag);
  if (text !== undefined) element.textContent = text;
  if (className) element.className = className;
  return element;
}
function notice(message, error = false) {
  $("#notice").textContent = message;
  $("#notice").dataset.error = String(error);
}
async function api(path, method = "GET", body) {
  const response = await fetch(path, {method, headers:{"Content-Type":"application/json"}, body:body === undefined ? undefined : JSON.stringify(body)});
  const data = await response.json();
  if (!response.ok) {
    const error = new Error(data.error || "Could not load the inbox.");
    error.status = response.status;
    throw error;
  }
  return data;
}
function safely(action) {
  return async () => { try { await action(); } catch (error) { notice(error.message, true); } };
}
async function refresh() {
  const request = ++listRequest;
  const query = new URLSearchParams({q:$("#search").value,status:$("#status").value,assignee:$("#assigned").value});
  const rows = await api("/api/conversations?" + query);
  if (request !== listRequest) return;
  $("#count").textContent = rows.length;
  $("#empty").hidden = rows.length > 0;
  $("#empty").textContent = query.get("q") || query.get("status") || query.get("assignee")
    ? "No conversations match. Try a different search or filter."
    : "Your inbox is empty. Load sample mail or receive a message through the mail endpoint.";
  $("#list").replaceChildren(...rows.map(item => {
    const button = node("button", undefined, "conversation");
    button.type = "button";
    button.dataset.id = item.id;
    button.setAttribute("aria-pressed", String(selected?.id === item.id));
    const meta = node("span", undefined, "row-meta");
    meta.append(node("span", item.status === "open" ? "Open" : "Resolved"), node("span", (item.assignee || "Unassigned") + " · " + item.message_count + (item.message_count === 1 ? " message" : " messages")));
    button.append(node("span", item.sender.split(" <")[0], "sender"), node("strong", item.subject), node("span", item.preview, "preview"), meta);
    button.addEventListener("click", safely(() => openConversation(item.id)));
    return button;
  }));
}
async function openConversation(id) {
  const request = ++selectionRequest;
  const item = await api("/api/conversations/" + id);
  if (request !== selectionRequest) return;
  selected = item;
  renderDetail();
  await refresh();
}
function renderDetail() {
  const item = selected;
  const heading = node("div", undefined, "detail-heading");
  const title = node("div");
  const h2 = node("h2", item.subject);
  h2.tabIndex = -1;
  title.append(h2, node("p", item.sender, "detail-subtitle"));
  heading.append(title, node("span", item.status === "open" ? "Open" : "Resolved", "state"));
  const controls = node("div", undefined, "detail-actions");
  const label = node("label", "Assigned to");
  const assignee = node("select");
  assignee.id = "conversation-assignee";
  for (const name of ["", "Maya", "Leo", "Sam"]) {
    const option = node("option", name || "Unassigned");
    option.value = name;
    option.selected = name === item.assignee;
    assignee.append(option);
  }
  label.append(assignee);
  assignee.addEventListener("change", safely(() => save({assignee:assignee.value})));
  const resolve = node("button", item.status === "open" ? "Mark resolved" : "Reopen conversation");
  resolve.type = "button";
  resolve.id = "conversation-status";
  resolve.addEventListener("click", safely(() => save({status:item.status === "open" ? "resolved" : "open"})));
  controls.append(label, resolve);
  const messages = item.messages.map(message => {
    const article = node("article", undefined, "message");
    const header = node("header");
    const date = new Date(message.received_at);
    header.append(node("span", message.sender.split(" <")[0]), node("time", date.toLocaleTimeString([], {hour:"2-digit",minute:"2-digit"})));
    article.append(header, node("p", message.body));
    return article;
  });
  $("#detail").replaceChildren(heading, controls, ...messages);
}
async function save(patch) {
  const id = selected.id;
  const focusedId = document.activeElement?.id;
  try {
    selected = await api("/api/conversations/" + id, "PATCH", {version:selected.version,...patch});
    renderDetail();
    await refresh();
    notice(patch.status ? (selected.status === "resolved" ? "Conversation resolved." : "Conversation reopened.") : "Assigned to " + (selected.assignee || "nobody") + ".");
  } catch (error) {
    if (error.status === 409) {
      await openConversation(id);
      error.message = "Someone else updated this conversation. Latest changes loaded; choose again.";
    }
    throw error;
  } finally {
    if (focusedId) document.getElementById(focusedId)?.focus({preventScroll:true});
  }
}
$("#filters").addEventListener("submit", event => { event.preventDefault(); safely(refresh)(); });
let searchTimer;
$("#search").addEventListener("input", () => { clearTimeout(searchTimer); searchTimer = setTimeout(safely(refresh), 150); });
for (const id of ["#status", "#assigned"]) $(id).addEventListener("change", safely(refresh));
$("#refresh").addEventListener("click", safely(async () => { if (selected) await openConversation(selected.id); else await refresh(); notice("Inbox is up to date."); }));
$("#sample").addEventListener("click", safely(async () => {
  $("#sample").disabled = true;
  try {
    let added = 0;
    for (const message of sampleMail) if ((await api("/api/mail", "POST", message)).created) added++;
    await refresh();
    notice(added ? added + " sample conversations received." : "All three messages were already received. No duplicates added.");
  } finally { $("#sample").disabled = false; }
}));
safely(refresh)();
