const text=(tag,value)=>{const node=document.createElement(tag);node.textContent=value;return node;};

// Dialog choices and read/admission lifetime. Rust still discovers agents and admits sessions.
export class ConversationPicker {
  constructor(invoke,element,{opened,rememberAgents}) {
    this.invoke=invoke;this.$=element;this.opened=opened;this.rememberAgents=rememberAgents;
    this.generation=0;this.stage="idle";this.choices=[];this.provider="";this.address=null;
    element("session-form").onsubmit=event=>{event.preventDefault();this.start();};
    element("session-picker-cancel").onclick=()=>this.close();
    element("session-picker").oncancel=event=>{event.preventDefault();this.close();};
    element("session-picker-retry").onclick=()=>this.agents();
    element("choose-initial-model").onclick=()=>this.models();
  }
  async show({directory,name,provider}) {
    this.directory=directory;this.preferred=provider;this.provider="";this.address=null;this.choices=[];
    this.$("session-destination-name").textContent=name;
    this.$("session-destination-path").textContent=directory;
    this.$("agent-choice").replaceChildren();this.clearModels();this.$("session-picker-error").hidden=true;
    if(!this.$("session-picker").open)this.$("session-picker").showModal();
    await this.agents();
  }
  close() {
    if(this.stage==="opening")return;
    ++this.generation;this.$("session-picker").close();this.$("new-session").focus();
  }
  clearModels() {
    this.$("initial-model-control").hidden=true;this.$("initial-model").replaceChildren();
    this.$("choose-initial-model").textContent="Choose a model…";
    this.$("model-choice-note").textContent="Use the default model, or check the available choices. Checking briefly connects to the agent.";
  }
  error(value) {
    this.$("session-picker-error").textContent=String(value);this.$("session-picker-error").hidden=false;
    this.$("session-picker-error").focus();
  }
  render() {
    const pending=this.stage==="opening",loading=this.stage==="agents",models=this.stage==="models";
    this.$("session-form").setAttribute("aria-busy",String(pending||loading||models));
    for(const input of this.$("agent-choice").querySelectorAll("input"))input.disabled=pending||!!this.address;
    this.$("choose-initial-model").disabled=pending||loading||models||!this.provider||!!this.address;
    this.$("initial-model").disabled=pending||models||!!this.address;
    this.$("session-picker-open").disabled=pending||loading||models||!this.provider;
    this.$("session-picker-cancel").disabled=pending;
    this.$("session-picker-retry").hidden=!!this.choices.length||loading;
    this.$("session-picker-open").textContent=pending?"Starting…":this.address?"Go to conversation":"Start conversation";
    this.$("session-picker-cancel").textContent=this.address?"Close":"Cancel";
    this.$("session-picker-status").textContent=loading?"Finding available agents…":models?"Checking available models…":pending?"Opening your conversation…":"";
    this.$("session-picker-status").hidden=!loading&&!models&&!pending;
    this.$("agent-choice-empty").hidden=loading||!!this.choices.length||!!this.discoveryFailed;
    this.$("session-model-section").hidden=!this.choices.length;
  }
  async agents() {
    const generation=++this.generation;
    this.stage="agents";this.discoveryFailed=false;this.$("session-picker-error").hidden=true;this.render();
    try {
      const choices=await this.invoke("agent_choices");
      if(generation!==this.generation)return;
      this.choices=choices;this.rememberAgents(choices);
      this.provider=choices.some(choice=>choice.id===this.preferred)?this.preferred:choices[0]?.id??"";
      this.$("agent-choice").replaceChildren(...choices.map(choice=>{
        const label=document.createElement("label"),input=document.createElement("input");
        input.type="radio";input.name="agent";input.value=choice.id;input.checked=choice.id===this.provider;
        input.onchange=()=>{++this.generation;this.provider=input.value;this.stage="ready";this.clearModels();this.$("session-picker-error").hidden=true;this.render();};
        const name=text("span",choice.label);name.className="agent-name";
        label.append(input,name);return label;
      }));
      this.stage="ready";this.render();
      (this.$("agent-choice").querySelector("input:checked")??this.$("session-picker-retry")).focus();
    }catch(error){if(generation===this.generation){this.discoveryFailed=true;this.stage="ready";this.render();this.error(error);}}
  }
  async models() {
    if(!this.provider||["models","agents","opening"].includes(this.stage))return;
    const generation=++this.generation,provider=this.provider,selected=this.$("initial-model").value;
    this.stage="models";this.$("session-picker-error").hidden=true;this.render();
    try {
      const models=await this.invoke("model_choices",{directory:this.directory,provider});
      if(generation!==this.generation)return;
      this.$("initial-model").replaceChildren(...[{id:"",name:"Use the default model"},...models].map(model=>{
        const option=text("option",model.name);option.value=model.id;return option;
      }));
      this.$("initial-model").value=models.some(model=>model.id===selected)?selected:"";
      this.$("initial-model-control").hidden=!models.length;
      this.$("choose-initial-model").textContent="Check models again";
      this.$("model-choice-note").textContent=models.length?"Choose a model for this conversation. You can change supported settings later.":"This agent returned no model choices. You can still start with its default model.";
      this.stage="ready";this.render();
      (models.length?this.$("initial-model"):this.$("choose-initial-model")).focus();
    }catch(error){if(generation===this.generation){this.stage="ready";this.render();this.error(error);}}
  }
  async start() {
    if(this.stage!=="ready"||!this.provider)return;
    this.stage="opening";this.$("session-picker-error").hidden=true;this.render();
    try {
      this.address??=await this.invoke("open_session",{directory:this.directory,choice:{
        kind:"new",provider:this.provider,model:this.$("initial-model-control").hidden?null:this.$("initial-model").value||null
      }});
      await this.opened(this.address);this.$("session-picker").close();this.$("draft").focus();
    }catch(error){this.error(this.address?"The conversation opened, but could not be shown. Try again. "+error:error);}
    finally{this.stage="ready";this.render();}
  }
}