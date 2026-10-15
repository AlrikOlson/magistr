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
export const SelectedStep={parameters:{desktop:{
 caption:"Saved-message decision with its outcome and supporting installation step",task:"Read a decision and follow its supporting evidence.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Keep each conversation with its saved message",exact:true}));},
 verify:({doc,expect})=>{expect(doc.querySelector("#work-browser-inspector h3")?.textContent).toBe("Keep each conversation with its saved message");expect(doc.querySelector(".reasoning-links").textContent).toContain("Inspect the installation result");expect(visible(doc,".reasoning-step")).toBe(true);},
}}};
export const InvestigationStep={parameters:{desktop:{
 caption:"Investigation decision with no related steps recorded",task:"Read an investigation’s conclusion and available context.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Check whether reopening needs a second saved message",exact:true}));},
 verify:({doc,expect})=>{expect(doc.querySelector("#work-browser-inspector h3")?.textContent).toBe("Check whether reopening needs a second saved message");expect(doc.querySelector(".reasoning-links").textContent).toContain("None");},
}}};
export const SupportingDetails={parameters:{desktop:{
 caption:"Selected decision scrolled to context, rationale, reasoning and next action",task:"Read the complete supporting fields and copy an exact field.",
 arrange:async context=>{await SelectedStep.parameters.desktop.arrange(context);context.doc.querySelector(".reasoning-evidence").scrollIntoView({block:"start"});},
 verify:({doc,expect})=>{expect(doc.querySelector(".reasoning-evidence").textContent).toContain("A second saved message would drift");const rect=doc.querySelector(".reasoning-evidence").getBoundingClientRect(),panel=doc.querySelector("#work-browser-inspector").getBoundingClientRect();expect(rect.top).toBeGreaterThanOrEqual(panel.top);expect(rect.bottom).toBeLessThanOrEqual(panel.bottom);},
}}};
