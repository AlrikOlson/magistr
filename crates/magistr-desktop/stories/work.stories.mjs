import {desktopMeta,openRelease,visible} from "./desktop.mjs";
export default {...desktopMeta,title:"Work"};
export const Plan={parameters:{desktop:{
 caption:"Current desktop release plan with completed, active and planned tasks",task:"Scan the objective and task states.",
 arrange:async context=>{await openRelease(context);await context.userEvent.click(context.ui.getByRole("tab",{name:"Current and saved work",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,".work-record[data-run=\"release-run\"]")).toBe(true);expect(doc.querySelectorAll(".work-tasks>li").length).toBe(3);expect(doc.querySelector(".work-record-heading h3").textContent).toBe("Prepare the desktop release");},
}}};
export const BlockedTask={parameters:{desktop:{...Plan.parameters.desktop,
 caption:"Current release plan with the installer task blocked",task:"Identify blocked work and read the reason.",
 seed:seed=>{const task=seed.workflow.ship.data.value.current.tasks[1];task.status="blocked";task.blocked_reason="The signing certificate needs to be renewed before the installer can be built.";},
 verify:context=>{Plan.parameters.desktop.verify(context);context.expect(context.doc.querySelector(".work-tasks [data-status=blocked]").textContent).toContain("The signing certificate needs to be renewed");},
}}};
export const Requirements={parameters:{desktop:{...Plan.parameters.desktop,
 caption:"Current release objective with its two requirements expanded",task:"Read the requirements before reviewing tasks.",
 arrange:async context=>{await Plan.parameters.desktop.arrange(context);await context.userEvent.click(context.doc.querySelector(".work-objective-details summary"));},
 verify:context=>{Plan.parameters.desktop.verify(context);context.expect(context.doc.querySelector(".work-objective-details").open).toBe(true);},
}}};
