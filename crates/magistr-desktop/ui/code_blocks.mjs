import { paintPatch, paintPatchComparison } from "./patch_document.mjs";
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
    const compare=create("button","code-compare","Side by side");compare.type="button";compare.setAttribute("aria-pressed","false");compare.hidden=true;
    const comparison=create("div","patch-comparison");comparison.hidden=true;comparison.tabIndex=0;comparison.setAttribute("role","region");comparison.setAttribute("aria-label","Before and after");
    const balance = create("div", "patch-balance"); balance.setAttribute("aria-hidden","true");
    const removed = create("span", "patch-balance-removed"), added = create("span", "patch-balance-added"); balance.append(removed,added);
    const surface = create("div", "code-surface");
    const gutter = create("span", "code-gutter"); gutter.setAttribute("aria-hidden", "true");
    const pre = create("pre", ""); pre.tabIndex = 0; pre.setAttribute("aria-label", "Code");
    const code = create("code", "");
    const overview = create("div", "code-overview"); overview.setAttribute("aria-hidden", "true");
    pre.append(code); surface.append(gutter, pre, overview); caption.append(title, count, wrap, compare, button);
    figure.append(caption, balance, surface, comparison); state = { title, count, button, wrap, compare, comparison, surface, gutter, code, overview, balance, removed, added }; states.set(figure, state);
  }
  const text = sourceText(source);
  const lines = text ? text.split("\n").length - Number(text.endsWith("\n")) : 0;
  const patch=["diff","patch"].includes(source.language)?paintPatch(state.code,text):null;
  figure.classList.toggle("patch-document",!!patch);state.wrap.hidden=!!patch;state.compare.hidden=!patch;
  const showComparison=()=>{const active=!!patch&&state.compare.getAttribute("aria-pressed")==="true";state.comparison.hidden=!active;state.surface.hidden=active;if(active)paintPatchComparison(state.comparison,patch);};
  state.compare.onclick=()=>{state.compare.setAttribute("aria-pressed",String(state.compare.getAttribute("aria-pressed")!=="true"));showComparison();};showComparison();
  if(!patch)paintSource(state.code, text, source.language);
  state.title.textContent = patch ? "Changes" : source.language || "Code";
  state.count.textContent = patch ? "+"+patch.added+" added · −"+patch.removed+" removed" : lines.toLocaleString() + (lines === 1 ? " line" : " lines");
  // Large sources remain complete; avoid manufacturing thousands of gutter nodes.
  state.gutter.hidden = !!patch || lines > 1000;
  const numbers = lines > 1000 ? "" : Array.from({ length: lines }, (_, i) => i + 1).join("\n");
  if (state.gutter.dataset.lines !== numbers) state.gutter.dataset.lines = numbers;
  state.balance.hidden = !patch || !(patch.added + patch.removed);
  state.removed.style.flexGrow = String(patch?.removed ?? 0); state.added.style.flexGrow = String(patch?.added ?? 0);
  state.overview.hidden = !!patch || lines < 24 || lines > 1000 || text.length > 65536;
  if (!state.overview.hidden) {
    const rows = text.split("\n"), stride = Math.ceil(lines / 120), marks = [];
    for (let index = 0; index < lines; index += stride) {
      const row = rows[index], indent = row.length - row.trimStart().length;
      const mark = figure.ownerDocument.createElement("i");
      mark.style.top = (index / lines * 100) + "%";
      mark.style.left = Math.min(35, indent / 120 * 100) + "%";
      mark.style.width = Math.min(100 - Math.min(35, indent / 120 * 100), row.trim().length / 120 * 100) + "%";
      marks.push(mark);
    }
    state.overview.replaceChildren(...marks);
  } else state.overview.replaceChildren();
  state.button.disabled = !copy || !text;
  state.button.onclick = () => copy?.(text);
}