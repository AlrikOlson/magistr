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
export const Checks={parameters:{desktop:{
 caption:"Release checks grouped by installer task, showing failed, passed and reported attempts",task:"Compare every recorded check attempt.",
 arrange:async context=>{await Plan.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("radio",{name:"Checks",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,".work-check-index")).toBe(true);expect(doc.querySelectorAll(".work-check-attempt").length).toBe(1);expect(doc.querySelectorAll(".work-check").length).toBe(3);},
}}};
export const FailedCheck={parameters:{desktop:{
 caption:"Failed installer check expanded to its recorded command and failure",task:"Read the command, exit code and test failure.",
 arrange:async context=>{await Checks.parameters.desktop.arrange(context);await context.userEvent.selectOptions(context.ui.getByLabelText("Check results",{exact:true}),"failed");await context.userEvent.click(context.doc.querySelector(".work-check summary"));context.doc.querySelector(".work-check").scrollIntoView({block:"start"});},
 verify:({doc,expect})=>{expect(doc.querySelectorAll(".work-check").length).toBe(1);expect(doc.querySelector(".work-check").open).toBe(true);expect(doc.querySelector(".work-check").textContent).toContain("Expected the saved message; found an empty draft.");},
}}};
export const Activity={parameters:{desktop:{
 caption:"Release activity with a recorded code change and produced items",task:"Read what changed and find the produced files and review link.",
 arrange:async context=>{await Plan.parameters.desktop.arrange(context);await context.userEvent.click(context.ui.getByRole("radio",{name:"Activity and produced items",exact:true}));},
 verify:({doc,expect})=>{expect(visible(doc,".work-activity-index")).toBe(true);expect(doc.querySelectorAll(".work-activity-entry").length).toBe(3);},
}}};
export const ProducedItems={parameters:{desktop:{
 caption:"Activity filtered to the release checklist and review link",task:"Copy a produced reference or open its review link.",
 arrange:async context=>{await Activity.parameters.desktop.arrange(context);await context.userEvent.selectOptions(context.ui.getByLabelText("Entries",{exact:true}),"items");},
 verify:({doc,expect})=>{expect(doc.querySelectorAll(".work-activity-entry").length).toBe(2);expect(doc.querySelector(".work-activity-index").textContent).toContain("docs/release-checklist.md");},
}}};
export const TaskActivity={parameters:{desktop:{
 caption:"Installer task activity expanded with its recorded result and references",task:"Read activity in the context of its task.",
 arrange:async context=>{await Plan.parameters.desktop.arrange(context);await context.userEvent.click(context.doc.querySelector(".work-activity summary"));context.doc.querySelector(".work-activity").scrollIntoView({block:"start"});},
 verify:({doc,expect})=>{expect(doc.querySelector(".work-activity").open).toBe(true);expect(doc.querySelector(".work-activity").textContent).toContain("The saved message now appears");},
}}};
