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
    const surface = create("div", "code-surface");
    const gutter = create("span", "code-gutter"); gutter.setAttribute("aria-hidden", "true");
    const pre = create("pre", ""); pre.tabIndex = 0; pre.setAttribute("aria-label", "Code");
    const code = create("code", "");
    pre.append(code); surface.append(gutter, pre); caption.append(title, count, button);
    figure.append(caption, surface); state = { title, count, button, gutter, code }; states.set(figure, state);
  }
  const text = sourceText(source);
  const lines = text ? text.split("\n").length - Number(text.endsWith("\n")) : 0;
  paintSource(state.code, text, source.language);
  state.title.textContent = source.language || "Code";
  state.count.textContent = lines.toLocaleString() + (lines === 1 ? " line" : " lines");
  // Large sources remain complete; avoid manufacturing thousands of gutter nodes.
  state.gutter.hidden = lines > 1000;
  const numbers = lines > 1000 ? "" : Array.from({ length: lines }, (_, i) => i + 1).join("\n");
  if (state.gutter.dataset.lines !== numbers) state.gutter.dataset.lines = numbers;
  state.button.disabled = !copy || !text;
  state.button.onclick = () => copy?.(state.code.textContent);
}
