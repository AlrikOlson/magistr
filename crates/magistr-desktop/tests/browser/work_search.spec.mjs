import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("tab",{name:"Current and saved work",exact:true}).click();await app.getByRole("tab",{name:"Roadmap",exact:true}).click();}
test("search spans closed work and complete structured fields, preserving query on return",async({app})=>{
  await open(app);await app.getByLabel("Search all roadmap",{exact:true}).fill("save complete");
  await app.getByRole("button",{name:"Store conversations and drafts",exact:true}).click();
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await expect(app.getByRole("button",{name:"Store conversations and drafts",exact:true})).toBeFocused();
  await expect(app.getByLabel("Search all roadmap",{exact:true})).toHaveValue("save complete");
  await app.getByLabel("Search all roadmap",{exact:true}).fill("ready edit");
  await expect(app.getByRole("button",{name:"Restore the project conversation and draft",exact:true})).toBeVisible();
  await app.getByLabel("Search all roadmap",{exact:true}).fill("no match");
  await expect(app.locator("#work-browser-content")).toContainText("No roadmap items match this search.");
});
test("all matches remain accessible with bounded rows and unchanged input focus",async({app})=>{
  await open(app);await app.evaluate(()=>{const f=window.__fixture,base=f.workflow.roadmap.data.value.chunks[0];
    f.workflow.roadmap.data.value.chunks=Array.from({length:123},(_,n)=>({...base,id:"item-"+n,title:"記録 "+n,notes:"Full width ＡＢＣ and <script>literal</script>"}));f.publish("release",{workflow_revision:1});});
  const search=app.getByLabel("Search all roadmap",{exact:true});await search.fill("abc 記録");
  await expect(app.locator(".roadmap-items>li")).toHaveCount(50);await expect(search).toBeFocused();
  await app.getByRole("button",{name:"Next page",exact:true}).click();await app.getByRole("button",{name:"Next page",exact:true}).click();
  await expect(app.locator(".roadmap-items>li")).toHaveCount(23);
  await app.getByRole("button",{name:"記録 122",exact:true}).click();await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await expect(app.getByRole("button",{name:"記録 122",exact:true})).toBeFocused();
  await expect(app.getByRole("navigation",{name:"Result pages",exact:true})).toContainText("Page 3 of 3 · 123 results");
  await search.fill("<script>literal</script>");await expect(app.locator(".roadmap-items>li")).toHaveCount(50);
});