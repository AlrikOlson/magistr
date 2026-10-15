import { ProjectOpening } from "./projects.mjs";
import { patchLines } from "./patch_document.mjs";
import { paintSource } from "./source_color.mjs";

const node=(tag,value,className="")=>{const element=document.createElement(tag);element.textContent=value;element.className=className;return element;};


export function sourceLines(value,diff=false) {
  if(diff){
    const patch=patchLines(value);
    return patch?patch.rows.map(row=>({text:row.text,before:row.old===""?null:row.old,after:row.next===""?null:row.next,kind:["added","removed","hunk"].includes(row.kind)?"diff-"+row.kind:""})):[{text:value,before:null,after:null,kind:""}];
  }
  const lines=value.match(/[^\n]*\n|[^\n]+$/g)??[];
  return lines.length>2000?[{text:value,before:null,after:null,kind:""}]:lines.map((text,index)=>({text,before:null,after:index+1,kind:""}));
}
function sourceView(value,diff=false,language="") {
  const pre=node("pre","","setup-source"+(diff?" setup-diff":""));
  pre.tabIndex=0;pre.setAttribute("aria-label",diff?"File changes":"File contents");
  for(const row of sourceLines(value,diff)) {
    const line=node("span","","source-line "+row.kind);
    for(const number of diff?[row.before,row.after]:[row.after]) {
      const gutter=node("span","","source-number");gutter.dataset.line=number??"";gutter.setAttribute("aria-hidden","true");line.append(gutter);
    }
    const source=node("span",row.text,"source-text");if(!diff)paintSource(source,row.text,language);
    line.append(source);pre.append(line);
  }
  return pre;
}

