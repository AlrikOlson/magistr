import {test,expect} from "@playwright/test";
import {root} from "./fixture.mjs";
import {captureIteration} from "./iteration_evidence.mjs";
import {readdir} from "node:fs/promises";
import {resolve} from "node:path";

const origin="http://127.0.0.1:6007";
const selected=process.env.MAGISTR_GUI_STORY?.split(",");
test("capture verified Storybook states",async({page,request})=>{
  const errors=[];page.on("pageerror",error=>errors.push(error.message));
  const response=await request.get(origin+"/index.json");
  expect(response.ok()).toBe(true);
  const index=await response.json();
  const entries=Object.values(index.entries).filter(entry=>entry.type==="story"&&(!selected||selected.includes(entry.id)));
  if(selected)expect(entries.map(entry=>entry.id).sort()).toEqual([...selected].sort());
  expect(entries.length).toBeGreaterThan(0);
  const extraSources=[];
  for(const folder of [".storybook","stories"]){
    for(const file of await readdir(resolve(root,"crates/magistr-desktop",folder)))
      extraSources.push(resolve(root,"crates/magistr-desktop",folder,file));
  }
  for(const file of ["observations.mjs","work_fixture.mjs","roadmap_fixture.mjs","reasoning_fixture.mjs","feedback_fixture.mjs"])
    extraSources.push(resolve(root,"crates/magistr-desktop/tests/browser",file));
  for(const entry of entries){
    for(const [size,viewport] of [["desktop",{width:1280,height:820}],["compact",{width:760,height:550}]]){
      if(entry.tags.includes(size==="desktop"?"compact-only":"desktop-only"))continue;
      for(const appearance of ["dark","light"]){
        await page.setViewportSize(viewport);
        await page.goto(origin+"/iframe.html?id="+entry.id+"&viewMode=story&globals=appearance:"+appearance);
        const preview=page.locator("iframe.desktop-preview");
        await expect.poll(()=>preview.evaluate(el=>({id:el.dataset.storyReady??null,error:el.dataset.storyError??null}))).toEqual({id:entry.id,error:null});
        const frame=await preview.contentFrame().locator("html").elementHandle().then(handle=>handle.ownerFrame());
        expect(errors).toEqual([]);
        const subject=await frame.evaluate(()=>{window.__verifyStory();return window.__storyState;});
        expect(subject.id).toBe(entry.id);
        expect(subject.title).toBe(entry.title);
        expect(subject.name).toBe(entry.name);
        expect(subject.viewport).toEqual(viewport);
        expect(subject.appearance).toBe(appearance);
        expect(await frame.evaluate(()=>document.documentElement.scrollWidth>innerWidth)).toBe(false);
        await page.mouse.move(0,0);
        await captureIteration(page,{root,name:entry.id+"-"+size+"-"+appearance,scenario:test.info().file,
          subject,documentFrame:frame,imageTarget:preview,extraSources});
      }
    }
  }
});
test("reject a directory capture after its visible screen changes",async({page})=>{
  await page.goto(origin+"/iframe.html?id=projects-directory--list&viewMode=story");
  const preview=page.locator("iframe.desktop-preview");
  await expect(preview).toHaveAttribute("data-story-ready","projects-directory--list");
  const frame=await preview.contentFrame().locator("html").elementHandle().then(handle=>handle.ownerFrame());
  await frame.evaluate(()=>{document.getElementById("welcome").hidden=true;});
  await expect(frame.evaluate(()=>window.__verifyStory())).rejects.toThrow();
});
