// Isolated wire observations, not a second implementation of owner readiness policy.
export function planFixture(){
  const activity={counts:{total:3,idea:0,to_do:2,in_progress:0,waiting:1},readiness:{ready:1,dependent:1,waiting:1},next_ready:"reopen"};
  const other={counts:{total:1,idea:1,to_do:0,in_progress:0,waiting:0},readiness:{ready:0,dependent:0,waiting:0},next_ready:null};
  const closed={counts:{total:0,idea:0,to_do:0,in_progress:0,waiting:0},readiness:{ready:0,dependent:0,waiting:0},next_ready:null};
  const tasks=[
    {id:"reopen",name:"Reopening",title:"Restore the project conversation and draft",feature:"Desktop",status:"to_do",priority:100,deps:["foundation"],acceptance:["The same conversation and unsent message return after reopening."],description:"Keep the last conversation available when opening a project.",refs:[],notes:""},
    {id:"packaging",name:"Packaging",title:"Build the desktop installer",feature:"Desktop",status:"to_do",priority:110,deps:["signing"],acceptance:["Install on a clean machine."],description:"Produce an installer for the supported desktop platforms.",refs:[],notes:""},
    {id:"release",name:"Release",title:"Publish the desktop release",feature:"Desktop",status:"waiting",priority:120,deps:[],waiting_on:{kind:"person",reason:"The release date needs approval."},acceptance:[],description:"Make the tested release available.",refs:[],notes:""},
    {id:"foundation",name:"Foundation",title:"Store conversations and drafts",feature:"Platform",status:"done",priority:10,deps:[],acceptance:[],description:"Save the complete conversation and its draft.",refs:[],notes:""},
    {id:"guide",name:"Guide",title:"Improve the installation guide",feature:null,status:"idea",priority:150,deps:[],acceptance:[],description:"Explain installation.",refs:[],notes:""}
  ];
  tasks[0].content={version:1,summary:"Return to the work you left.",facts:[{label:"Applies to",value:"Project reopening"}],
    sections:[{heading:"What changes",prose:"Keep the conversation and draft together when the project opens.",list:[{text:"Restore the saved conversation.",done:true},{text:"Keep unsent text ready to edit.",done:false}]}]};
  tasks[0].notes="Check reopening with a long conversation and an unsent message.";
  tasks[2].status_proposal={suggested_status:"to_do",reason:"The release date has been discussed and is ready for a decision.",source:"decision:41",proposed_at:"2026-10-15T08:00:00Z"};
  const frontiers=[
    {feature:"Desktop",activity,items:[
      {id:"reopen",name:"Reopening",priority:100,status:"to_do",readiness:{state:"ready"}},
      {id:"packaging",name:"Packaging",priority:110,status:"to_do",readiness:{state:"dependent",dependencies:[{id:"signing",status:null}]}},
      {id:"release",name:"Release",priority:120,status:"waiting",readiness:{state:"waiting",waiting_on:{kind:"person",reason:"The release date needs approval."}}}
    ]},
    {feature:"Platform",activity:closed,items:[]},
    {feature:null,activity:other,items:[{id:"guide",name:"Guide",priority:150,status:"idea",readiness:{state:"inactive"}}]}
  ];
  const source={kind:"memory"};
  return {records:{state:"current",data:{source,value:{tasks,progress:{}}}},
    overview:{state:"current",data:{source,value:{catalog:{current:{Desktop:activity},history:{Platform:{done:1,retired:0}},unassigned:other,unassigned_history:{done:0,retired:0}},frontiers}}}};
}
export async function planScene(app,capture){
  await app.getByRole("tab",{name:"Plan",exact:true}).click();
  await app.getByRole("heading",{name:"Desktop",exact:true}).waitFor();
  await capture("plan-dark.png");
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await app.getByRole("heading",{name:"Restore the project conversation and draft",exact:true}).waitFor();
  await capture("plan-item-dark.png");
  await app.getByRole("region",{name:"Required work",exact:true}).getByRole("button",{name:"Store conversations and drafts",exact:true}).click();
  await app.getByRole("heading",{name:"Store conversations and drafts",exact:true}).waitFor();
  await capture("plan-dependency-dark.png");
  await app.getByRole("button",{name:"Back to previous item",exact:true}).click();
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await app.getByRole("button",{name:"Publish the desktop release",exact:true}).click();
  await app.getByRole("heading",{name:"Publish the desktop release",exact:true}).waitFor();
  await app.getByRole("region",{name:"Suggested changes",exact:true}).evaluate(node=>node.scrollIntoView({block:"start"}));await capture("plan-proposal-dark.png");
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await app.getByLabel("Track",{exact:true}).selectOption('"Platform"');
  await app.getByLabel("Items",{exact:true}).selectOption("closed");
  await capture("plan-history-dark.png");
  await app.getByLabel("Search all plan",{exact:true}).fill("conversation");
  await capture("plan-search-dark.png");
  await app.getByLabel("Search all plan",{exact:true}).fill("");
  await app.getByLabel("Items",{exact:true}).selectOption("open");
  await app.getByLabel("Track",{exact:true}).selectOption('"Desktop"');
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
}