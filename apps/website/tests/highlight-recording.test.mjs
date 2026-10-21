import assert from "node:assert/strict";
import test from "node:test";
import {applyPatch} from "diff";
import {readFile} from "node:fs/promises";
import {recordedDiff,renderRecording,terminalText} from "../scripts/highlight-recording.mjs";

test("recorded diffs reconstruct the new file, including replacements and missing final newlines",async()=>{
 for(const [before,after]of [
  ["","pub fn main() {}\n"],
  ["pub fn score() -> u8 {\n    1\n}\n","pub fn score() -> u8 {\n    2\n}\n"],
  ["a\nb","a\nc"],["a\nb\n","a\n"],["a\n","a\nb\n"],
 ]){
  const result=await recordedDiff(before,after,"src/lib.rs");
  const patch="--- src/lib.rs\n+++ src/lib.rs\n"+result.lines.map(l=>l.text).join("\n")+"\n";
  assert.equal(applyPatch(before,patch),after);
  assert.equal(result.added,result.lines.filter(l=>l.kind==="add").length);
  assert.equal(result.removed,result.lines.filter(l=>l.kind==="remove").length);
 }
});
test("highlighted versions use only earlier snapshots and escape HTML and terminal controls",async()=>{
 const first='pub const LABEL: &str = "<script>alert(1)</script>";\n';
 const second=first+'pub const LATER: u8 = 2;\n';
 const frames=await renderRecording({events:[
  {id:"one",kind:"change",operation:"write",title:"Initial source",output:first,file:{path:"src/lib.rs",content:first}},
  {id:"two",kind:"change",operation:"write",title:"Add a value",output:second,file:{path:"src/lib.rs",content:second}},
 ]});
 assert(!frames[0].html.includes("LATER"));
 assert(!frames[0].html.includes("<script>"));
 assert(frames[0].html.includes("--shiki-dark"));
 assert.equal(frames[1].diff.added,1);assert.equal(frames[1].diff.removed,0);
 assert.equal(frames[0].diff.first,true);assert.equal(frames[1].diff.first,false);
 assert(frames[1].terminal.dark.includes("\x1b[48;2;"));
 assert(!terminalText("\x1b]52;c;secret\x07\x9b2J").includes("\x1b"));
 assert(!terminalText("\x1b]52;c;secret\x07\x9b2J").includes("\x9b"));
});
test("the published recording retains real failures and changes with both palettes",async()=>{
 const recording=JSON.parse(await readFile(new URL("../public/demo/tic-tac-toe/recording.json",import.meta.url),"utf8"));
 const frames=await renderRecording(recording);
 assert.equal(frames.length,recording.events.length);
 for(let i=0;i<frames.length;i++){
  assert.equal(frames[i].id,recording.events[i].id);
  assert(frames[i].terminal.light);assert(frames[i].terminal.dark);
  assert(!/\x1b\]/.test(frames[i].terminal.light),"Only generated SGR and erase-line escapes enter the terminal");
 }
 assert(frames.some(f=>f.diff?.removed>0));
 assert(frames.some(f=>f.html.includes("output-error")));
 assert(frames.some(f=>f.html.includes("output-command")));
 const diagnostic=frames.find(f=>f.id==="action-49");
 assert(diagnostic.html.includes('output-code"><span style="">73 | /'));
 assert(diagnostic.html.includes("collapsible_if"));
});
