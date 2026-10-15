import {desktopMeta,openRelease,visible} from "./desktop.mjs";
export default {...desktopMeta,title:"Roadmap"};
export const Overview={parameters:{desktop:{
  caption:"Desktop roadmap with dependency connections",
  task:"Follow the dependencies between planned work.",
  arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("tab",{name:"Roadmap",exact:true}));},
  verify:({doc,expect})=>{expect(visible(doc,"#work-browser")).toBe(true);expect(doc.querySelector("#work-tab-roadmap").getAttribute("aria-selected")).toBe("true");expect(doc.querySelectorAll(".surface-node").length).toBeGreaterThan(0);},
}}};
export const Search={parameters:{desktop:{
 caption:"Roadmap search results for draft",
 task:"Find work across tracks while keeping track and item filters visible.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.type(context.doc.querySelector("#work-browser-search"),"draft");},
 verify:({doc,expect})=>{expect(doc.querySelector("#work-browser-search").value).toBe("draft");expect(doc.querySelector("#work-browser-content").textContent).toContain("Restore the project conversation and draft");},
}}};
export const MoveView={parameters:{desktop:{...Overview.parameters.desktop,
 caption:"Roadmap with directional view controls expanded above the toolbar",
 task:"Move the canvas using directional buttons.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.doc.querySelector("#work-browser .surface-pan summary"));},
 verify:context=>{Overview.parameters.desktop.verify(context);context.expect(context.doc.querySelector("#work-browser .surface-pan").open).toBe(true);context.expect(context.doc.querySelector("#work-browser .surface-pan-actions").getBoundingClientRect().bottom).toBeLessThan(context.doc.querySelector("#work-browser .surface-tools").getBoundingClientRect().top);},
}}};
export const ArrangeItem={parameters:{desktop:{...Overview.parameters.desktop,
 caption:"Roadmap with controls to move the reopening item",
 task:"Adjust an item's position and return to the canvas.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.doc.querySelector('.surface-node[data-node="reopen"] .surface-grip'));},
 verify:context=>{Overview.parameters.desktop.verify(context);context.expect(visible(context.doc,".surface-arrange")).toBe(true);},
}}};
export const SelectedItem={parameters:{desktop:{
 caption:"Roadmap with the reopening item selected",
 task:"Read the selected item’s title, state and purpose beside its dependency graph.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Restore the project conversation and draft",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,"#work-browser-inspector .roadmap-item-heading")).toBe(true);expect(doc.querySelector("#work-browser-inspector h3").textContent).toBe("Restore the project conversation and draft");expect(doc.querySelector(".roadmap-item-heading").textContent).toContain("Return to the work you left.");},
}}};
export const BlockedItem={parameters:{desktop:{
 caption:"Roadmap with the release item blocked by an approval",
 task:"Read why work is blocked and the proposed change.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Publish the desktop release",exact:true}));},
 verify:({doc,expect})=>{expect(doc.querySelector("#work-browser-inspector h3")?.textContent).toBe("Publish the desktop release");expect(doc.querySelector("#work-browser-inspector").textContent).toContain("The release date needs approval.");},
}}};
export const MissingDependency={parameters:{desktop:{
 caption:"Installer item waiting for an unavailable prerequisite",task:"Understand why a required item cannot be opened.",
 arrange:async context=>{await Overview.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("button",{name:"Build the desktop installer",exact:true}));},
 verify:({doc,expect})=>{expect(doc.querySelector("#work-browser-inspector h3")?.textContent).toBe("Build the desktop installer");expect(doc.querySelector(".roadmap-relations [data-available=false]").textContent).toContain("Required item unavailable");},
}}};
export const DependentItem={parameters:{desktop:{
 caption:"Completed storage item with its dependent reopening work",task:"Follow the work that depends on the selected item.",
 arrange:async context=>{await SelectedItem.parameters.desktop.arrange(context);await context.userEvent.click(context.doc.querySelector(".roadmap-relations [data-focus-key=\"dependencies:foundation\"]"));},
 verify:({doc,expect})=>{expect(doc.querySelector("#work-browser-inspector h3")?.textContent).toBe("Store conversations and drafts");expect(doc.querySelector(".roadmap-relations [data-focus-key=\"dependents:reopen\"]").textContent).toBe("Restore the project conversation and draft");},
}}};
export const Requirements={parameters:{desktop:{
 caption:"Reopening item with completed and outstanding checklist entries and three requirements",
 task:"Review the checklist and complete authored requirements.",
 seed:seed=>{seed.workflow.roadmap.data.value.chunks.find(item=>item.id==="reopen").acceptance.push("Switching projects preserves each unsent message.", "Keyboard focus returns to the conversation you opened.");},
 arrange:async context=>{await SelectedItem.parameters.desktop.arrange(context);context.doc.querySelector(".roadmap-requirements").scrollIntoView({block:"end"});},
 verify:({doc,expect})=>{const panel=doc.querySelector("#work-browser-inspector").getBoundingClientRect(),list=doc.querySelector(".roadmap-requirements");expect(list.querySelectorAll("li").length).toBe(3);expect(list.getBoundingClientRect().bottom).toBeLessThanOrEqual(panel.bottom);expect(doc.querySelector(".work-checklist").getBoundingClientRect().top).toBeGreaterThanOrEqual(panel.top);},
}}};
