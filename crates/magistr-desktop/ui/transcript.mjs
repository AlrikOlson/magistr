import { renderToolDetail } from "./tool_result.mjs";
import { renderCodeBlock } from "./code_blocks.mjs";
import { targetKey } from "./context.mjs";
import { remember, captureSelection, restoreSelection } from "./reading.mjs";
// Native selection and reading position belong to this semantic document view.
import { AttachmentList } from "./attachments.mjs";
const tags=new Set(["p","h1","h2","h3","h4","h5","h6","blockquote","pre","ol","ul","li","em","strong","s","a","span","code","hr","br","table","thead","tbody","tr","th","td"]);
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

export function renderDocument(parent,nodes,openLink,copy) {
  const document=parent.ownerDocument;
  nodes.forEach((source,index)=>{
    const tag=source.tag==="text"?"text":tags.has(source.tag)?source.tag:"span";
    const hostTag=tag==="pre"?"figure":tag==="table"?"div":tag;
    let node=parent.childNodes[index];
    if(!node||(tag==="text"?node.nodeType!==3:node.localName!==hostTag)) {
      const next=tag==="text"?document.createTextNode(""):document.createElement(hostTag);
      if(node)node.replaceWith(next);else parent.append(next);node=next;
    }
    if(tag==="text") {if(node.data!==source.text)node.data=source.text;return;}
    node.dataset.source=String(source.start);
    if(tag==="pre"){renderCodeBlock(node,source,copy);return;}
    if(tag==="table"){
      node.className="table-document";node.tabIndex=0;node.setAttribute("role","region");node.setAttribute("aria-label","Table");
      let table=node.firstElementChild;
      if(!table){table=document.createElement("table");node.append(table);}
      renderDocument(table,source.children??[],openLink,copy);return;
    }
    if(tag==="th")node.scope="col";
    if(tag==="th"||tag==="td")node.style.textAlign=["left","center","right"].includes(source.alignment)?source.alignment:"";
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
    } else renderDocument(node,source.children??[],openLink,copy);
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
    this.places=new Map();this.selectedPlace=null;this.readingAnchor=null;this.size=null;
    this.gap=element(this.document,"p","Some earlier messages are unavailable.");this.gap.className="gap";this.gap.setAttribute("role","status");
    this.empty=element(this.document,"div");this.empty.className="empty";
    const emptyIcon=element(this.document,"span");emptyIcon.className="empty-mark";emptyIcon.dataset.icon="chat";emptyIcon.setAttribute("aria-hidden","true");
    this.empty.append(emptyIcon,element(this.document,"h2","What would you like to work on?"),element(this.document,"p","Write a message below. Add files if they’ll help explain what you need."));
    this.document.addEventListener("selectionchange",()=>{
      if(selectedWithin(this.document.getSelection(),this.root)){this.following=false;this.selectedPlace=captureSelection(root);}
      else if(root.contains(this.document.activeElement))this.selectedPlace=null;
      if(this.pending&&!selectedWithin(this.document.getSelection(),this.root)) {
        this.pending=false;this.render(this.view);
      }
    });
    root.addEventListener("pointerdown",()=>{this.selectedPlace=null;});
    root.addEventListener("scroll",()=>{
      if(!root.clientHeight||!this.sameWidth())return;
      this.following=nearBottom(root)&&!selectedWithin(this.document.getSelection(),root);
      this.readingAnchor=this.anchor();latest.hidden=nearBottom(root);
    },{passive:true});
    this.resize=new ResizeObserver(()=>{
      if(this.resizeFrame)return;
      this.resizeFrame=requestAnimationFrame(()=>{
        this.resizeFrame=null;
        if(!root.clientHeight||!root.clientWidth||!this.view||this.sameSize())return;
        if(this.selectedPlace&&!selectedWithin(this.document.getSelection(),root))this.selectedPlace=restoreSelection(root,this.selectedPlace)?this.selectedPlace:null;
        // Height-only changes do not reflow text; keep a scroll made before this frame.
        const anchor=this.sameWidth()?this.anchor():this.readingAnchor;
        this.restoreAnchor(this.following?null:anchor,this.following);
        this.size=this.dimensions();this.readingAnchor=this.anchor();latest.hidden=nearBottom(root);
      });
    });
    this.resize.observe(root);
    latest.onclick=()=>this.resumeLatest();
  }
  resumeLatest() {
    this.document.getSelection()?.removeAllRanges();this.selectedPlace=null;this.pending=false;this.following=true;
    this.render(this.view);this.root.scrollTop=this.root.scrollHeight;this.latest.hidden=true;
  }
  dimensions(){return {width:this.root.clientWidth,height:this.root.clientHeight};}
  sameWidth(){return this.size?.width===this.root.clientWidth;}
  sameSize(){return this.sameWidth()&&this.size.height===this.root.clientHeight;}
  remember() {
    if(!this.address||(this.targets&&!this.targets.has(this.address)))return;
    remember(this.places,this.address,{anchor:this.root.clientHeight?this.anchor():this.readingAnchor,following:this.following,
      selection:captureSelection(this.root)??this.selectedPlace,open:[...this.rows].filter(([,row])=>row.details?.open).map(([id])=>id)});
  }
  clear() {
    this.remember();
    if(selectedWithin(this.document.getSelection(),this.root))this.document.getSelection().removeAllRanges();
    this.rows.clear();this.root.replaceChildren();this.address="";this.view=null;this.pending=false;this.following=true;
    this.selectedPlace=null;this.readingAnchor=null;this.size=null;
  }
  retain(targets){this.targets=targets;for(const key of this.places.keys())if(!targets.has(key))this.places.delete(key);}
  restoreAnchor(anchor,following=false,previousTop=this.root.scrollTop) {
    if(following){this.root.scrollTop=this.root.scrollHeight;return;}
    const node=anchor&&[...this.root.querySelectorAll("[data-reading]")].find(node=>node.dataset.reading===anchor.key);
    this.root.scrollTop=node?this.root.scrollTop+node.getBoundingClientRect().top-this.root.getBoundingClientRect().top-anchor.offset:previousTop;
  }
  anchor() {
    const top=this.root.getBoundingClientRect().top;
    const candidates=[...this.root.querySelectorAll("article [data-reading]"),...this.root.querySelectorAll("article[data-reading]")];
    const node=candidates.find(node=>node.getBoundingClientRect().bottom>top);
    return node?{key:node.dataset.reading,offset:node.getBoundingClientRect().top-top}:null;
  }
  render(view) {
    if(!view)return;
    const address=targetKey(view.target),fresh=address!==this.address;
    if(fresh) {this.clear();this.address=address;}
    const saved=fresh?this.places.get(address):null;
    if(saved)this.following=saved.following;
    this.view=view;
    const selection=this.document.getSelection(),selected=selectedWithin(selection,this.root);
    if(selected)this.following=false;
    const visible=this.root.clientHeight>0;
    const follow=(fresh?!saved||saved.following:this.following&&(!visible||nearBottom(this.root)))&&!selected;
    const anchor=follow?null:saved?.anchor??(visible&&this.sameWidth()?this.anchor():this.readingAnchor),previousTop=this.root.scrollTop;
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
    if(saved) {
      for(const id of saved.open){const row=this.rows.get(id);if(row?.details)row.details.open=true;}
      this.selectedPlace=!visible||restoreSelection(this.root,saved.selection)?saved.selection:null;
      if(this.selectedPlace)this.following=false;
    }
    if(visible){this.restoreAnchor(anchor,follow,previousTop);this.size=this.dimensions();this.readingAnchor=this.anchor();}
    else{this.readingAnchor=anchor;this.size=null;this.following=follow;}
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
      row.toolTitle=element(this.document,"span");row.toolTitle.className="tool-title";
      row.toolStatus=element(this.document,"span");row.toolStatus.className="tool-status";
      row.toolDuration=element(this.document,"span");row.toolDuration.className="tool-duration";
      const separator=element(this.document,"span"," · ");separator.className="sr-only";
      row.summary.append(row.toolTitle,separator,row.toolStatus,row.toolDuration);
      article.dataset.tool="true";
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
      row.toolTitle.textContent=tool.title;row.toolStatus.textContent=tool.status;
      row.toolStatus.dataset.leadingIcon=tool.status==="Completed"?"check":tool.status==="Failed"?"close":"clock";
      row.article.dataset.status=tool.status.toLowerCase();
      row.toolDuration.textContent=tool.duration_ms==null?"":(tool.duration_ms/1000).toLocaleString(undefined,{maximumFractionDigits:2})+" s";
      row.toolDuration.hidden=tool.duration_ms==null;
      row.article.classList.toggle("failed",tool.status==="Failed");
      const specs=tool.details.map((detail,index)=>({
        tag:"section",key:index,label:detail.label,source:detail.source
      }));
      specs.forEach((detail,index)=>{
        let section=row.body.children[index];
        if(!section||section.localName!=="section") {const next=element(this.document,"section");if(section)section.replaceWith(next);else row.body.append(next);section=next;}
        section.dataset.reading=message.id+":tool:"+index;
        renderToolDetail(section,detail,this.copy);
      });
      while(row.body.children.length>specs.length)row.body.lastChild.remove();
      if(tool.truncated_from) {
        const note=element(this.document,"p","Some tool details are unavailable. Original size: "+tool.truncated_from+" bytes.");note.className="gap";row.body.append(note);
      }
      if(tool.image_count)row.body.append(element(this.document,"p",tool.image_count+" image"+(tool.image_count===1?"":"s")+" attached."));
    } else {
      row.attachments.apply(message.attachments??[]);
      const nodes=message.document?.nodes??[{tag:"p",text:message.text,children:[],start:0}];
      renderDocument(row.body,nodes,this.openLink,this.copy);
      for(const block of row.body.children)block.dataset.reading=message.id+":"+block.dataset.source;
      if(message.document?.limited) {
        const note=element(this.document,"p","This reply is shown as plain text because it contains extensive formatting.");note.className="gap";row.body.append(note);
      }
    }
  }
}