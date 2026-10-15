import test from "node:test";
import assert from "node:assert/strict";
import { SavedDraftEditor } from "../ui/saved_drafts.mjs";

const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return {promise, resolve};
};

function harness() {
  const elements = new Map(), calls = [];
  const records = new Map([
    ["first", {revision: 9, text: "First conversation's message"}],
    ["second", {revision: 1, text: "Second conversation's message"}],
    ["third", {revision: 3, text: "Third conversation's message"}],
  ]);
  const document = {activeElement: null, createElement: () => node()};
  function node() {
    const listeners = new Map();
    return {
      value: "", textContent: "", disabled: false, hidden: false,
      children: [], ownerDocument: document, parent: null, attributes: {}, dataset: {},
      addEventListener: (name, listener) => listeners.set(name, listener),
      setAttribute(name, value) { this.attributes[name] = value; },
      replaceChildren(...children) {for(const child of this.children)child.parent=null;this.children=[];this.append(...children);},
      append(...children) { for (const child of children) this.insertBefore(child, null); },
      insertBefore(child, before) {
        child.remove();
        const index = before === null ? this.children.length : this.children.indexOf(before);
        this.children.splice(index, 0, child); child.parent = this;
      },
      remove() {
        if (this.parent) this.parent.children.splice(this.parent.children.indexOf(this), 1);
        this.parent = null;
      },
      contains(candidate) {
        return candidate === this || this.children.some(child => child.contains(candidate));
      },
      input(value) {
        assert.equal(this.disabled, false, "A user cannot edit a disabled control");
        this.value = value;
        listeners.get("input")?.();
      },
    };
  }
  const element = id => {
    if (!elements.has(id)) elements.set(id, node());
    return elements.get(id);
  };
  const status = (record, conversation) => ({
    conversation,
    revision: record.revision, saved_revision: record.revision, save_attempt: 0, error: null,
  });
  const fixture = {element, records, calls, owner: null, before: async () => {}};
  const invoke = async (command, args) => {
    calls.push({command, ...args});
    await fixture.before(command, args);
    const record = records.get(args.id);
    assert.ok(record, "Every request addresses a real conversation");
    if (command === "saved_draft") {
      return {text: record.text, attachments: record.attachments ?? [], status: status(record, args.id)};
    }
    assert.equal(args.expectedRevision, record.revision,
      "A write must compare the revision of its own conversation");
    if (command === "save_saved_draft") record.text = args.text;
    else if (command === "discard_saved_draft") record.text = "";
    else throw new Error("Unexpected fixture command: " + command);
    record.revision += 1;
    return status(record, args.id);
  };
  fixture.editor = new SavedDraftEditor(element, invoke, () => fixture.owner);
  return fixture;
}

test("switching during discard keeps its result with the original conversation", async () => {
  const h = harness(), entered = deferred(), release = deferred();
  await h.editor.open({id: "first"});
  h.before = async command => {
    if (command === "discard_saved_draft") { entered.resolve(); await release.promise; }
  };
  const discard = h.editor.mutate("discard_saved_draft");
  await entered.promise;
  assert.equal(h.element("saved-message").disabled, true);
  const switching = h.editor.open({id: "second"});
  assert.equal(h.calls.some(call => call.id === "second"), false);
  release.resolve();
  await discard;
  assert.equal(await switching, true);
  assert.equal(h.records.get("first").text, "");
  assert.equal(h.editor.current.conversation, "second");
  assert.equal(h.element("saved-message").value, "Second conversation's message");
  assert.equal(h.editor.writer.status({conversation: "second"}).revision, 1);
  assert.equal(h.editor.writer.status({conversation: "second"}).save_attempt, 0);
  h.element("saved-message").input("A later edit to the second conversation");
  await h.editor.flush();
  assert.equal(h.records.get("second").text, "A later edit to the second conversation");
});

test("switching during explicit Save cannot redirect its text or revision", async () => {
  const h = harness(), entered = deferred(), release = deferred();
  await h.editor.open({id: "first"});
  h.element("saved-message").input("Keep this text with the first conversation");
  await h.editor.flush();
  h.before = async (command, args) => {
    if (command === "saved_draft" && args.id === "first") {
      entered.resolve(); await release.promise;
    }
  };
  const marker = h.calls.length, saving = h.editor.retry();
  await entered.promise;
  assert.equal(h.element("saved-message").disabled, true);
  const switching = h.editor.open({id: "second"});
  release.resolve();
  await saving;
  assert.equal(await switching, true);
  assert.equal(h.records.get("first").text, "Keep this text with the first conversation");
  assert.equal(h.records.get("second").text, "Second conversation's message");
  assert.equal(h.element("saved-message").value, "Second conversation's message");
  assert.deepEqual(h.calls.slice(marker).filter(call => call.command === "save_saved_draft")
    .map(call => call.id), ["first"]);
  assert.equal(h.editor.writer.status({conversation: "second"}).revision, 1);
  assert.equal(h.editor.writer.status({conversation: "second"}).save_attempt, 0);
});

