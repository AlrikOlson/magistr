import {desktopMeta,openRelease,visible} from "./desktop.mjs";
export default {...desktopMeta,title:"Feedback"};
export const Overview={parameters:{desktop:{
 caption:"Feedback list with a researched concern and an idea added to the roadmap",task:"Scan feedback by kind, title and status.",
 arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("tab",{name:"Feedback",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,".feedback-list")).toBe(true);expect(doc.querySelectorAll(".feedback-row").length).toBe(2);},
}}};
export const DenseList={parameters:{desktop:{
 caption:"Feedback list with eight records across several review states",task:"Compare several feedback records without oversized rows.",
 seed:seed=>{const records=seed.workflow.signals.data.value,base=records[0];for(const [index,title] of ["Make it easier to find the conversation I used yesterday","Show which project has work waiting for my answer","Keep keyboard focus in the conversation after closing settings","Explain why an installation check failed and how to read its complete output","Let me copy the source reference from a research note","Keep the text readable when I reduce the window size"].entries())records.push({...structuredClone(base),id:"feedback-extra-"+index,kind:index%2?"idea":"concern",status:["new","triaged","researched"][index%3],content:{...base.content,summary:title}});},
 arrange:Overview.parameters.desktop.arrange,
 verify:({doc,expect})=>{expect(doc.querySelectorAll(".feedback-row").length).toBe(8);expect(visible(doc,".feedback-list")).toBe(true);},
}}};
export const Selected={parameters:{desktop:{
 caption:"Reopening feedback with its author, original request and expected behavior",task:"Read the complete feedback and its recorded context.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Keep my place when reopening a project",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,".feedback-detail")).toBe(true);expect(doc.querySelector(".feedback-detail h3").textContent).toBe("Keep my place when reopening a project");expect(doc.querySelector(".feedback-detail").textContent).toContain("When I reopen a project");},
}}};
export const Research={parameters:{desktop:{
 caption:"Feedback research note expanded with its summary, reasoning link and two sources",task:"Inspect evidence behind a feedback item.",
 arrange:async context=>{await Selected.parameters.desktop.arrange(context);await context.userEvent.click(context.doc.querySelector(".feedback-research details summary"));context.doc.querySelector(".feedback-research").scrollIntoView({block:"start"});},
 verify:({doc,expect})=>{expect(doc.querySelector(".feedback-research details").open).toBe(true);expect(doc.querySelectorAll(".feedback-sources article").length).toBe(2);expect(doc.querySelector(".feedback-research").textContent).toContain("The conversation owns the saved message.");},
}}};
export const NoResearch={parameters:{desktop:{
 caption:"Light-appearance feedback with no research recorded",task:"Read a short feedback item and see whether research exists.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Offer a light appearance for daytime work.",exact:true}));},
 verify:({doc,expect})=>{expect(doc.querySelector(".feedback-detail h3")?.textContent).toBe("Offer a light appearance for daytime work.");expect(doc.querySelector(".feedback-research h4").textContent).toBe("ResearchNone");},
}}};
export const NoResults={parameters:{desktop:{
 caption:"Feedback search with no matches",task:"Understand an empty search and revise the query.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.type(context.ui.getByLabelText("Search feedback",{exact:true}),"unmatched");},
 verify:({doc,expect})=>{expect(doc.querySelectorAll(".feedback-row").length).toBe(0);expect(doc.querySelector(".feedback-list").textContent).toContain("No feedback matches these filters.");},
}}};
