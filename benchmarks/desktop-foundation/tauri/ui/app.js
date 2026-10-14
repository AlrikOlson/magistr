import { attachComposer } from "./composer.mjs";
const { invoke, Channel } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);
const state = {
  workspace: 0,
  session: 0,
  running: false,
  sequences: Array(8).fill(0),
  texts: Array(8).fill(""),
  frames: [],
  delays: [],
  delayed: 0,
  errors: [],
  received: 0,
  dropped: 0,
};
let corpus,
  selectedItem = null;
let pending = false,
  streamFinished = false,
  stopping = false,
  started = 0,
  pendingEvents = [];
function record(kind, value) {
  return invoke("record", { kind, value });
}
function snapshot() {
  return record("snapshot", {
    workspace: state.workspace,
    session: state.session,
    draft: $("draft").value,
    running: state.running,
    sequences: state.sequences,
    received: state.received,
    errors: state.errors,
    roadmap: !$("roadmap-pane").hidden,
    light: document.body.classList.contains("light"),
    reduced_motion: $("motion").getAttribute("aria-pressed") === "true",
    waiting: !$("request").hidden,
    item: selectedItem,
    status: $("status").textContent,
  });
}
function virtual(scroller, container, rows, height, render) {
  let current = -1;
  container.style.height = rows.length * height + "px";
  const paint = () => {
    const first = Math.max(0, Math.floor(scroller.scrollTop / height) - 2);
    const count = Math.ceil(scroller.clientHeight / height) + 5;
    if (current === first) return;
    current = first;
    const children = [];
    for (let i = first; i < Math.min(rows.length, first + count); i++) {
      const node = render(rows[i], i);
      node.style.top = i * height + "px";
      children.push(node);
    }
    container.replaceChildren(...children);
  };
  scroller.addEventListener(
    "scroll",
    () =>
      requestAnimationFrame(() => {
        const start = performance.now();
        paint();
        state.frames.push(performance.now() - start);
      }),
    { passive: true },
  );
  new ResizeObserver(() => {
    current = -1;
    requestAnimationFrame(() => {
      const start = performance.now();
      paint();
      state.frames.push(performance.now() - start);
    });
  }).observe(scroller);
  paint();
  return () => {
    current = -1;
    paint();
  };
}
function makeMessage(source) {
  const lines = source.split("\n"),
    node = document.createElement("article");
  node.className = "message";
  const title = document.createElement("h2");
  title.textContent = lines[0];
  node.append(title);
  const prose = document.createElement("p");
  prose.textContent = lines[1];
  node.append(prose);
  const code = document.createElement("code");
  code.textContent = lines[2];
  node.append(code);
  const heading = document.createElement("h3");
  heading.textContent = lines[3];
  node.append(heading);
  const list = document.createElement("ul");
  for (const text of lines.slice(4, 6)) {
    const item = document.createElement("li");
    item.textContent = text;
    list.append(item);
  }
  node.append(list);
  const table = document.createElement("table");
  for (const [index, text] of lines.slice(6, 8).entries()) {
    const row = document.createElement("tr");
    for (const cell of text.split(" | ")) {
      const field = document.createElement(index ? "td" : "th");
      field.textContent = cell;
      row.append(field);
    }
    table.append(row);
  }
  node.append(table);
  const a = document.createElement("a");
  a.href = lines[8];
  a.textContent = "Recorded evidence";
  node.append(a);
  return node;
}
function workspace(index) {
  state.workspace = index;
  $("workspace-title").textContent = index ? "Source review" : "Magistr";
  $("branch").textContent = index ? "review" : "main";
  $("workspace-main").classList.toggle("selected", index === 0);
  $("workspace-review").classList.toggle("selected", index === 1);
  $("marker").style.background = index ? "#ffcc80" : "#83b2ff";
  $("marker").setAttribute(
    "aria-label",
    index ? "Review branch selected" : "Main branch selected",
  );
  requestAnimationFrame(snapshot);
}
function session(index) {
  state.session = index;
  renderSessions();
  flush();
}
function renderSessions() {
  if (!$("sessions").children.length) {
    for (let i = 0; i < 8; i++) {
      const button = document.createElement("button");
      button.className = "session";
      button.onclick = () => session(i);
      $("sessions").append(button);
    }
  }
  Array.from($("sessions").children).forEach((button, i) => {
    button.classList.toggle("selected", state.session === i);
    button.textContent =
      "Agent " +
      (i + 1) +
      " · " +
      (state.running
        ? "Working"
        : i === 2 && !$("request").hidden
          ? "Waiting for you"
          : "Ready") +
      " · " +
      state.sequences[i];
  });
}
function flush() {
  pending = false;
  $("live").textContent = state.texts[state.session]
    .trimEnd()
    .split("\n")
    .slice(-3)
    .join("\n");
  renderSessions();
  const events = pendingEvents;
  pendingEvents = [];
  // This acknowledges a rendering callback, not compositor-visible pixels.
  requestAnimationFrame(() => {
    const now = (performance.timeOrigin + performance.now()) * 1000;
    for (const event of events) {
      const delay = Math.max(0, (now - event.sent_us) / 1000);
      state.delays.push(delay);
      if (delay > 50) state.delayed++;
    }
    if (streamFinished) {
      state.running = false;
      $("stop").disabled = true;
      $("start").disabled = false;
      $("status").textContent = stopping ? "Work stopped" : "Work complete";
      renderSessions();
      record("metrics", {
        received: state.received,
        sequences: state.sequences,
        acknowledged_sequences: [...state.sequences],
        dropped: state.dropped,
        delays_ms: state.delays,
        delayed: state.delayed,
        frames_ms: state.frames,
        errors: state.errors,
        elapsed_ms: performance.now() - started,
        acknowledgement: "requestAnimationFrame after DOM update",
      });
    }
    snapshot();
  });
}
function start() {
  if (state.running) return;
  state.running = true;
  state.sequences.fill(0);
  state.texts.fill("");
  state.received = 0;
  state.errors = [];
  state.delays = [];
  state.delayed = 0;
  state.dropped = 0;
  pendingEvents = [];
  streamFinished = false;
  stopping = false;
  started = performance.now();
  $("stop").disabled = false;
  $("start").disabled = true;
  $("status").textContent = "Working · 8 agents";
  renderSessions();
  const channel = new Channel();
  channel.onmessage = (batch) => {
    state.dropped = batch.dropped;
    streamFinished = batch.finished;
    for (const event of batch.events) {
      if (state.sequences[event.stream] !== event.sequence) {
        state.errors.push(
          "Stream " + event.stream + " sequence " + event.sequence,
        );
        continue;
      }
      state.sequences[event.stream]++;
      state.texts[event.stream] += event.text;
      state.received++;
      pendingEvents.push(event);
    }
    if (!pending) {
      pending = true;
      requestAnimationFrame(flush);
    }
  };
  invoke("stream", { channel }).catch((error) => {
    state.errors.push(String(error));
    $("status").textContent = "Work could not start";
  });
}
async function stop() {
  if (!state.running) return;
  stopping = true;
  $("status").textContent = "Stopping…";
  await invoke("stop");
}
function showRoadmap(value) {
  $("roadmap-pane").hidden = !value;
  $("change-pane").hidden = value;
  $("roadmap").classList.toggle("selected", value);
  $("changes").classList.toggle("selected", !value);
  snapshot();
}
function answer(allowed) {
  $("request").hidden = true;
  $("status").textContent = allowed ? "Command allowed" : "Command denied";
  record("journey", {
    answer: allowed ? "allow" : "deny",
    draft: $("draft").value,
  });
  renderSessions();
  snapshot();
}
async function boot() {
  corpus = await invoke("fixture");
  renderSessions();
  virtual($("conversation"), $("messages"), corpus.messages, 260, makeMessage);
  virtual($("diff"), $("diff-lines"), corpus.diff, 24, (text) => {
    const node = document.createElement("div");
    node.className =
      "diff-line " +
      (text[0] === "+" ? "added" : text[0] === "-" ? "removed" : "");
    node.textContent = text;
    return node;
  });
  virtual($("items"), $("item-lines"), corpus.items, 32, (text, index) => {
    const node = document.createElement("button");
    node.className = "item";
    node.textContent = text;
    node.onclick = () => {
      selectedItem = index;
      $("item-title").textContent = text;
      record("journey", { item: index, requirements: true, evidence: true });
      snapshot();
    };
    return node;
  });
  $("workspace-main").onclick = () => workspace(0);
  $("workspace-review").onclick = () => workspace(1);
  $("choose-main").onclick = () => workspace(0);
  $("choose-review").onclick = () => workspace(1);
  $("open").onclick = () => $("workspace-picker").showModal();
  $("changes").onclick = () => showRoadmap(false);
  $("roadmap").onclick = () => showRoadmap(true);
  $("allow").onclick = () => answer(true);
  $("deny").onclick = () => answer(false);
  $("theme").onclick = () => {
    document.body.classList.toggle("light");
    snapshot();
  };
  $("motion").onclick = () => {
    $("motion").setAttribute(
      "aria-pressed",
      String($("motion").getAttribute("aria-pressed") !== "true"),
    );
    snapshot();
  };
  $("start").onclick = start;
  $("stop").onclick = stop;
  $("draft").oninput = () => {
    const entered = $("draft").value.length > 0;
    $("input-state").textContent = $("draft").value.length + " characters";
    $("input-marker").style.background = entered ? "#ffcc80" : "#83b2ff";
    $("input-marker").setAttribute(
      "aria-label",
      entered ? "Message entered" : "Message empty",
    );
    requestAnimationFrame(snapshot);
  };
  attachComposer($("draft"), $("composer"));
  $("composer").onsubmit = (event) => {
    event.preventDefault();
    const value = $("draft").value;
    if (!value.trim()) return;
    $("status").textContent = "Message sent";
    $("draft").value = "";
    $("draft").dispatchEvent(new Event("input"));
    record("journey", { sent: value });
    snapshot();
  };
  document.addEventListener("keydown", (event) => {
    if (event.metaKey && event.key === "1") {
      event.preventDefault();
      workspace(0);
    }
    if (event.metaKey && event.key === "2") {
      event.preventDefault();
      workspace(1);
    }
    if (event.metaKey && event.key === "d") {
      event.preventDefault();
      showRoadmap(false);
    }
    if (event.metaKey && event.key === "r") {
      event.preventDefault();
      showRoadmap(true);
    }
  });
  requestAnimationFrame(() =>
    requestAnimationFrame(() =>
      record("ready", {
        items: corpus.items.length,
        messages: corpus.messages.length,
        diff: corpus.diff.length,
      }),
    ),
  );
  $("draft").focus();
  await snapshot();
}
boot().catch((error) => {
  $("status").textContent = "Could not open work: " + error;
});