test("the latest navigation request wins while a previous discard settles", async () => {
  const h = harness(), entered = deferred(), release = deferred();
  await h.editor.open({id: "first"});
  h.before = async command => {
    if (command === "discard_saved_draft") { entered.resolve(); await release.promise; }
  };
  const discard = h.editor.mutate("discard_saved_draft");
  await entered.promise;
  const earlier = h.editor.open({id: "second"});
  const latest = h.editor.open({id: "third"});
  release.resolve();
  await discard;
  assert.deepEqual(await Promise.all([earlier, latest]), [false, true]);
  assert.equal(h.calls.some(call => call.id === "second"), false);
  assert.equal(h.editor.current.conversation, "third");
  assert.equal(h.element("saved-message").value, "Third conversation's message");
  assert.equal(h.editor.writer.status({conversation: "third"}).revision, 3);
});

test("an older history read cannot replace a newer selected message", async () => {
  const h = harness(), entered = deferred(), release = deferred();
  await h.editor.open({id: "first"});
  h.before = async (command, args) => {
    if (command === "saved_draft" && args.id === "second") {
      entered.resolve(); await release.promise;
    }
  };
  const earlier = h.editor.open({id: "second"});
  await entered.promise;
  assert.equal(await h.editor.open({id: "third"}), true);
  release.resolve();
  assert.equal(await earlier, false);
  assert.equal(h.editor.current.conversation, "third");
  assert.equal(h.element("saved-message").value, "Third conversation's message");
});

test("a rejected discard keeps the original text and a visible persistent error", async () => {
  const h = harness();
  await h.editor.open({id: "first"});
  h.before = async command => {
    if (command === "discard_saved_draft") throw new Error("The disk is full.");
  };
  await h.editor.mutate("discard_saved_draft");
  h.editor.render();
  assert.match(h.element("saved-draft-status").textContent, /disk is full/);
  assert.equal(h.element("saved-message").value, "First conversation's message");
  assert.equal(h.records.get("first").text, "First conversation's message");
  assert.equal(h.element("saved-message").disabled, false);
});

test("failed save during navigation leaves the unsaved editor and retry available", async () => {
  const h = harness();
  await h.editor.open({id: "first"});
  h.before = async command => {
    if (command === "save_saved_draft") throw new Error("The disk is full.");
  };
  h.element("saved-message").input("This unsaved edit must remain reachable");
  await assert.rejects(h.editor.flush(), /disk is full/);
  await assert.rejects(h.editor.open({id: "second"}), /disk is full/);
  assert.equal(h.element("saved-draft").hidden, false);
  assert.equal(h.element("saved-message").value, "This unsaved edit must remain reachable");
  assert.equal(h.element("saved-message").disabled, false);
  assert.equal(h.element("save-saved-message").disabled, false);
  assert.match(h.element("saved-draft-status").textContent, /disk is full/);
  assert.equal(h.editor.current.conversation, "first");
  assert.equal(h.calls.some(call => call.id === "second"), false);
  h.before = async () => {};
  await h.editor.retry();
  assert.equal(h.records.get("first").text, "This unsaved edit must remain reachable");
});

test("a failed history read restores editing of the previously selected message", async () => {
  const h = harness(), entered = deferred(), release = deferred();
  await h.editor.open({id: "first"});
  h.before = async (command, args) => {
    if (command === "saved_draft" && args.id === "second") {
      entered.resolve(); await release.promise;
      throw new Error("The saved message cannot be read.");
    }
  };
  const switching = h.editor.open({id: "second"});
  await entered.promise;
  assert.equal(h.element("saved-draft").hidden, false);
  assert.equal(h.element("saved-message").disabled, true);
  assert.equal(h.element("saved-message").value, "First conversation's message");
  release.resolve();
  await assert.rejects(switching, /cannot be read/);
  assert.equal(h.editor.current.conversation, "first");
  assert.equal(h.element("saved-message").disabled, false);
  h.element("saved-message").input("The original conversation remains editable");
  await h.editor.flush();
  assert.equal(h.records.get("first").text, "The original conversation remains editable");
});

