import test from "node:test";
import assert from "node:assert/strict";
import { projectGroups } from "../ui/navigation.mjs";
test("duplicate names remain distinct while linked working copies stay under their actual owner",()=>{
  const projects=[{id:"one",name:"App",directory:"/one/app"},{id:"two",name:"App",directory:"/two/app"}];
  const sessions=[
    {address:{project:"one",workspace:"/one/app",session:"main"}},
    {address:{project:"one",workspace:"/worktrees/fix",session:"fix"}},
    {address:{project:"two",workspace:"/two/app",session:"other"}},
  ];
  const groups=projectGroups(projects,sessions);
  assert.deepEqual(groups.map(group=>group.sessions.map(session=>session.address.session)),[["main","fix"],["other"]]);
  assert.equal(groups[0].directory,"/one/app");
  assert.equal(groups[1].directory,"/two/app");
});
test("a newly observed resident appears before the next recent-project refresh",()=>{
  const session={name:"New project",address:{project:"new",workspace:"/new",session:"fresh"}};
  assert.deepEqual(projectGroups([], [session]),[{id:"new",name:"New project",directory:"/new",sessions:[session]}]);
});
