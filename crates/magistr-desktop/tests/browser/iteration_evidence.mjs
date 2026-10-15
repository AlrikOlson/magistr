// Source-bound screenshots for a single GUI iteration; no OS input.
import {access,mkdir,readFile,readdir,writeFile} from "node:fs/promises";
import {resolve,relative} from "node:path";
import {createHash} from "node:crypto";
import {execFileSync} from "node:child_process";
const sha=bytes=>createHash("sha256").update(bytes).digest("hex");
export async function prepareIteration(root,iteration){
  if(!/^[a-z0-9][a-z0-9/_-]*$/.test(iteration))throw new Error("Use a dated, named GUI iteration.");
  const directory=resolve(root,"docs/evidence/desktop-iterations",iteration);await mkdir(directory,{recursive:true});
  await writeFile(resolve(directory,"review.md"),"# Pending visual review\n\nThese captures have not been accepted. Inspect each image and record its task,\ncomposition, connections, detail, and verdict before completing this iteration.\n",{flag:"wx"}).catch(error=>{if(error.code!=="EEXIST")throw error;});
  return directory;
}
export async function captureIteration(page,{root,name,scenario,subject,documentFrame=page,imageTarget=page,extraSources=[]}){
  const iteration=process.env.MAGISTR_GUI_ITERATION;
  if(!iteration)return;
  if(!/^[a-z0-9][a-z0-9_-]*$/.test(name))throw new Error("Use a named GUI view without path separators.");
  const directory=await prepareIteration(root,iteration);
  for(const extension of ["png","json"]){
    const destination=resolve(directory,name+"."+extension);
    try{await access(destination);}catch(error){if(error.code==="ENOENT")continue;throw error;}
    throw new Error("Screenshot evidence already exists: "+destination+". Use a new iteration folder.");
  }
  await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  const image=await imageTarget.screenshot({animations:"disabled"});
  await writeFile(resolve(directory,name+".png"),image,{flag:"wx"});
  const ui=resolve(root,"crates/magistr-desktop/ui"),sources={};
  for(const file of (await readdir(ui)).sort()){
    const path=resolve(ui,file);sources[relative(root,path)]=sha(await readFile(path));
  }
  for(const path of [scenario,resolve(root,"crates/magistr-desktop/tests/browser/iteration_evidence.mjs"),resolve(root,"crates/magistr-desktop/tests/browser/fixture.mjs"),...extraSources])sources[relative(root,path)]=sha(await readFile(path));
  const receipt={iteration,view:name,captured_at:new Date().toISOString(),renderer:"headless WebKit",renderer_version:page.context().browser().version(),
    viewport:page.viewportSize(),appearance:await documentFrame.evaluate(()=>document.documentElement.dataset.theme??"system"),
    ...(subject?{story:subject}:{}),
    base_commit:execFileSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"}).trim(),source_hashes:sources,
    image:{file:name+".png",sha256:sha(image)},scope:"Production HTML/CSS/JavaScript with isolated observation fixtures. No native OS input, Rust command integration or assistive technology."};
  await writeFile(resolve(directory,name+".json"),JSON.stringify(receipt,null,2)+"\n",{flag:"wx"});
}