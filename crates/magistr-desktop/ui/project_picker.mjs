import { ProjectOpening } from "./projects.mjs";

const node=(tag,value,className="")=>{const element=document.createElement(tag);element.textContent=value;element.className=className;return element;};

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
    }catch(error){if(intent===this.flow.intent)this.flow.emit({stage:"error",error:String(error)});}
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
  error(error) {this.$("picker-error").textContent=String(error);this.$("picker-error").hidden=false;this.$("picker-error").focus();}
  render(state) {
    const busy=["reading","applying","opening"].includes(state.stage),review=state.preview?.kind==="setup"&&!state.result;
    const reviewing=review||!!state.result;
    this.$("project-picker").classList.toggle("has-review",reviewing);
    this.$("project-form").setAttribute("aria-busy",String(busy));
    this.$("picker-title").textContent=reviewing?"Review project setup":"Open a project";
    this.$("picker-intro").textContent=reviewing?"Choose the files to change. Read each change before applying it.":"Choose the folder you want to work in.";
    this.$("picker-folder-entry").hidden=reviewing;
    this.$("picker-destination").hidden=!reviewing;
    this.$("setup-root").textContent=state.preview?.setup_directory??state.directory;
    this.$("setup-preview").hidden=!review;
    this.$("setup-result").hidden=!state.result;
    this.$("picker-error").textContent=state.error??"";this.$("picker-error").hidden=!state.error;
    const status={reading:"Checking the folder…",applying:"Applying selected files…",opening:"Opening your conversation…",partial:"Some changes still need attention. Review the folder again to continue."}[state.stage]??"";
    this.$("picker-status").textContent=status;this.$("picker-status").hidden=!status;
    for(const id of ["browse-folder","project-folder","picker-submit","change-folder","select-files","clear-files"])this.$(id).disabled=busy;
    this.$("picker-cancel").disabled=["applying","opening"].includes(state.stage);
    this.$("picker-submit").textContent=state.stage==="review"?"Apply selected files":busy?"Please wait…":reviewing?"Review again":"Open project";
    if(state.directory)this.$("project-folder").value=state.directory;
    if(review&&state.preview.review!==this.review)this.files(state.preview);
    for(const row of this.rows)if(row.check)row.check.disabled=busy;
    this.selection();
    if(state.result) {
      this.$("setup-result-title").textContent=state.result.complete?"Setup complete":"Setup needs attention";
      this.$("setup-record").textContent=state.result.setup_record;
      const outcomes={applied:"Applied",skipped:"Kept",refused:"Cannot change",failed:"Failed"};
      this.$("setup-outcomes").replaceChildren(...state.result.files.map(file=>{
        const row=node("li","");row.append(node("strong",file.path),node("span",outcomes[file.outcome]+(file.detail?" · "+file.detail:"")));return row;
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
      button.append(node("span",file.path),node("small",file.action));
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
    body.append(node("h3",file.path),node("p",file.action+" · "+file.ownership,"setup-change-meta"));
    if(file.reason)body.append(node("p",file.reason,"setup-reason"));
    if(file.diff) {
      body.append(node("h4","Changes"));
      const pre=node("pre","","setup-diff");
      for(const line of file.diff.split("\n"))pre.append(node("span",line+"\n",line.startsWith("+")?"diff-added":line.startsWith("-")?"diff-removed":""));
      body.append(pre);
    } else {
      const value=file.selectable?(file.action==="Remove file"?file.before:file.after):(file.before??file.after);
      body.append(node("h4",file.selectable?(file.action==="Remove file"?"Contents to remove":"Contents after setup"):"Current contents"));
      body.append(node("pre",value||"This file is empty."));
    }
  }
}
