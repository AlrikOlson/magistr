import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Reasoning",exact:true}).click();}
test("reasoning sessions expose complete stable steps and exact field copying",async({app})=>{
  await open(app);
  await app.evaluate(()=>{Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async text=>window.__copied=text}});const f=window.__fixture;f.workflow.think.data.value.sessions[0].steps[0].thought="日本語\n".repeat(6000)+"<script>literal</script>";f.publish("release",{workflow_revision:1});});
  await app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}).click();
  await expect(app.locator(".reasoning-step")).toContainText("<script>literal</script>");await expect(app.locator(".reasoning-step script")).toHaveCount(0);
  await app.getByRole("button",{name:"Copy reasoning",exact:true}).click();expect(await app.evaluate(()=>window.__copied)).toBe("日本語\n".repeat(6000)+"<script>literal</script>");
  await app.getByRole("button",{name:"Back to reasoning",exact:true}).click();await expect(app.getByRole("button",{name:"Keep each conversation with its saved message",exact:true})).toBeFocused();
  await app.getByLabel("Reasoning to inspect",{exact:true}).selectOption('"review"');
  await app.getByRole("button",{name:"Inspect the installation result",exact:true}).click();await expect(app.locator(".reasoning-step")).toContainText("Step 42");
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.think.data.value.sessions=f.workflow.think.data.value.sessions.filter(s=>s.session_id!=="review");f.publish("release",{workflow_revision:2});});
  await expect(app.locator("#work-browser-content")).toContainText("This reasoning is no longer available.");
});
test("reasoning pages retain complete records while stale and absent reads stay distinct",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,h=f.workflow.think.data.value.sessions[0],base=h.steps[0];h.steps=Array.from({length:121},(_,n)=>({...base,record_id:"record-"+n,step_number:n+1,purpose:"Decision "+n}));f.publish("release",{workflow_revision:1});});
  await expect(app.locator(".roadmap-items>li")).toHaveCount(50);await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();
  await expect(app.locator(".roadmap-items>li")).toHaveCount(21);
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.think={state:"stale",data:{last:f.workflow.think.data,error:{kind:"io",message:"Cannot read reasoning."}}};f.publish("release",{workflow_revision:2});});
  await expect(app.locator("#work-browser-state")).toContainText("Showing the last saved records.");await expect(app.locator(".roadmap-items>li")).toHaveCount(21);
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.think={state:"unavailable",data:{kind:"io",message:"Cannot read reasoning."}};f.publish("release",{workflow_revision:3});});
  await expect(app.locator("#work-browser-content")).toContainText("Reasoning records are unavailable.");
});
