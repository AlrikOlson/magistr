// One ephemeral image inspection surface; attachment bytes remain with their owner.
const viewers = new WeakMap();
export function attachmentViewer(document) {
  if (!viewers.has(document)) viewers.set(document, new AttachmentViewer(document));
  return viewers.get(document);
}
export function releaseAttachmentPreview(document, source, id = null) {
  viewers.get(document)?.removed(id, source);
}
class AttachmentViewer {
  constructor(document) {
    this.document = document;
    this.dialog = document.getElementById("attachment-viewer");
    this.image = document.getElementById("attachment-image");
    this.status = document.getElementById("attachment-image-status");
    this.size = document.getElementById("attachment-size");
    document.getElementById("attachment-close").onclick = () => this.dialog.close();
    this.size.onclick = () => {
      const actual = this.dialog.dataset.actual !== "true";
      this.dialog.dataset.actual = String(actual);
      this.size.textContent = actual ? "Fit image" : "Actual size";
    };
    this.dialog.addEventListener("close", () => {
      this.image.removeAttribute("src"); this.file = null; this.source = null;
      if (this.opener?.isConnected) this.opener.focus();
      else this.fallback?.();
      this.fallback = null;
      this.opener = null;
    });
    this.image.onload = () => {
      if (!this.file) return;
      this.size.disabled = false;
      this.status.hidden = true;
      this.image.hidden = false;
      this.metadata.textContent = this.file.size.toLocaleString() + " bytes · " + this.image.naturalWidth + " × " + this.image.naturalHeight;
    };
    this.image.onerror = () => {
      if (!this.file) return;
      this.image.hidden = true; this.size.disabled = true;
      this.status.textContent = "This image preview is unavailable."; this.status.hidden = false;
    };
    this.metadata = document.getElementById("attachment-metadata");
  }
  open(file, opener, source, fallback) {
    if (!file.preview || this.dialog.open) return;
    this.file = file; this.opener = opener; this.source = source; this.fallback = fallback;
    this.document.getElementById("attachment-title").textContent = file.name;
    this.document.getElementById("attachment-location").textContent = file.path ?? file.source ?? "Pasted image";
    this.metadata.textContent = file.size.toLocaleString() + " bytes";
    this.dialog.dataset.actual = "false"; this.size.textContent = "Actual size"; this.size.disabled = true;
    this.status.textContent = "Loading image…"; this.status.hidden = false;
    this.image.hidden = true; this.image.alt = file.name; this.image.src = file.preview;
    this.dialog.showModal();
    this.document.getElementById("attachment-close").focus();
  }
  removed(id, source) {
    if (this.dialog.open && this.source === source && (id === null || this.file?.id === id)) this.dialog.close();
  }
}
