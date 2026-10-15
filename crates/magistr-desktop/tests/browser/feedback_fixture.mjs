export function feedbackFixture(){return {state:"current",data:{source:{kind:"memory"},value:[
  {id:"feedback-reopen",kind:"concern",from:"Project owner",body:"When I reopen a project, I need the same conversation and unsent message to be there.",created:"2026-10-15T07:00:00Z",status:"researched",enrichment:[{at:"2026-10-15T08:00:00Z",summary:"The conversation owns the saved message. Restore it before presenting the editor.",confidence:0.9,sources:["https://example.com/reopening","docs/reopening.md"],think_step:41}],cross_refs:[],
   content:{version:1,summary:"Keep my place when reopening a project",facts:[{label:"When",value:"After closing and reopening"}],sections:[{heading:"What I expect",prose:"The text I was writing should still be ready to edit.",list:[{text:"Restore my conversation.",done:true},{text:"Keep my unsent message.",done:false}]}]}},
  {id:"feedback-theme",kind:"idea",from:"Reviewer",body:"Offer a light appearance for daytime work.",created:"2026-10-14T07:00:00Z",status:"promoted",enrichment:[],cross_refs:[]}
]}};}
export async function feedbackScene(app,capture){
  await app.getByRole("tab",{name:"Feedback",exact:true}).click();await capture("feedback-dark.png");
  await app.getByRole("button",{name:"Keep my place when reopening a project",exact:true}).click();await capture("feedback-detail-dark.png");
  const note=app.locator(".feedback-research details").first();await note.locator("summary").click();await note.evaluate(node=>node.scrollIntoView({block:"start"}));await capture("feedback-research-dark.png");
  await app.getByRole("button",{name:"Back to feedback",exact:true}).click();
  await app.getByLabel("Kind",{exact:true}).selectOption("concern");await app.getByLabel("Search feedback",{exact:true}).fill("editor");await capture("feedback-search-dark.png");
  await app.getByLabel("Kind",{exact:true}).selectOption("all");await app.getByLabel("Search feedback",{exact:true}).fill("");
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
}
