import {readFileSync,writeFileSync,mkdirSync,copyFileSync} from "node:fs";
import {createHash} from "node:crypto";
import {execFileSync} from "node:child_process";
import {resolve,dirname} from "node:path";
import {fileURLToPath} from "node:url";
const root=resolve(dirname(fileURLToPath(import.meta.url)),"../../.."),game=resolve(root,"examples/bevy-tic-tac-toe"),out=resolve(root,"apps/website/public/demo/tic-tac-toe");
const hash=bytes=>createHash("sha256").update(bytes).digest("hex");
const files=["Cargo.toml","Cargo.lock","README.md","build-web.mjs","src/lib.rs","src/main.rs","examples/compare_opponents.rs","web/index.html",".gitignore"];
mkdirSync(out,{recursive:true});
const sources=files.map(path=>({path,sha256:hash(readFileSync(resolve(game,path)))}));
execFileSync("python3",["-c",`import sys,json,zipfile,pathlib
root=pathlib.Path(sys.argv[1])
with zipfile.ZipFile(sys.argv[2],"w",zipfile.ZIP_DEFLATED) as z:
 for name in json.loads(sys.argv[3]):
  info=zipfile.ZipInfo("tic-tac-toe/"+name,date_time=(2026,10,21,0,0,0))
  info.compress_type=zipfile.ZIP_DEFLATED
  info.external_attr=0o100644<<16
  z.writestr(info,(root/name).read_bytes())
`,game,resolve(out,"source.zip"),JSON.stringify(files)]);
const evidence="docs/evidence/desktop-iterations/2026-10-21-bevy-rematch-02";
for(const name of ["desktop-result.png","compact-rematch.png"])copyFileSync(resolve(root,evidence,name),resolve(out,name));
const assets=["recording.json","source.zip","desktop-result.png","compact-rematch.png"].map(path=>({path,sha256:hash(readFileSync(resolve(out,path)))}));
writeFileSync(resolve(out,"manifest.json"),JSON.stringify({version:1,engine:"Bevy 0.20.0",sources,assets,evidence},null,2)+"\n");
console.log("Published allowlisted game source, records and reviewed captures.");
