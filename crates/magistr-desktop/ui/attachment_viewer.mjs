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
    this.scaleLabel=document.getElementById("attachment-scale");
    this.zoomIn=document.getElementById("attachment-zoom-in");this.zoomOut=document.getElementById("attachment-zoom-out");
    this.size.onclick=()=>this.setZoom(this.scale?0:1);
    this.zoomIn.onclick=()=>this.setZoom(this.scale?Math.min(4,this.scale*2):1);
    this.zoomOut.onclick=()=>this.setZoom(this.scale?Math.max(.25,this.scale/2):.5);
    this.backgrounds=[...this.dialog.querySelectorAll("[data-background]")];
    for(const button of this.backgrounds)button.onclick=()=>{
      this.dialog.dataset.background=button.dataset.background;
      for(const choice of this.backgrounds)choice.setAttribute("aria-pressed",String(choice===button));
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
      this.setZoom(0);
      this.status.hidden = true;
      this.image.hidden = false;
      this.metadata.textContent = this.file.size.toLocaleString() + " bytes · " + this.image.naturalWidth + " × " + this.image.naturalHeight;
    };
    this.image.onerror = () => {
      if (!this.file) return;
      this.image.hidden = true; this.size.disabled = true; this.zoomIn.disabled=true;this.zoomOut.disabled=true;
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
    this.setZoom(0);this.size.disabled = true;this.zoomIn.disabled=true;this.zoomOut.disabled=true;
    this.status.textContent = "Loading image…"; this.status.hidden = false;
    this.image.hidden = true; this.image.alt = file.name; this.image.src = file.preview;
    this.dialog.showModal();
    this.document.getElementById("attachment-close").focus();
  }
  setZoom(scale){
    this.scale=scale;this.dialog.dataset.actual=String(scale>0);
    this.image.style.width=scale?this.image.naturalWidth*scale+"px":"";
    this.image.style.height=scale?this.image.naturalHeight*scale+"px":"";
    this.scaleLabel.textContent=scale?Math.round(scale*100)+"%":"Fit";
    this.size.textContent=scale?"Fit image":"Actual size";this.size.disabled=false;
    this.zoomOut.disabled=scale===.25;this.zoomIn.disabled=scale===4;
    const stage=this.image.parentElement;stage.scrollTop=0;stage.scrollLeft=0;
  }
  removed(id, source) {
    if (this.dialog.open && this.source === source && (id === null || this.file?.id === id)) this.dialog.close();
  }
}
