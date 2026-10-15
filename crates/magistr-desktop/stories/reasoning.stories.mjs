import {desktopMeta,openRelease,visible} from "./desktop.mjs";
export default {...desktopMeta,title:"Reasoning"};
export const Overview={parameters:{desktop:{
 caption:"Reasoning graph with a saved-message decision and its merged investigation",
 task:"Follow the investigation and its conclusion.",
 arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("tab",{name:"Reasoning",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,".reasoning-spatial")).toBe(true);expect(doc.querySelectorAll(".decision-piece").length).toBeGreaterThan(0);},
}}};
