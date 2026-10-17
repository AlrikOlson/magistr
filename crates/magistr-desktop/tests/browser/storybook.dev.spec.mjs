import {test,expect} from "@playwright/test";

test("development catalog loads and switches verified stories promptly",async({page})=>{
  const errors=[],timings=[];
  page.on("pageerror",error=>errors.push(error.message));
  const preview=page.frameLocator("#storybook-preview-iframe").locator("iframe.desktop-preview");
  async function ready(id,started){
    await expect(preview).toHaveAttribute("data-story-ready",id,{timeout:5000});
    await expect(preview).not.toHaveAttribute("data-story-error");
    const subject=await preview.evaluate(frame=>{
      frame.contentWindow.__verifyStory();
      return frame.contentWindow.__storyState;
    });
    expect(subject.id).toBe(id);
    timings.push({id,ready_ms:Math.round(performance.now()-started)});
  }
  const started=performance.now();
  await page.goto("http://127.0.0.1:6007/?path=/story/projects-directory--list",{waitUntil:"domcontentloaded",timeout:5000});
  await ready("projects-directory--list",started);
  for(const [group,id] of [["conversation","conversation--release-plan"],["roadmap","roadmap--overview"],["feedback","feedback--research"]]){
    const started=performance.now();
    await page.locator("button#"+group).click();
    await page.locator("a#"+id).click();
    await ready(id,started);
  }
  expect(errors).toEqual([]);
  console.log("Storybook development timings "+JSON.stringify(timings));
  await test.info().attach("story-loading.json",{body:JSON.stringify(timings,null,2),contentType:"application/json"});
});
