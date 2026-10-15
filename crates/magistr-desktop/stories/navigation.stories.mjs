import {desktopMeta,visible} from "./desktop.mjs";
export default {...desktopMeta,title:"Projects/Navigation"};
export const Sidebar={globals:{viewport:{value:"desktop",isRotated:false}},tags:["desktop-only"],parameters:{desktop:{
  caption:"Projects directory with project navigation and waiting request sidebar",
  task:"Review a waiting request and choose a project conversation from the sidebar.",
  verify:({doc,expect})=>{expect(visible(doc,"#welcome")).toBe(true);expect(visible(doc,"#project-navigation")).toBe(true);expect(visible(doc,"#toggle-projects")).toBe(false);},
}}};
export const CompactPanel={globals:{viewport:{value:"compact",isRotated:false}},tags:["compact-only"],parameters:{desktop:{
  caption:"Expanded project navigation panel with a waiting request; project directory hidden",
  task:"Review a waiting request and choose a conversation from the compact project panel.",
  arrange:async({ui,userEvent})=>{await userEvent.click(ui.getByRole("button",{name:"Projects",exact:true}));},
  verify:({doc,expect})=>{expect(visible(doc,"#project-navigation")).toBe(true);expect(doc.querySelector("#toggle-projects").getAttribute("aria-expanded")).toBe("true");expect(visible(doc,"#welcome")).toBe(false);},
}}};
