// Source-bound screenshots for a single GUI iteration; no OS input.
import {mkdir,readFile,readdir,writeFile} from "node:fs/promises";
import {resolve,relative} from "node:path";
import {createHash} from "node:crypto";
import {execFileSync} from "node:child_process";
const sha=bytes=>createHash("sha256").update(bytes).digest("hex");
export async function captureIteration(page,{root,name,scenario}){
  const iteration=process.env.MAGISTR_GUI_ITERATION;
  if(!iteration)return;
  if(!/^[a-z0-9][a-z0-9/_-]*$/.test(iteration))throw new Error("Use a dated, named GUI iteration.");
  const directory=resolve(root,".magistr/desktop-iterations",iteration);await mkdir(directory,{recursive:true});
  const image=await page.screenshot({path:resolve(directory,name+".png"),animations:"disabled"});
  const ui=resolve(root,"crates/magistr-desktop/ui"),sources={};
  for(const file of (await readdir(ui)).sort()){
    const path=resolve(ui,file);sources[relative(root,path)]=sha(await readFile(path));
  }
  for(const path of [scenario,resolve(root,"crates/magistr-desktop/tests/browser/iteration_evidence.mjs"),resolve(root,"crates/magistr-desktop/tests/browser/fixture.mjs")])sources[relative(root,path)]=sha(await readFile(path));
  const receipt={iteration,view:name,captured_at:new Date().toISOString(),renderer:"headless WebKit",renderer_version:page.context().browser().version(),
    viewport:page.viewportSize(),appearance:await page.evaluate(()=>document.documentElement.dataset.theme??"system"),
    base_commit:execFileSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"}).trim(),source_hashes:sources,
    image:{file:name+".png",sha256:sha(image)},scope:"Production HTML/CSS/JavaScript with isolated observation fixtures. No native OS input, Rust command integration or assistive technology."};
  await writeFile(resolve(directory,name+".json"),JSON.stringify(receipt,null,2)+"\n");
}
