// Isolated wire observations, not a second implementation of owner readiness policy.
export function roadmapFixture(){
  const activity={counts:{total:3,backlog:0,pending:2,in_progress:0,blocked:1},readiness:{ready:1,waiting:1,blocked:1},next_ready:"reopen"};
  const other={counts:{total:1,backlog:1,pending:0,in_progress:0,blocked:0},readiness:{ready:0,waiting:0,blocked:0},next_ready:null};
  const closed={counts:{total:0,backlog:0,pending:0,in_progress:0,blocked:0},readiness:{ready:0,waiting:0,blocked:0},next_ready:null};
  const chunks=[
    {id:"reopen",name:"Reopening",title:"Restore the project conversation and draft",group:"Desktop",status:"pending",priority:100,deps:["foundation"],acceptance:["The same conversation and unsent message return after reopening."],description:"Keep the last conversation available when opening a project.",cross_refs:[],notes:""},
    {id:"packaging",name:"Packaging",title:"Build the desktop installer",group:"Desktop",status:"pending",priority:110,deps:["signing"],acceptance:["Install on a clean machine."],description:"Produce an installer for the supported desktop platforms.",cross_refs:[],notes:""},
    {id:"release",name:"Release",title:"Publish the desktop release",group:"Desktop",status:"blocked",priority:120,deps:[],blocked_by:{kind:"awaiting_human",reason:"The release date needs approval."},acceptance:[],description:"Make the tested release available.",cross_refs:[],notes:""},
    {id:"foundation",name:"Foundation",title:"Store conversations and drafts",group:"Platform",status:"done",priority:10,deps:[],acceptance:[],description:"Save the complete conversation and its draft.",cross_refs:[],notes:""},
    {id:"guide",name:"Guide",title:"Improve the installation guide",group:null,status:"backlog",priority:150,deps:[],acceptance:[],description:"Explain installation.",cross_refs:[],notes:""}
  ];
  chunks[0].content={version:1,summary:"Return to the work you left.",facts:[{label:"Applies to",value:"Project reopening"}],
    sections:[{heading:"What changes",prose:"Keep the conversation and draft together when the project opens.",list:[{text:"Restore the saved conversation.",done:true},{text:"Keep unsent text ready to edit.",done:false}]}]};
  chunks[0].notes="Check reopening with a long conversation and an unsent message.";
  chunks[2].status_proposal={suggested_status:"pending",reason:"The release date has been discussed and is ready for a decision.",source:"think:41",proposed_at:"2026-10-15T08:00:00Z"};
  const frontiers=[
    {group:"Desktop",activity,items:[
      {id:"reopen",name:"Reopening",priority:100,status:"pending",readiness:{state:"ready"}},
      {id:"packaging",name:"Packaging",priority:110,status:"pending",readiness:{state:"waiting",dependencies:[{id:"signing",status:null}]}},
      {id:"release",name:"Release",priority:120,status:"blocked",readiness:{state:"blocked",blocker:{kind:"awaiting_human",reason:"The release date needs approval."}}}
    ]},
    {group:"Platform",activity:closed,items:[]},
    {group:null,activity:other,items:[{id:"guide",name:"Guide",priority:150,status:"backlog",readiness:{state:"inactive"}}]}
  ];
  const source={kind:"memory"};
  return {records:{state:"current",data:{source,value:{chunks,progress:{}}}},
    overview:{state:"current",data:{source,value:{catalog:{current:{Desktop:activity},history:{Platform:{done:1,obsoleted:0}},ungrouped:other,ungrouped_history:{done:0,obsoleted:0}},frontiers}}}};
}
export async function roadmapScene(app,capture){
  await app.getByRole("tab",{name:"Roadmap",exact:true}).click();
  await app.getByRole("heading",{name:"Desktop",exact:true}).waitFor();
  await capture("roadmap-dark.png");
  await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await app.getByRole("heading",{name:"Restore the project conversation and draft",exact:true}).waitFor();
  await capture("roadmap-item-dark.png");
  await app.getByRole("region",{name:"Required work",exact:true}).getByRole("button",{name:"Store conversations and drafts",exact:true}).click();
  await app.getByRole("heading",{name:"Store conversations and drafts",exact:true}).waitFor();
  await capture("roadmap-dependency-dark.png");
  await app.getByRole("button",{name:"Back to previous item",exact:true}).click();
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await app.getByRole("button",{name:"Publish the desktop release",exact:true}).click();
  await app.getByRole("heading",{name:"Publish the desktop release",exact:true}).waitFor();
  await app.getByRole("region",{name:"Suggested changes",exact:true}).evaluate(node=>node.scrollIntoView({block:"start"}));await capture("roadmap-proposal-dark.png");
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await app.getByLabel("Track",{exact:true}).selectOption('"Platform"');
  await app.getByLabel("Items",{exact:true}).selectOption("closed");
  await capture("roadmap-history-dark.png");
  await app.getByLabel("Search all roadmap",{exact:true}).fill("conversation");
  await capture("roadmap-search-dark.png");
  await app.getByLabel("Search all roadmap",{exact:true}).fill("");
  await app.getByLabel("Items",{exact:true}).selectOption("open");
  await app.getByLabel("Track",{exact:true}).selectOption('"Desktop"');
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
}