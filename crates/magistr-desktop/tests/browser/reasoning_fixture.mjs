export function reasoningFixture(){
  const step={record_id:"decision-41",step_number:41,estimated_total:42,purpose:"Keep each conversation with its saved message",context:"Reopening a project should restore the same conversation.",
    thought:"The conversation already owns its saved message. Read that record before the editor opens.",rationale:"A second saved message would drift from the conversation.",outcome:"Keep one writer and restore the recorded text.",
    next_action:"Check reopening with an unsent message.",timestamp:"2026-10-15T08:05:00Z",pinned:true,tools_used:["Source inspection"]};
  return {state:"current",data:{source:{kind:"memory"},value:{sessions:[
    {session_id:null,steps:[step],branches:[],completed:false,created_at:step.timestamp},
    {session_id:"review",steps:[{...step,record_id:"decision-42",step_number:42,purpose:"Inspect the installation result",pinned:false}],branches:[],completed:true,created_at:step.timestamp}
  ]}}};
}
export async function reasoningScene(app,capture){
  await app.getByRole("tab",{name:"Reasoning",exact:true}).click();await capture("reasoning-dark.png");
  await app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();await capture("reasoning-step-dark.png");
  await app.getByRole("button",{name:"Back to reasoning",exact:true}).click();await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
}
