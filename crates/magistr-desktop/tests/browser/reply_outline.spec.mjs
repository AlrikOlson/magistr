import {test,expect,root} from "./fixture.mjs";
import {outlineScene} from "./design_scenes.mjs";
import {captureIteration} from "./iteration_evidence.mjs";
test("reply outline leaves content visible in compact windows and moves focus to the chosen section",async({app})=>{
  await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  await outlineScene(app,name=>captureIteration(app,{root,name:"20-"+name.replace(".png",""),scenario:test.info().file}),expect);
});
