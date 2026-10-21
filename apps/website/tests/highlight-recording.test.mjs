import assert from "node:assert/strict";
import test from "node:test";
import {applyPatch} from "diff";
import {readFile} from "node:fs/promises";
import {recordedDiff,renderRecording} from "../scripts/highlight-recording.mjs";
import {formatTerminalFrame,terminalFrameHTML,terminalText} from "../src/data/terminal-format.mjs";
const plain=text=>text.replace(/\x1b\[[0-9;]*m/g,"");

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
 assert(formatTerminalFrame(frames[1].terminal,80,"dark").includes("\x1b[48;2;"));
 assert(!terminalText("\x1b]52;c;secret\x07\x9b2J").includes("\x1b"));
 assert(!terminalText("\x1b]52;c;secret\x07\x9b2J").includes("\x9b"));
});
test("the published recording retains real failures and changes with both palettes",async()=>{
 const recording=JSON.parse(await readFile(new URL("../public/demo/tic-tac-toe/recording.json",import.meta.url),"utf8"));
 const frames=await renderRecording(recording);
 assert.equal(frames.length,recording.events.length);
 for(let i=0;i<frames.length;i++){
  assert.equal(frames[i].id,recording.events[i].id);
  for(const theme of ["light","dark"])for(const columns of [32,40,80,120]){
   const output=formatTerminalFrame(frames[i].terminal,columns,theme,24);
   assert(output.startsWith("\x1b["));assert(!/\x1b\]/.test(output));
   const lines=plain(output).split(/\r?\n/);
   assert(lines.every(line=>Array.from(line).length<columns),frames[i].id+" fits "+columns+" columns");
   assert(lines.length<=26,"Preview stays inside its terminal screen");
   assert(lines.filter(Boolean).slice(1).every(line=>line.startsWith("  ")),frames[i].id+" retains its output gutter");
  }
 }
 assert(frames.some(f=>f.diff?.removed>0));
 assert(frames.some(f=>f.html.includes("output-error")));
 assert(frames.some(f=>f.html.includes("output-command")));
 const diagnostic=frames.find(f=>f.id==="action-49");
 assert(diagnostic.html.includes('output-code"><span style="">73 | /'));
 assert(diagnostic.html.includes("collapsible_if"));
});
test("tool results wrap as one group without duplicated editorial titles",async()=>{
 const [frame]=await renderRecording({events:[{id:"call",operation:"think_record_step",title:"Do not repeat this title",
  output:"A long result should keep its words together and its output beneath the tool.\n\n  - A nested item should keep its continuation under the item text."}]});
 const lines=plain(formatTerminalFrame(frame.terminal,40,"dark",30)).trimEnd().split("\r\n");
 assert.equal(lines[0],"• Called magistr.think_record_step");
 assert.equal(lines[1],"  └ A long result should keep its words");
 assert.equal(lines[2],"    together and its output beneath the");
 assert(lines.includes("      - A nested item should keep its"));
 assert(lines.includes("        continuation under the item"));
 assert(!lines.join("\n").includes("Do not repeat"));
 const html=terminalFrameHTML(frame.terminal);
 assert(html.includes("terminal-gutter"));assert(html.includes("A long result"));
});
test("numbered changes retain source indentation, signs and syntax colors",async()=>{
 const before="fn main() {\n    let score = 1;\n}\n",after="fn main() {\n    let score = 2;\n}\n";
 const frames=await renderRecording({events:[
  {id:"before",file:{path:"src/main.rs",content:before}},
  {id:"after",file:{path:"src/main.rs",content:after}},
 ]});
 const ansi=formatTerminalFrame(frames[1].terminal,80,"light"),text=plain(ansi);
 assert(text.includes("    2 -    let score = 1;"));
 assert(text.includes("    2 +    let score = 2;"));
 assert(text.includes("    3  }"));
 assert(ansi.includes("\x1b[48;2;255;235;233m"));
 assert(ansi.includes("\x1b[48;2;230;245;235m"));
 assert(!text.includes("@@"),"The terminal uses source line numbers; the full reader keeps unified hunks");
});
test("wrapping is independent of syntax token boundaries and bounds long output",()=>{
 const frame={rows:[{prefix:"  └ ",continuation:"    ",spans:[
  {text:"Keep this "},{text:"identi",light:"#123456"},{text:"fier intact while wrapping."},
 ]}]};
 assert.deepEqual(plain(formatTerminalFrame(frame,25,"light")).trimEnd().split("\r\n"),[
  "  └ Keep this identifier","    intact while","    wrapping.",
 ]);
 const rows=Array.from({length:30},(_,i)=>({prefix:"    ",continuation:"    ",spans:[{text:"Line "+i}]}));
 assert(plain(formatTerminalFrame({rows},40,"dark",8)).includes("… more in details"));
});
