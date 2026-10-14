import test from "node:test";
import assert from "node:assert/strict";
import { remember, EditorPlaces } from "../ui/reading.mjs";
import { targetKey } from "../ui/context.mjs";
test("view coordinates have a finite recent-conversation bound",()=>{
  const places=new Map();
  for(let index=0;index<65;index++)remember(places,index,{top:index});
  assert.equal(places.size,64);assert.equal(places.has(0),false);
  remember(places,1,{top:900});remember(places,66,{top:10});
  assert.equal(places.has(1),true);assert.equal(places.has(2),false);
});
test("closed conversation claims remove editor coordinates and prevent their recreation",()=>{
  const target={address:{project:"project",workspace:"/work",session:"one"},conversation:"draft"};
  const editor={value:"Draft",selectionStart:1,selectionEnd:4,selectionDirection:"forward",scrollTop:0};
  const places=new EditorPlaces(editor);
  places.retain(new Set([targetKey(target)]));places.capture(target,4);assert.equal(places.places.size,1);
  places.retain(new Set());assert.equal(places.places.size,0);
  places.capture(target,4);assert.equal(places.places.size,0);
});
