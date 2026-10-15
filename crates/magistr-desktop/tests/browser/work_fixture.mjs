import { feedbackFixture,feedbackScene } from "./feedback_fixture.mjs";
import { reasoningFixture,reasoningScene } from "./reasoning_fixture.mjs";
import { roadmapScene } from "./roadmap_fixture.mjs";
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
  current.tasks[1].actions=[{id:1,task_id:"build",timestamp:"2026-10-15T08:12:00Z",type:"code",description:"Restore the saved message before opening the project conversation.",files_touched:["crates/desktop/src/reopen.rs"],tools_used:["Source editor"],result:"The saved message now appears when the project opens. The focused installer checks passed."}];
  current.tasks[1].artifacts=[
    {type:"file",ref:"docs/release-checklist.md",description:"Release checklist"},
    {type:"pr",ref:"https://example.com/review/42",description:"Review the reopening change"}
  ];
  current.tasks[1].checks=[
    {receipt_id:"check-failed",name:"Installer checks",type:"test",passed:false,verified:true,required:true,command:"cargo test --test installer",exit_code:101,timestamp:"2026-10-15T08:10:00Z",details:"running 2 tests\nrestore_message ... FAILED\nopen_project ... ok\n\nExpected the saved message; found an empty draft.",results:{total:2,passed:1,failed:1,skipped:0,failures:[{name:"restore_message",message:"Expected the saved message; found an empty draft."}]}},
    {receipt_id:"check-passed",name:"Installer checks",type:"test",passed:true,verified:true,required:true,command:"cargo test --test installer",exit_code:0,timestamp:"2026-10-15T08:15:00Z",details:"running 2 tests\nrestore_message ... ok\nopen_project ... ok\n\n2 passed; 0 failed.",results:{total:2,passed:2,failed:0,skipped:0,failures:[]}},
    {name:"Visual review",type:"review",passed:true,verified:false,required:false,timestamp:"2026-10-15T08:20:00Z",details:"The installer window is readable."}
  ];
  const previous=structuredClone(current);previous.run_id="previous-run";
  previous.objective={...previous.objective,description:"Improve project reopening",status:"completed",created_at:"2026-10-14T08:00:00Z",completed_at:"2026-10-14T09:00:00Z"};
  previous.tasks=previous.tasks.map(task=>({...task,title:"Earlier: "+task.title,status:"completed"}));
  previous.task_instances={review:"earlier-review",build:"earlier-build",verify:"earlier-verify"};
  return {ship:{state:"current",data:{source:{kind:"memory"},value:{current,retained:[previous]}}},
    roadmap:{state:"unobserved"},think:reasoningFixture(),signals:feedbackFixture()};
}
export async function workScene(app,capture){
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await app.getByRole("heading",{name:"Prepare the desktop release",exact:true}).waitFor();
  await capture("work-plan-dark.png");
  await app.getByLabel("Task status",{exact:true}).selectOption("active");
  await app.locator(".work-tasks").evaluate(node=>node.scrollIntoView({block:"start"}));await capture("work-task-filter-dark.png");
  await app.getByLabel("Task status",{exact:true}).selectOption("all");await app.locator("#work-browser-content").evaluate(node=>node.scrollTop=0);
  await app.getByRole("radio",{name:"Checks",exact:true}).check();await app.getByLabel("Check results",{exact:true}).selectOption("failed");
  await app.locator(".work-check").locator("summary").click();await app.locator(".work-check").evaluate(node=>node.scrollIntoView({block:"start"}));await capture("work-check-index-dark.png");
  await app.getByLabel("Check results",{exact:true}).selectOption("all");await app.getByRole("radio",{name:"Plan and tasks",exact:true}).check();await app.locator("#work-browser-content").evaluate(node=>node.scrollTop=0);
  await app.getByRole("radio",{name:"Activity and produced items",exact:true}).check();await app.getByLabel("Entries",{exact:true}).selectOption("items");
  await app.locator(".work-activity-entry").first().evaluate(node=>node.scrollIntoView({block:"start"}));await capture("work-activity-index-dark.png");
  await app.getByLabel("Entries",{exact:true}).selectOption("all");await app.getByRole("radio",{name:"Plan and tasks",exact:true}).check();await app.locator("#work-browser-content").evaluate(node=>node.scrollTop=0);
  await app.getByRole("tab",{name:"Record status",exact:true}).click();await app.locator("#work-browser-content").evaluate(node=>node.scrollTop=0);await capture("work-record-status-dark.png");
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
  await roadmapScene(app,capture);await reasoningScene(app,capture);await feedbackScene(app,capture);
  const check=app.locator(".work-check").nth(1);await check.locator("summary").click();
  await check.evaluate(node=>node.scrollIntoView({block:"start"}));await capture("work-check-results-dark.png");
  await check.locator("summary").click();await app.locator("#work-browser-content").evaluate(node=>node.scrollTop=0);
  const activity=app.locator(".work-activity").first();await activity.locator("summary").click();
  await activity.evaluate(node=>node.scrollIntoView({block:"start"}));await capture("work-activity-dark.png");
  await activity.locator("summary").click();await app.locator("#work-browser-content").evaluate(node=>node.scrollTop=0);
  await app.getByLabel("Work to inspect",{exact:true}).selectOption("run:previous-run");
  await capture("saved-work-dark.png");
  await app.getByLabel("Search work history",{exact:true}).fill("installer");await capture("work-history-search-dark.png");
  await app.getByLabel("Search work history",{exact:true}).fill("");
  await app.getByLabel("Work to inspect",{exact:true}).selectOption("current");
  await app.getByRole("button",{name:"Back to conversation",exact:true}).click();
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("light");
  await app.getByRole("button",{name:"Done",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await capture("work-plan-light.png");
  await app.getByRole("tab",{name:"Roadmap",exact:true}).click();await capture("roadmap-light.png");
  await app.setViewportSize({width:760,height:550});await capture("roadmap-minimum.png");
  await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();
  await app.setViewportSize({width:760,height:550});await capture("work-plan-minimum.png");
  await app.getByRole("tab",{name:"Roadmap",exact:true}).click();await app.getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();await app.locator(".roadmap-item-detail").waitFor();await capture("roadmap-detail-minimum-light.png");
  await app.getByRole("tab",{name:"Reasoning",exact:true}).click();await app.locator(".reasoning-step").waitFor();await capture("reasoning-detail-minimum-light.png");
  await app.getByRole("tab",{name:"Feedback",exact:true}).click();await app.getByRole("button",{name:"Keep my place when reopening a project",exact:true}).click();await capture("feedback-detail-minimum-light.png");
  await app.getByRole("button",{name:"Back to conversation",exact:true}).click();
  await app.setViewportSize({width:1280,height:820});
  await app.getByRole("button",{name:"Settings",exact:true}).click();
  await app.getByLabel("Appearance",{exact:true}).selectOption("dark");
  await app.getByRole("button",{name:"Done",exact:true}).click();
}
