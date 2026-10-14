import { addressKey } from "./context.mjs";
// Native selection and reading position belong to this semantic document view.
import { AttachmentList } from "./attachments.mjs";
const tags=new Set(["p","h1","h2","h3","h4","h5","h6","blockquote","pre","ol","ul","li","em","strong","s","a","span","code","hr","br"]);
const element=(document,tag,value="")=>{const node=document.createElement(tag);node.textContent=value;return node;};
export function selectedWithin(selection,node) {
  if(!selection||selection.isCollapsed||!selection.rangeCount)return false;
  for(let index=0;index<selection.rangeCount;index++)if(selection.getRangeAt(index).intersectsNode(node))return true;
  return false;
}
export function nearBottom(node) {return node.scrollHeight-node.scrollTop-node.clientHeight<32;}
export function allowedDestination(value) {
  try {const url=new URL(value);return ["http:","https:"].includes(url.protocol)&&!url.username&&!url.password&&!/[\u0000-\u001f\u007f]/.test(value)&&value.trim()===value;}catch{return false;}
}

export function renderDocument(parent,nodes,openLink) {
  const document=parent.ownerDocument;
  nodes.forEach((source,index)=>{
    const tag=source.tag==="text"?"text":tags.has(source.tag)?source.tag:"span";
    let node=parent.childNodes[index];
    if(!node||(tag==="text"?node.nodeType!==3:node.localName!==tag)) {
      const next=tag==="text"?document.createTextNode(""):document.createElement(tag);
      if(node)node.replaceWith(next);else parent.append(next);node=next;
    }
    if(tag==="text") {if(node.data!==source.text)node.data=source.text;return;}
    node.dataset.source=String(source.start);
    if(tag==="a") {
      const safe=allowedDestination(source.destination??"");
      if(safe) {node.href=source.destination;node.title=source.destination;node.rel="noreferrer noopener";
        node.onclick=event=>{event.preventDefault();openLink(source.destination);};
        node.onauxclick=event=>event.preventDefault();
      } else {node.removeAttribute("href");node.onclick=null;}
    }
    if(tag==="ol"&&source.number!==null)node.start=source.number;
    if(source.text) {
      if(node.textContent!==source.text)node.textContent=source.text;
    } else renderDocument(node,source.children??[],openLink);
  });
  while(parent.childNodes.length>nodes.length)parent.lastChild.remove();
}
function place(parent,nodes) {
  nodes.forEach((node,index)=>{if(parent.children[index]!==node)parent.insertBefore(node,parent.children[index]??null);});
  while(parent.children.length>nodes.length)parent.lastChild.remove();
}

