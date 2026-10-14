import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
const directory=fileURLToPath(new URL(".",import.meta.url));
for(const script of ["native_window.py","draft_persistence_journey.py","work_journey.py"]) {
  test(script+" refuses before creating evidence or controlling the desktop",()=>{
    const temporary=mkdtempSync(join(tmpdir(),"magistr-native-opt-in-"));
    try {
      const evidence=join(temporary,"untouched");
      const result=spawnSync("python3",[join(directory,script),"/nonexistent/magistr-desktop","--evidence",evidence],{encoding:"utf8",timeout:5000});
      assert.equal(result.status,2,result.stderr);
      assert.match(result.stderr,/controls the foreground desktop/);
      assert.match(result.stderr,/--allow-foreground/);
      assert.equal(existsSync(evidence),false);
    } finally {rmSync(temporary,{recursive:true,force:true});}
  });
}
