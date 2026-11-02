export function reasoningFixture(){
  const step={record_id:"decision-41",note:41,estimated_total:42,purpose:"Keep each conversation with its saved message",context:"Reopening a project should restore the same conversation.",
    thought:"The conversation already owns its saved message. Read that record before the editor opens.",rationale:"A second saved message would drift from the conversation.",outcome:"Keep one writer and restore the recorded text.",
    next_action:"Check reopening with an unsent message.",timestamp:"2026-10-15T08:05:00Z",pinned:true,tools_used:["Source inspection"]};
  return {state:"current",data:{source:{kind:"memory"},value:{logs:[
    {log_id:null,notes:[step,{...step,record_id:"decision-43",note:43,purpose:"Check whether reopening needs a second saved message",thread_id:"storage",pinned:false}],threads:[{id:"storage",name:"Does reopening need another saved message?",from_note:41,notes:[],status:"merged",merged_into:41,created_at:step.timestamp,depth:1}],completed:false,created_at:step.timestamp},
    {log_id:"review",notes:[{...step,record_id:"decision-42",note:42,purpose:"Inspect the installation result",pinned:false,dependencies:[{note:41,relation:"supports"}]}],threads:[],completed:true,created_at:step.timestamp}
  ]}}};
}
export async function reasoningScene(app,capture){
  await app.getByRole("tab",{name:"Reasoning",exact:true}).click();await capture("reasoning-dark.png");
  await app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();await capture("reasoning-step-dark.png");
  await app.getByRole("region",{name:"Related reasoning",exact:true}).getByRole("button",{name:"Inspect the installation result",exact:true}).click();await capture("reasoning-related-dark.png");
  await app.getByRole("button",{name:"Back to previous step",exact:true}).click();
  await app.getByRole("button",{name:"Back to reasoning",exact:true}).click();
  await app.getByLabel("Investigation",{exact:true}).selectOption("branch:storage");await capture("reasoning-investigation-dark.png");
  await app.getByLabel("Investigation",{exact:true}).selectOption("all");
  await app.getByLabel("Search all reasoning",{exact:true}).fill("installation");await capture("reasoning-search-dark.png");
  await app.getByLabel("Search all reasoning",{exact:true}).fill("");
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
}
