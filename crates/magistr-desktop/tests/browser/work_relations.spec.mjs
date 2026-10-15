import {test,expect} from "./fixture.mjs";
async function open(app,title="Restore the project conversation and draft"){
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await app.getByRole("button",{name:"Work plan",exact:true}).click();
  await app.getByRole("tab",{name:"Roadmap",exact:true}).click();
  await app.getByRole("button",{name:title,exact:true}).click();
}
test("navigate dependencies and dependents with local Back and origin focus",async({app})=>{
  await open(app);
  const required=app.getByRole("region",{name:"Required work",exact:true});
  await required.getByRole("button",{name:"Store conversations and drafts",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Store conversations and drafts",exact:true})).toBeFocused();
  await app.getByRole("region",{name:"Work that depends on this",exact:true}).getByRole("button",{name:"Restore the project conversation and draft",exact:true}).click();
  await expect(app.getByRole("heading",{name:"Restore the project conversation and draft",exact:true})).toBeFocused();
  await app.getByRole("button",{name:"Back to previous item",exact:true}).click();
  await expect(app.getByRole("region",{name:"Work that depends on this",exact:true}).getByRole("button")).toBeFocused();
  await app.getByRole("button",{name:"Back to previous item",exact:true}).click();
  await expect(required.getByRole("button")).toBeFocused();
  await app.getByRole("button",{name:"Back to track",exact:true}).click();
  await expect(app.getByRole("button",{name:"Restore the project conversation and draft",exact:true})).toBeFocused();
  await expect(app.getByLabel("Track",{exact:true})).toHaveValue('"Desktop"');
});
test("missing dependencies are explicit and never interactive",async({app})=>{
  await open(app,"Build the desktop installer");
  await expect(app.locator(".roadmap-readiness")).toHaveText("Waiting for required work.");
  const required=app.getByRole("region",{name:"Required work",exact:true});
  await expect(required).toContainText("Required item unavailable");
  await expect(required.getByRole("button")).toHaveCount(0);
});
test("deleted dependency restores focus to readable content",async({app})=>{
  await open(app);
  await app.getByRole("button",{name:"Store conversations and drafts",exact:true}).focus();
  await app.evaluate(()=>{
    const f=window.__fixture;f.workflow.roadmap.data.value.chunks=f.workflow.roadmap.data.value.chunks.filter(item=>item.id!=="foundation");
    f.publish("release",{workflow_revision:1});
  });
  await expect(app.getByRole("region",{name:"Required work",exact:true})).toContainText("Required item unavailable");
  await expect(app.locator("#work-browser-content")).toBeFocused();
});
