import {desktopMeta,openRelease,visible} from "./desktop.mjs";
export default {...desktopMeta,title:"Reasoning"};
export const Overview={parameters:{desktop:{
 caption:"Reasoning graph with a saved-message decision and its merged investigation",
 task:"Follow the investigation and its conclusion.",
 arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("tab",{name:"Reasoning",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,".reasoning-spatial")).toBe(true);expect(doc.querySelectorAll(".decision-piece").length).toBeGreaterThan(0);},
}}};
export const Investigation={parameters:{desktop:{
 caption:"Reasoning filtered to the saved-message investigation",task:"Choose an investigation and follow its conclusion.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.selectOptions(context.ui.getByLabelText("Investigation",{exact:true}),"branch:storage");},
 verify:({doc,expect})=>{expect(doc.querySelector("#work-browser-branch").value).toBe("branch:storage");expect(doc.querySelectorAll(".decision-piece").length).toBe(1);expect(doc.querySelector(".thread-heading").textContent).toContain("Does reopening need another saved message?");},
}}};
export const Search={parameters:{desktop:{
 caption:"Reasoning search showing the installation decision",task:"Find a recorded decision across reasoning histories.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.type(context.ui.getByLabelText("Search all reasoning",{exact:true}),"installation");},
 verify:({doc,expect})=>{expect(doc.querySelector("#work-browser-reasoning-search").value).toBe("installation");expect(doc.querySelector("#work-browser-content").textContent).toContain("Inspect the installation result");expect(visible(doc,".reasoning-spatial")).toBe(false);},
}}};
