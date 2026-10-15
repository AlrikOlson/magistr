import { paintPatch } from "./patch_document.mjs";
import { paintSource } from "./source_color.mjs";

// Render source without changing the authored bytes or the transcript's selection ownership.
function sourceText(node) {
  return node.text || (node.children ?? []).map(sourceText).join("");
}
const states = new WeakMap();
export function renderCodeBlock(figure, source, copy) {
  let state = states.get(figure);
  if (!state) {
    const document = figure.ownerDocument;
    const create = (tag, className, value = "") => {
      const node = document.createElement(tag); node.className = className; node.textContent = value; return node;
    };
    figure.className = "code-document";
    const caption = create("figcaption", "code-caption");
    const title = create("span", "code-title", "Code");
    const count = create("span", "code-count");
    const button = create("button", "code-copy", "Copy code"); button.type = "button";
    const wrap = create("button", "code-wrap", "Wrap lines");wrap.type = "button";wrap.setAttribute("aria-pressed","false");
    wrap.onclick = () => {const wrapped=wrap.getAttribute("aria-pressed")!=="true";wrap.setAttribute("aria-pressed",String(wrapped));figure.dataset.wrapped=String(wrapped);};
    const surface = create("div", "code-surface");
    const gutter = create("span", "code-gutter"); gutter.setAttribute("aria-hidden", "true");
    const pre = create("pre", ""); pre.tabIndex = 0; pre.setAttribute("aria-label", "Code");
    const code = create("code", "");
    pre.append(code); surface.append(gutter, pre); caption.append(title, count, wrap, button);
    figure.append(caption, surface); state = { title, count, button, wrap, gutter, code }; states.set(figure, state);
  }
  const text = sourceText(source);
  const lines = text ? text.split("\n").length - Number(text.endsWith("\n")) : 0;
  const patch=["diff","patch"].includes(source.language)?paintPatch(state.code,text):null;
  figure.classList.toggle("patch-document",!!patch);state.wrap.hidden=!!patch;
  if(!patch)paintSource(state.code, text, source.language);
  state.title.textContent = patch ? "Changes" : source.language || "Code";
  state.count.textContent = patch ? "+"+patch.added+" added · −"+patch.removed+" removed" : lines.toLocaleString() + (lines === 1 ? " line" : " lines");
  // Large sources remain complete; avoid manufacturing thousands of gutter nodes.
  state.gutter.hidden = !!patch || lines > 1000;
  const numbers = lines > 1000 ? "" : Array.from({ length: lines }, (_, i) => i + 1).join("\n");
  if (state.gutter.dataset.lines !== numbers) state.gutter.dataset.lines = numbers;
  state.button.disabled = !copy || !text;
  state.button.onclick = () => copy?.(text);
}
