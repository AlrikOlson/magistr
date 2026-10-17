import {test} from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,mkdir,writeFile,rm,realpath} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join,relative,sep} from "node:path";
import {once} from "node:events";
import {setTimeout} from "node:timers/promises";
import {createServer} from "vite";
import catalog from "../.storybook/main.mjs";

test("catalog watches source changes without traversing native build products",async()=>{
  const root=await realpath(await mkdtemp(join(tmpdir(),"magistr-storybook-watch-")));
  let server;
  try{
    const inputs=["ui/style.css","stories/example.stories.mjs","tests/browser/observations.mjs"];
    const outputs=["target/debug/.fingerprint/example/result","target/aarch64-apple-darwin/release/bundle/result","gen/schemas/desktop.json","tests/__pycache__/fixture.pyc"];
    for(const file of [...inputs,...outputs]){
      await mkdir(join(root,file,".."),{recursive:true});
      await writeFile(join(root,file),"initial");
    }
    const config=await catalog.viteFinal({
      configFile:false,root,logLevel:"silent",appType:"custom",
      optimizeDeps:{noDiscovery:true,include:[]},
      server:{middlewareMode:true,ws:false,watch:{usePolling:true,interval:20}},
    });
    server=await createServer(config);
    await once(server.watcher,"ready",{signal:AbortSignal.timeout(5000)});
    const watched=()=>Object.entries(server.watcher.getWatched()).flatMap(([directory,files])=>files.map(file=>relative(root,join(directory,file)).split(sep).join("/")));
    // Chokidar can report ready before its nested directory registrations settle.
    const deadline=Date.now()+5000;
    while(!inputs.every(file=>watched().includes(file))){
      assert.ok(Date.now()<deadline,"source watch registration timed out: "+JSON.stringify(watched()));
      await setTimeout(20);
    }
    for(const path of watched())assert.ok(!/(^|\/)(target|gen|__pycache__)(\/|$)/.test(path),"generated path must not be watched: "+path);
    for(const file of inputs){
      const changed=once(server.watcher,"change",{signal:AbortSignal.timeout(5000)});
      await writeFile(join(root,file),"changed");
      assert.equal((await changed)[0],join(root,file));
    }
  }finally{
    await server?.close();
    await rm(root,{recursive:true,force:true});
  }
});
