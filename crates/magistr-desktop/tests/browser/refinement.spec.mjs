// Focused screenshots use production assets and existing isolated domain observations.
import {test,expect,root,showProjects} from "./fixture.mjs";
import {captureIteration} from "./iteration_evidence.mjs";

const scene=process.env.MAGISTR_GUI_SCENE;
test("focused desktop refinement",async({app})=>{
  test.skip(!scene,"Select the affected area with MAGISTR_GUI_SCENE.");
  if(!scene.startsWith("welcome")&&scene!=="projects"){
    await app.getByRole("button",{name:"Review the release plan",exact:true}).click();
  }
  if(scene==="welcome-setup")await app.getByText("About project setup",{exact:true}).click();
  if(scene==="roadmap")await app.getByRole("tab",{name:"Roadmap",exact:true}).click();
  for(const [size,viewport] of [["desktop",{width:1280,height:820}],["compact",{width:760,height:550}]]){
    await app.setViewportSize(viewport);
    if(scene==="projects")await showProjects(app);
    for(const appearance of ["dark","light"]){
      await app.evaluate(value=>document.documentElement.dataset.theme=value,appearance);
      await app.mouse.move(0,0);
      await captureIteration(app,{root,name:scene+"-"+size+"-"+appearance,scenario:test.info().file});
      expect(await app.evaluate(()=>document.documentElement.scrollWidth>innerWidth)).toBe(false);
    }
  }
});
