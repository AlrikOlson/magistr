import {desktopMeta,openRelease,visible} from "./desktop.mjs";
export default {...desktopMeta,title:"Roadmap"};
export const Overview={parameters:{desktop:{
  caption:"Desktop roadmap with dependency connections",
  task:"Follow the dependencies between planned work.",
  arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("tab",{name:"Roadmap",exact:true}));},
  verify:({doc,expect})=>{expect(visible(doc,"#work-browser")).toBe(true);expect(doc.querySelector("#work-tab-roadmap").getAttribute("aria-selected")).toBe("true");expect(doc.querySelectorAll(".surface-node").length).toBeGreaterThan(0);},
}}};
