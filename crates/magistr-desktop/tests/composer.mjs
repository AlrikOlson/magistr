import assert from "node:assert/strict";
import test from "node:test";
import { attachComposer } from "../ui/composer.mjs";

function editor() {
  const draft = new EventTarget();
  let sent = 0;
  attachComposer(draft, {
    requestSubmit() {
      sent++;
    },
  });
  return {
    fire(type, properties = {}) {
      const event = new Event(type, { cancelable: true });
      Object.assign(event, properties);
      draft.dispatchEvent(event);
      return event;
    },
    get sent() {
      return sent;
    },
  };
}
const enter = { key: "Enter", isComposing: false, shiftKey: false };
test("WebKit native conversion ends before its Return key-down", () => {
  const input = editor();
  input.fire("compositionstart");
  input.fire("keyup", { ...enter, isComposing: true });
  input.fire("compositionend", { data: "日本" });
  input.fire("keydown", enter);
  assert.equal(input.sent, 0, "conversion must remain in the draft");
  input.fire("keyup", enter);
  input.fire("keydown", enter);
  assert.equal(input.sent, 1, "a separate Return sends the committed draft");
});
test("conversion key-down before compositionend also needs only one Return", () => {
  const input = editor();
  input.fire("compositionstart");
  input.fire("keydown", { ...enter, isComposing: true });
  input.fire("compositionend", { data: "日本" });
  input.fire("keyup", enter);
  input.fire("keydown", enter);
  assert.equal(input.sent, 1);
});
test("Shift Return remains native multiline editing", () => {
  const input = editor();
  const event = input.fire("keydown", { ...enter, shiftKey: true });
  assert.equal(input.sent, 0);
  assert.equal(event.defaultPrevented, false);
});
test("a cancelled composition does not swallow a later send", () => {
  const input = editor();
  input.fire("compositionstart");
  input.fire("keydown", { key: "Escape", isComposing: true });
  input.fire("compositionend", { data: "" });
  input.fire("keydown", enter);
  assert.equal(input.sent, 1);
});
test("typing after conversion starts a separate keyboard action", () => {
  const input = editor();
  input.fire("compositionstart");
  input.fire("compositionend", { data: "日本" });
  input.fire("keydown", { key: "a", isComposing: false });
  input.fire("keydown", enter);
  assert.equal(input.sent, 1);
});
test("leaving the editor ends its submission boundary", () => {
  const input = editor();
  input.fire("compositionstart");
  input.fire("compositionend", { data: "日本" });
  input.fire("blur");
  input.fire("keydown", enter);
  assert.equal(input.sent, 1);
});