test("a failed discard cannot silently complete a queued history switch", async () => {
  const h = harness(), entered = deferred(), release = deferred();
  await h.editor.open({id: "first"});
  h.before = async command => {
    if (command === "discard_saved_draft") {
      entered.resolve(); await release.promise;
      throw new Error("The disk is full.");
    }
  };
  const discard = h.editor.mutate("discard_saved_draft");
  await entered.promise;
  const switching = h.editor.open({id: "second"});
  const refused = assert.rejects(switching, /disk is full/);
  release.resolve();
  await Promise.all([discard, refused]);
  assert.equal(h.editor.current.conversation, "first");
  assert.equal(h.element("saved-draft").hidden, false);
  assert.equal(h.element("saved-message").disabled, false);
  assert.equal(h.records.get("first").text, "First conversation's message");
  assert.match(h.element("saved-draft-status").textContent, /disk is full/);
  assert.equal(h.calls.some(call => call.id === "second"), false);
  h.before = async () => {};
  await h.editor.retry();
  assert.equal(await h.editor.open({id: "second"}), true);
});

test("attachment removal is disabled throughout saved actions and history loading", async () => {
  const h = harness();
  h.records.get("first").attachments = [
    {id: "file", name: "notes.txt", size: 3, content_hash: "abc"},
  ];
  await h.editor.open({id: "first"});
  const remove = h.element("saved-attachments").children[0].children
    .find(child => child.attributes["aria-label"] === "Remove notes.txt");
  assert.equal(remove.disabled, false);
  for (const action of ["save", "open"]) {
    const entered = deferred(), release = deferred();
    h.before = async command => {
      if (command === "saved_draft") { entered.resolve(); await release.promise; }
    };
    const operation = action === "save" ? h.editor.retry() : h.editor.open({id: "second"});
    await entered.promise;
    assert.equal(remove.disabled, true);
    release.resolve();
    await operation;
    if (action === "save") assert.equal(remove.disabled, false);
  }
  assert.equal(h.editor.current.conversation, "second");
  assert.equal(h.element("saved-attachments").hidden, true);
});

test("saved history selection cannot reuse another conversation's file preview",async()=>{
 const h=harness();
 h.records.get("first").attachments=[{id:"same",name:"first.png",mime_type:"image/png",preview:"data:image/png;base64,first",size:5}];
 h.records.get("second").attachments=[{id:"same",name:"second.txt",excerpt:"second text",size:11}];
 await h.editor.open({id:"first"});
 assert.equal(h.editor.files.files[0].preview,"data:image/png;base64,first");
 await h.editor.open({id:"second"});
 assert.equal(h.editor.files.files[0].preview,undefined);
 assert.equal(h.editor.files.files[0].excerpt,"second text");
});

test("parked and closing claims keep saved text read-only until the owner releases it",async()=>{
 const h=harness();
 await h.editor.open({id:"first"});
 h.owner={target:{conversation:"planning"},closed:false};
 h.editor.render();
 assert.equal(h.element("saved-message").disabled,true);
 assert.equal(h.element("save-saved-message").disabled,true);
 assert.equal(h.element("discard-saved-draft").disabled,true);
 assert.match(h.element("saved-draft-status").textContent,/Close that session/);
 const before=h.calls.length;
 await h.editor.mutate("discard_saved_draft");
 assert.equal(h.calls.length,before);
 h.owner.target.conversation="first";h.editor.render();
 assert.match(h.element("saved-draft-status").textContent,/Resume it/);
 h.owner.closed=true;h.editor.render();
 assert.match(h.element("saved-draft-status").textContent,/still closing/);
 h.owner=null;h.editor.render();
 assert.equal(h.element("saved-message").disabled,false);
 h.element("saved-message").input("Editable after release");
 await h.editor.flush();
 assert.equal(h.records.get("first").text,"Editable after release");
});

test("closing a browser cancels its pending read without replacing the previous editor",async()=>{
 const h=harness(),entered=deferred(),release=deferred();
 await h.editor.open({id:"first"});
 h.before=async(command,args)=>{if(command==="saved_draft"&&args.id==="second"){entered.resolve();await release.promise;}};
 const opening=h.editor.open({id:"second"});await entered.promise;
 h.editor.cancelOpen();
 h.element("saved-message").input("Keep editing the original message");await h.editor.flush();
 release.resolve();
 assert.equal(await opening,false);
 assert.equal(h.editor.current.conversation,"first");
 assert.equal(h.element("saved-message").value,"Keep editing the original message");
});
