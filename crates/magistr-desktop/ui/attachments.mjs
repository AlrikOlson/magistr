// The visible files are a projection of the selected conversation's staged content.
export function inputIdentity(input) {
  return JSON.stringify([input.text, input.attachments.map(file =>
    [file.id, file.name, file.content_hash])]);
}
export const sameDraft = (left, right) => inputIdentity(left) === inputIdentity(right);
export const sizeLabel = bytes => bytes < 1024 ? bytes + " B" :
  bytes < 1024 * 1024 ? Math.ceil(bytes / 1024) + " KB" : (bytes / (1024 * 1024)).toLocaleString(undefined, {maximumFractionDigits: 1}) + " MB";

export function clipboardFiles(event, limits, current = []) {
  const files = Array.from(event.clipboardData?.files ?? []);
  if (!files.length) return [];
  event.preventDefault();
  if (!limits) throw new Error("File selection is not ready. Try again.");
  if (files.length + current.length > limits.count) throw new Error("Add up to " + limits.count + " files to one message.");
  const large = files.find(file => file.size > limits.file_bytes);
  if (large) throw new Error("Cannot attach " + large.name + ": files must be " + sizeLabel(limits.file_bytes) + " or smaller.");
  if (files.reduce((size, file) => size + file.size, 0) + current.reduce((size, file) => size + file.size, 0) > limits.total_bytes) {
    throw new Error("The files exceed " + sizeLabel(limits.total_bytes) + ". Remove a file and try again.");
  }
  return files;
}

export class AttachmentList {
  constructor(root, remove, focusFallback) {
    this.root = root; this.document = root.ownerDocument;
    this.remove = remove; this.focusFallback = focusFallback;
    this.files = []; this.rows = new Map();
  }
  input(text) {
    return {text, attachments: this.files.map(({id, name, content_hash}) => ({id, name, content_hash}))};
  }
  clear() {this.files=[];this.rows.clear();this.root.replaceChildren();this.root.hidden=true;}
  apply(files, disabled = false) {
    const previous = new Map(this.files.map(file => [file.id, file]));
    this.files = files.map(file => ({...file,
      preview: file.preview ?? previous.get(file.id)?.preview,
      excerpt: file.excerpt ?? previous.get(file.id)?.excerpt
    }));
    const retained = new Set(this.files.map(file => file.id));
    for (const [id, row] of this.rows) if (!retained.has(id)) {
      const focused = row.item.contains(this.document.activeElement);
      row.item.remove(); this.rows.delete(id);
      if (focused) this.focusFallback?.();
    }
    this.files.forEach((file, index) => {
      let row = this.rows.get(file.id);
      if (!row) {
        const item = this.document.createElement("div"); item.className = "attachment"; item.setAttribute("role", "listitem");
        const content = this.document.createElement("div"); content.className = "attachment-content";
        const name = this.document.createElement("strong"), detail = this.document.createElement("small");
        name.textContent = file.name;
        detail.textContent = sizeLabel(file.size) + " · " + (file.path ?? file.source ?? "Pasted file");
        content.append(name, detail);
        if (file.preview) {
          const image = this.document.createElement("img"); image.src = file.preview; image.alt = "Preview of " + file.name;
          item.append(image);
        } else if (file.excerpt !== undefined) {
          const preview = this.document.createElement("details"), summary = this.document.createElement("summary"), excerpt = this.document.createElement("pre");
          summary.textContent = "Preview"; summary.setAttribute("aria-label", "Preview " + file.name);
          excerpt.textContent = file.excerpt || "This file is empty."; preview.append(summary, excerpt); content.append(preview);
        }
        item.append(content);
        let remove=null;
        if(this.remove) {
          remove=this.document.createElement("button"); remove.type="button"; remove.textContent="Remove";
          remove.setAttribute("aria-label","Remove "+file.name); remove.onclick=()=>this.remove(file.id); item.append(remove);
        }
        row={item,remove}; this.rows.set(file.id,row);
      }
      if(row.remove)row.remove.disabled = disabled;
      if (this.root.children[index] !== row.item) this.root.insertBefore(row.item, this.root.children[index] ?? null);
    });
    this.root.hidden = !this.files.length;
  }
}
