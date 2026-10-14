import test from "node:test";
import assert from "node:assert/strict";
import { clipboardFiles, inputIdentity } from "../ui/attachments.mjs";
const limits={file_bytes:4,total_bytes:6,count:2};
function paste(files=[]) {
  return {clipboardData:{files},prevented:false,preventDefault(){this.prevented=true;}};
}
test("ordinary clipboard text remains native editing",()=>{
  const event=paste();
  assert.deepEqual(clipboardFiles(event,limits),[]);
  assert.equal(event.prevented,false);
});
test("file clipboard data is bounded before reading its bytes",()=>{
  for(const files of [[{name:"large",size:5}],[{name:"a",size:1},{name:"b",size:1},{name:"c",size:1}],[{name:"a",size:4},{name:"b",size:4}]]) {
    const event=paste(files);
    assert.throws(()=>clipboardFiles(event,limits));
    assert.equal(event.prevented,true);
  }
  assert.throws(()=>clipboardFiles(paste([{name:"b",size:3}]),limits,[{size:4}]),/exceed/);
});
test("supported paste keeps the original files and counts existing staged files",()=>{
  const files=[{name:"日本.txt",size:2}],event=paste(files);
  assert.deepEqual(clipboardFiles(event,limits,[{size:4}]),files);
  assert.equal(event.prevented,true);
  assert.throws(()=>clipboardFiles(paste(files),limits,[{size:1},{size:1}]),/up to 2/);
});
test("attachment receipt identity preserves selection IDs, names and ordering",()=>{
  const files=[{id:"1",name:"a",content_hash:"same"},{id:"2",name:"b",content_hash:"same"}];
  assert.notEqual(inputIdentity({text:"",attachments:files}),inputIdentity({text:"",attachments:[...files].reverse()}));
  assert.notEqual(inputIdentity({text:"",attachments:files}),inputIdentity({text:"",attachments:files.map(file=>({...file,id:"new"}))}));
});
