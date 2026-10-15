import {desktopMeta,visible} from "./desktop.mjs";
export default {...desktopMeta,title:"Projects/Directory"};
const directory={
  task:"Choose a project or open a folder.",
  verify:({doc,expect})=>{
    expect(visible(doc,"#welcome")).toBe(true);
    expect(visible(doc,"#project-search")).toBe(true);
    expect(doc.querySelector("#view-title").textContent).toBe("Projects");
  },
};
export const List={parameters:{desktop:{...directory,caption:"Projects directory; list view; project setup explanation collapsed",
  verify:context=>{directory.verify(context);context.expect(context.doc.querySelector(".welcome-hint").open).toBe(false);}}}};
export const SetupExpanded={parameters:{desktop:{...directory,caption:"Projects directory; list view; project setup explanation expanded",
  arrange:({doc})=>{doc.querySelector(".welcome-hint").open=true;},
  verify:context=>{directory.verify(context);context.expect(context.doc.querySelector(".welcome-hint").open).toBe(true);}}}};
export const Map={parameters:{desktop:{
 caption:"Projects directory in map view",
 task:"Follow each project to its open conversations.",
 arrange:async context=>{await context.userEvent.click(context.ui.getByRole("button",{name:"Map",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,"#welcome .work-surface")).toBe(true);expect(doc.querySelector('#welcome [aria-label="Projects canvas"]').getBoundingClientRect().height).toBeGreaterThan(100);},
}}};