// Presentation and focus only. ProjectOpening owns the review lifetime and Rust owns every write.
export class ProjectPicker {
  constructor(invoke,element,{open,opened,closed}) {
    this.invoke=invoke;this.$=element;this.closed=closed;this.review=null;this.rows=[];this.returnFocus=null;
    this.flow=new ProjectOpening(invoke,{open,opened:async address=>{
      await opened(address);this.$("project-picker").close();this.$("draft").focus();
    },changed:state=>this.render(state)});
    element("browse-folder").onclick=()=>this.browse();
    element("picker-cancel").onclick=()=>this.close();
    element("change-folder").onclick=()=>this.changeFolder();
    element("project-picker").oncancel=event=>{event.preventDefault();this.close();};
    element("project-form").onsubmit=event=>{event.preventDefault();this.submit();};
    for(const [id,value] of [["select-files",true],["clear-files",false]])element(id).onclick=()=>{
      for(const row of this.rows)if(row.check)row.check.checked=value;
      this.selection();
    };
  }
  show(directory="") {
    if(this.$("project-picker").open)return;
    this.returnFocus=document.activeElement===document.body?this.$("open-project"):document.activeElement;
    this.flow.emit({stage:"idle",directory:directory||this.flow.state.directory,preview:null,result:null,error:null});
    if(directory)this.$("project-folder").value=directory;
    this.$("project-picker").showModal();this.$("project-folder").focus();
  }
  async browse() {
    this.$("browse-folder").disabled=true;
    const intent=this.flow.intent;
    try {
      const directory=await this.invoke("pick_folder");
      if(directory&&this.$("project-picker").open&&intent===this.flow.intent)await this.flow.choose(directory);
    }catch(error){if(intent===this.flow.intent)this.flow.fail(error,"Couldn’t open the folder picker");}
    finally{this.render(this.flow.state);}
  }
  async close() {
    if(["applying","opening"].includes(this.flow.state.stage))return;
    try {
      await this.flow.cancel();this.$("project-picker").close();this.closed();
      this.returnFocus?.focus();
    }catch(error){this.error(error);}
  }
  async changeFolder() {
    try {
      await this.flow.cancel();
      this.flow.emit({stage:"idle",result:null});
      this.$("project-folder").focus();
    }catch(error){this.error(error);}
  }
  async submit() {
    const state=this.flow.state;
    if(["reading","applying","opening"].includes(state.stage))return;
    if(state.stage==="review")await this.flow.apply(this.rows.flatMap((row,index)=>row.check?.checked?[index]:[]));
    else {
      const directory=this.$("project-folder").value.trim();
      await this.flow.choose(directory,!!(state.preview?.kind==="setup"&&!state.result?.complete));
    }
  }
  error(error) {this.flow.fail(error,"Couldn’t close the setup review");}
  renderError(state) {
    const detail=state.error??"",long=detail.length>240||detail.includes("\n");
    const changed=detail!==this.errorDetail;
    this.errorDetail=detail;
    this.$("picker-failure").hidden=!detail;
    this.$("picker-error").textContent=state.errorTitle??"Couldn’t open this project";
    this.$("picker-error-message").hidden=long;
    this.$("picker-error-message").textContent=long?"":detail;
    this.$("picker-error-details").hidden=!long;
    this.$("picker-error-source").textContent=long?detail:"";
    if(changed)this.$("picker-error-details").open=false;
    this.$("picker-error-help").hidden=!long;
  }
  render(state) {
    const busy=["reading","applying","opening"].includes(state.stage),review=state.preview?.kind==="setup"&&!state.result;
    const reviewing=review||!!state.result;
    this.$("project-picker").classList.toggle("has-review",reviewing);
    this.$("project-form").setAttribute("aria-busy",String(busy));
    this.$("picker-title").textContent=reviewing?"Review project setup":"Open a project";
    this.$("picker-intro").hidden=!reviewing;
    this.$("picker-intro").textContent=reviewing?"Review each change, then apply the selected files.":"";
    this.$("picker-folder-entry").hidden=reviewing;
    this.$("picker-destination").hidden=!reviewing;
    this.$("setup-root").textContent=state.preview?.setup_directory??state.directory;
    this.$("setup-preview").hidden=!review;
    this.$("setup-result").hidden=!state.result;
    this.renderError(state);
    const status={reading:"Checking the folder…",applying:"Applying selected files…",opening:"Opening your conversation…",partial:"Some changes still need attention. Review the folder again to continue."}[state.stage]??"";
    this.$("picker-status").textContent=status;this.$("picker-status").hidden=!status;
    for(const id of ["browse-folder","project-folder","picker-submit","change-folder","select-files","clear-files"])this.$(id).disabled=busy;
    this.$("picker-cancel").disabled=["applying","opening"].includes(state.stage);
    this.$("picker-submit").textContent=state.stage==="review"?"Apply selected files":busy?"Please wait…":state.error?"Try again":reviewing?"Review again":"Open project";
    if(state.directory)this.$("project-folder").value=state.directory;
    if(review&&state.preview.review!==this.review)this.files(state.preview);
    for(const row of this.rows)if(row.check)row.check.disabled=busy;
    this.selection();
    if(state.result) {
      this.$("setup-result-title").textContent=state.result.complete?"Setup complete":"Setup needs attention";
      this.$("setup-record").textContent=state.result.setup_record;
      const outcomes={applied:"Applied",skipped:"Kept",refused:"Cannot change",failed:"Failed"};
      this.$("setup-outcomes").replaceChildren(...state.result.files.map(file=>{
        const row=node("li","");row.dataset.outcome=file.outcome;row.append(node("strong",file.path),node("span",outcomes[file.outcome]+(file.detail?" · "+file.detail:"")));return row;
      }));
    }
    if(["review","partial"].includes(state.stage)){this.$("picker-title").focus();}
    if(state.error)this.$("picker-error").focus();
  }
  selection() {
    const total=this.rows.filter(row=>row.check).length,count=this.rows.filter(row=>row.check?.checked).length;
    this.$("setup-count").hidden=this.$("setup-preview").hidden;
    this.$("setup-count").textContent=count+" of "+total+" changes selected";
  }
  files(preview) {
    this.review=preview.review;
    this.rows=preview.files.map((file,index)=>{
      const row=node("div","","setup-file"),button=node("button","","setup-file-open");button.type="button";
      row.dataset.action=file.action;
      const slash=file.path.lastIndexOf("/");
      const path=node("span","","setup-file-name");
      if(slash>=0)path.append(node("small",file.path.slice(0,slash+1),"setup-file-directory"));
      path.append(node("strong",file.path.slice(slash+1)));button.append(path,node("small",file.action,"setup-file-action"));
      button.setAttribute("aria-label","Read "+file.path);
      button.onclick=()=>this.readFile(index);
      let check=null;
      if(file.selectable) {
        check=document.createElement("input");check.type="checkbox";check.checked=true;check.dataset.index=String(index);
        check.setAttribute("aria-label","Include "+file.path);check.onchange=()=>this.selection();row.append(check);
      }else row.append(node("span","—","setup-kept"));
      row.append(button);return {row,button,check,file};
    });
    this.$("setup-files").replaceChildren(...this.rows.map(row=>row.row));
    this.readFile(0);
  }
  readFile(index) {
    for(const [i,row] of this.rows.entries())row.button.setAttribute("aria-current",String(i===index));
    const file=this.rows[index]?.file,body=this.$("setup-change");
    body.replaceChildren();body.scrollTop=0;
    if(!file){body.append(node("p","No file changes are needed."));return;}
    const heading=node("header","","setup-change-heading");
    const directory=file.path.slice(0,file.path.lastIndexOf("/")+1);
    if(directory)heading.append(node("p",directory,"setup-file-path"));
    heading.append(node("h3",file.path.split("/").at(-1)),node("p",file.action+" · "+file.ownership,"setup-change-meta"));body.append(heading);
    if(file.reason)body.append(node("p",file.reason,"setup-reason"));
    const tabs=node("div","","setup-source-modes");tabs.setAttribute("role","group");tabs.setAttribute("aria-label","File view");
    const modes=[];
    if(file.diff)modes.push(["changes","Changes"]);
    if(file.before!==null&&file.before!==undefined)modes.push(["before","Current file"]);
    if(file.selectable&&file.action!=="Remove file")modes.push(["after","After setup"]);
    const sheet=node("figure","","setup-code-sheet");
    const choices=modes.map(([mode,label])=>{const button=node("button",label);button.type="button";button.onclick=()=>show(mode);tabs.append(button);return {button,mode};});
    const show=mode=>{
      for(const choice of choices)choice.button.setAttribute("aria-pressed",String(choice.mode===mode));
      const caption=node("figcaption","");sheet.replaceChildren(caption);
      if(mode==="changes"){
        caption.append(node("span","Changes"));
        const patch=patchLines(file.diff);
        if(patch){const counts=node("span","","setup-delta");counts.append(node("span","+"+patch.added,"added"),node("span","−"+patch.removed,"removed"));counts.setAttribute("aria-label",patch.added+" lines added, "+patch.removed+" lines removed");caption.append(counts);}
        sheet.append(sourceView(file.diff,true));
      }else{
        const value=mode==="before"?file.before:file.after;
        caption.append(node("span",mode==="before"?"Current contents":"Contents after setup"));
        sheet.append(value?sourceView(value,false,file.path.split(".").at(-1)):node("p","This file is empty."));
      }
    };
    body.append(tabs,sheet);show(modes[0]?.[0]??"before");
  }
}
