export function workSnapshot(){
  const current={run_id:"release-run",task_instances:{review:"review-instance",build:"build-instance",verify:"verify-instance"},
    objective:{description:"Prepare the desktop release",status:"active",scope:"chunk:desktop-release",created_at:"2026-10-15T08:00:00Z",
      acceptance_criteria:["Reopening a project restores the same conversation and unsent message.","The release can be installed on a clean machine, and the previous build remains available."],
      constraints:["Keep each change focused and verify the affected behavior."]},
    tasks:[
      {id:"review",title:"Review the release requirements",status:"completed",type:"review",estimate:null,actions:[],checks:[],artifacts:[]},
      {id:"build",title:"Build and inspect the installer",status:"active",type:"implement",estimate:"One focused pass",actions:[],checks:[],artifacts:[]},
      {id:"verify",title:"Verify reopening and rollback",status:"planned",type:"test",estimate:null,actions:[],checks:[],artifacts:[]}
    ]};
  const previous=structuredClone(current);previous.run_id="previous-run";
  previous.objective={...previous.objective,description:"Improve project reopening",status:"completed",created_at:"2026-10-14T08:00:00Z",completed_at:"2026-10-14T09:00:00Z"};
  previous.tasks=previous.tasks.map(task=>({...task,title:"Earlier: "+task.title,status:"completed"}));
  previous.task_instances={review:"earlier-review",build:"earlier-build",verify:"earlier-verify"};
  return {ship:{state:"current",data:{source:{kind:"memory"},value:{current,retained:[previous]}}},
    roadmap:{state:"unobserved"},think:{state:"unobserved"},signals:{state:"unobserved"}};
}
export async function workScene(app,capture){
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await app.getByRole("heading",{name:"Prepare the desktop release",exact:true}).waitFor();
  await capture("work-plan-dark.png");
  await app.getByLabel("Work to inspect",{exact:true}).selectOption("run:previous-run");
  await capture("saved-work-dark.png");
  await app.getByLabel("Work to inspect",{exact:true}).selectOption("current");
  await app.getByRole("button",{name:"Close planned work",exact:true}).click();
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await capture("work-plan-light.png");
  await app.setViewportSize({width:760,height:550});await capture("work-plan-minimum.png");
  await app.getByRole("button",{name:"Close planned work",exact:true}).click();
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("dark");
  await app.getByRole("button",{name:"Done",exact:true}).click();
}
