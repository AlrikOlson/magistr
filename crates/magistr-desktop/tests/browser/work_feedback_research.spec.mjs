import {test,expect} from "./fixture.mjs";
async function open(app){await app.getByRole("button",{name:"Review the release plan",exact:true}).click();await app.getByRole("button",{name:"Work plan",exact:true}).click();await app.getByRole("tab",{name:"Feedback",exact:true}).click();await app.getByRole("button",{name:"Keep my place when reopening a project",exact:true}).click();}
test("research preserves full summaries, safe sources and open notes across appends",async({app})=>{
  await open(app);await app.evaluate(()=>{Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:async value=>window.__copied=value}});const f=window.__fixture,n=f.workflow.signals.data.value[0].enrichment[0];n.summary="Complete note 日本語\n".repeat(2000);n.sources=["https://example.com/research","file:///private/report","javascript:alert(1)","https://user:secret@example.com"];f.publish("release",{workflow_revision:1});});
  const note=app.locator(".feedback-research details").first();await note.locator("summary").click();
  await expect(note).toContainText("Reported confidence: 90%.");await expect(note.getByRole("button",{name:"Open link",exact:true})).toHaveCount(1);
  await note.getByRole("button",{name:"Copy research summary",exact:true}).click();expect(await app.evaluate(()=>window.__copied)).toBe("Complete note 日本語\n".repeat(2000));
  await note.getByRole("button",{name:"Open link",exact:true}).click();expect(await app.evaluate(()=>window.__fixture.calls.filter(c=>c.command==="open_link").at(-1).args.destination)).toBe("https://example.com/research");
  await note.getByRole("button",{name:"Copy reference",exact:true}).nth(1).click();expect(await app.evaluate(()=>window.__copied)).toBe("file:///private/report");
  await app.evaluate(()=>{const f=window.__fixture;f.workflow.signals.data.value[0].enrichment.push({summary:"Later finding",confidence:0.5,at:"2026-10-15T09:00:00Z",sources:[]});f.publish("release",{workflow_revision:2});});
  await expect(app.locator(".feedback-research details")).toHaveCount(2);await expect(note).toHaveAttribute("open","");
  await app.locator(".feedback-research details").nth(1).locator("summary").click();await expect(app.locator(".feedback-research")).toContainText("No sources were recorded.");
});
