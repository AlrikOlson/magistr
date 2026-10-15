import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Record status",exact:true}).click();}
test("record status keeps partial failures, missing records and unread records distinct",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,w=f.workflow;w.ship={state:"stale",data:{last:w.ship.data,error:{kind:"io",message:"Could not read saved work. 日本語 <script>plain</script>"}}};w.roadmap={state:"current",data:{source:{kind:"missing"},value:{chunks:[],progress:{}}}};w.think={state:"unobserved"};w.signals={state:"unavailable",data:{kind:"decode",message:"Feedback could not be read."}};Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async text=>window.__copied=text}});f.publish("release",{workflow_revision:1});});
  const work=app.getByRole("region",{name:"Work records",exact:true});await expect(work).toContainText("Last known records");await expect(work).toContainText("6");
  await expect(app.getByRole("region",{name:"Roadmap records",exact:true})).toContainText("No saved records");
  await expect(app.getByRole("region",{name:"Reasoning records",exact:true})).toContainText("Not read yet");await expect(app.getByRole("region",{name:"Reasoning records",exact:true}).locator("dl")).toHaveCount(0);
  await expect(app.getByRole("region",{name:"Feedback records",exact:true})).toContainText("Could not read records");await expect(app.locator(".work-status script")).toHaveCount(0);
  await work.getByRole("button",{name:"Copy work read error",exact:true}).click();expect(await app.evaluate(()=>window.__copied)).toBe("Could not read saved work. 日本語 <script>plain</script>");
  await app.evaluate(()=>{const w=window.__fixture.workflow;w.ship={state:"current",data:w.ship.data.last};});await app.getByRole("button",{name:"Refresh",exact:true}).click();await expect(work).toContainText("Read successfully");
  await work.getByRole("button",{name:"Open work",exact:true}).click();await expect(app.getByRole("tab",{name:"Current and saved work",exact:true})).toHaveAttribute("aria-selected","true");await expect(app.locator(".work-record-heading")).toContainText("Prepare the desktop release");
});
test("record counts use complete histories once and label file time correctly",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,w=f.workflow;w.think.data.value.sessions[0].branches[0].steps=[...w.think.data.value.sessions[0].steps];delete w.think.data.value.sessions[1].branches;w.think.data.source={kind:"file",stamp:{inode:123456789,length:1900,modified:{secs_since_epoch:1760000000,nanos_since_epoch:123000000}}};f.publish("release",{workflow_revision:1});});
  const reasoning=app.getByRole("region",{name:"Reasoning records",exact:true});
  await expect(reasoning.locator("dt").filter({hasText:"Recorded steps"}).locator("..").locator("dd")).toHaveText("3");await expect(reasoning.locator("time")).toHaveAttribute("datetime",new Date(1760000000123).toISOString());
  await expect(reasoning).toContainText("Saved records last changed");await expect(reasoning).not.toContainText("123456789");await expect(reasoning).not.toContainText("1900");await expect(reasoning).not.toContainText("Last read");
  await app.setViewportSize({width:760,height:550});await reasoning.scrollIntoViewIfNeeded();await expect(reasoning).toBeVisible();expect(await app.locator("#work-browser-content").evaluate(node=>node.scrollWidth<=node.clientWidth+1)).toBe(true);
});