export class Transcript {
  constructor(root,latest,{copy,openLink}) {
    this.root=root;this.latest=latest;this.document=root.ownerDocument;this.copy=copy;this.openLink=openLink;
    this.rows=new Map();this.address="";this.view=null;this.pending=false;this.following=true;
    this.gap=element(this.document,"p","Some earlier messages are unavailable.");this.gap.className="gap";this.gap.setAttribute("role","status");
    this.empty=element(this.document,"div");this.empty.className="empty";
    this.empty.append(element(this.document,"h2","What would you like to work on?"),element(this.document,"p","Send a message to start a conversation in this working copy."));
    this.document.addEventListener("selectionchange",()=>{
      if(selectedWithin(this.document.getSelection(),this.root))this.following=false;
      if(this.pending&&!selectedWithin(this.document.getSelection(),this.root)) {
        this.pending=false;this.render(this.view);
      }
    });
    root.addEventListener("scroll",()=>{this.following=nearBottom(root)&&!selectedWithin(this.document.getSelection(),root);latest.hidden=nearBottom(root);},{passive:true});
    latest.onclick=()=>this.resumeLatest();
  }
  resumeLatest() {
    this.document.getSelection()?.removeAllRanges();this.pending=false;this.following=true;
    this.render(this.view);this.root.scrollTop=this.root.scrollHeight;this.latest.hidden=true;
  }
  clear() {this.rows.clear();this.root.replaceChildren();this.address="";this.view=null;this.pending=false;this.following=true;}
  anchor() {
    const top=this.root.getBoundingClientRect().top;
    const candidates=[...this.root.querySelectorAll("article [data-reading]"),...this.root.querySelectorAll("article[data-reading]")];
    const node=candidates.find(node=>node.getBoundingClientRect().bottom>top);
    return node?{key:node.dataset.reading,offset:node.getBoundingClientRect().top-top}:null;
  }
  render(view) {
    if(!view)return;
    const address=JSON.stringify([addressKey(view.address),view.target?.conversation]),fresh=address!==this.address;
    if(fresh) {this.clear();this.address=address;}
    this.view=view;
    const selection=this.document.getSelection(),selected=selectedWithin(selection,this.root);
    if(selected)this.following=false;
    const follow=(fresh||this.following&&nearBottom(this.root))&&!selected;
    const anchor=follow?null:this.anchor(),previousTop=this.root.scrollTop;
    const children=view.dropped?[this.gap]:[];
    if(!view.messages.length&&!view.dropped)children.push(this.empty);
    const retained=new Set(view.messages.map(message=>message.id));
    for(const [id,row] of this.rows)if(!retained.has(id)) {
      if(selectedWithin(selection,row.article))selection.removeAllRanges();
      this.rows.delete(id);row.article.remove();
    }
    for(const message of view.messages) {
      let row=this.rows.get(message.id);
      if(!row) {row=this.create(message);this.rows.set(message.id,row);}
      row.message=message;
      if(row.revision!==message.revision) {
        if(selectedWithin(selection,row.article))this.pending=true;
        else {this.update(row,message);row.revision=message.revision;}
      }
      children.push(row.article);
    }
    place(this.root,children);
    if(follow)this.root.scrollTop=this.root.scrollHeight;
    else {
      const node=anchor&&[...this.root.querySelectorAll("[data-reading]")].find(node=>node.dataset.reading===anchor.key);
      this.root.scrollTop=node?this.root.scrollTop+node.getBoundingClientRect().top-this.root.getBoundingClientRect().top-anchor.offset:previousTop;
    }
    this.latest.hidden=nearBottom(this.root);
  }
  create(message) {
    const article=element(this.document,"article");article.className=message.role.toLowerCase();article.dataset.message=String(message.id);
    article.setAttribute("aria-label",message.role==="Agent"?"Agent reply":message.tool?"Tool activity":message.role);
    article.dataset.reading=String(message.id);
    const header=element(this.document,"header"),role=element(this.document,"span",message.role);role.className="message-role";
    const copy=element(this.document,"button",message.tool?"Copy tool details":message.role==="Agent"?"Copy reply":"Copy message");
    const row={article,copy,message,revision:null};copy.onclick=()=>this.copy(row.message.text);
    header.append(role,copy);article.append(header);
    if(message.tool) {
      row.details=element(this.document,"details");row.summary=element(this.document,"summary");row.body=element(this.document,"div");row.body.className="tool-body";
      row.details.append(row.summary,row.body);article.append(row.details);
    } else {
      row.body=element(this.document,"div");row.body.className="message-body";article.append(row.body);
      const files=element(this.document,"div");files.className="attachments";files.setAttribute("role","list");files.setAttribute("aria-label","Attached files");article.append(files);
      row.attachments=new AttachmentList(files);
    }
    return row;
  }
  update(row,message) {
    row.copy.disabled=!message.text;
    if(message.tool) {
      const tool=message.tool;
      row.summary.textContent=tool.title+" · "+tool.status;
      row.article.classList.toggle("failed",tool.status==="Failed");
      const specs=tool.details.map((detail,index)=>({
        tag:"section",key:index,label:detail.label,source:detail.source
      }));
      specs.forEach((detail,index)=>{
        let section=row.body.children[index];
        if(!section||section.localName!=="section") {const next=element(this.document,"section");next.append(element(this.document,"h3"),element(this.document,"pre"));if(section)section.replaceWith(next);else row.body.append(next);section=next;}
        section.dataset.reading=message.id+":tool:"+index;
        if(section.children[0].textContent!==detail.label)section.children[0].textContent=detail.label;
        if(section.children[1].textContent!==detail.source)section.children[1].textContent=detail.source;
      });
      while(row.body.children.length>specs.length)row.body.lastChild.remove();
      if(tool.truncated_from) {
        const note=element(this.document,"p","Some tool details are unavailable. Original size: "+tool.truncated_from+" bytes.");note.className="gap";row.body.append(note);
      }
      if(tool.duration_ms!==null)row.body.append(element(this.document,"p","Duration: "+(tool.duration_ms/1000).toLocaleString(undefined,{maximumFractionDigits:2})+" seconds."));
      if(tool.image_count)row.body.append(element(this.document,"p",tool.image_count+" image"+(tool.image_count===1?"":"s")+" attached."));
    } else {
      row.attachments.apply(message.attachments??[]);
      const nodes=message.document?.nodes??[{tag:"p",text:message.text,children:[],start:0}];
      renderDocument(row.body,nodes,this.openLink);
      for(const block of row.body.children)block.dataset.reading=message.id+":"+block.dataset.source;
      if(message.document?.limited) {
        const note=element(this.document,"p","This reply is shown as plain text because it contains extensive formatting.");note.className="gap";row.body.append(note);
      }
    }
  }
}
