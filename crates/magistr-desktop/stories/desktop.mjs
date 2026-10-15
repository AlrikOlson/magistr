// Stories mount the production document; only the native bridge is simulated.
import html from "../ui/index.html?raw";
import { expect, waitFor, within, userEvent } from "storybook/test";
import { workspaceSeed, install } from "../tests/browser/observations.mjs";

export function renderDesktop(_args, context) {
  const frame=document.createElement("iframe");
  frame.className="desktop-preview";
  frame.title=context.title+" — "+context.name;
  const seed=workspaceSeed();
  seed.preferences.theme=context.globals.appearance??"dark";
  context.parameters.desktop.seed?.(seed);
  const script="("+install.toString()+")("+JSON.stringify(seed).replaceAll("<","\\u003c")+");";
  frame.srcdoc=html.replace("<head>",'<head><base href="/desktop-assets/"><script>'+script+"<"+"/script>");
  return frame;
}

export function visible(doc,selector) {
  const el=doc.querySelector(selector);
  return !!el && el.getClientRects().length>0 && doc.defaultView.getComputedStyle(el).visibility!=="hidden";
}

export async function verifyDesktop({canvasElement,parameters,globals,id,title,name}) {
  const frame=canvasElement.querySelector("iframe");
  try {
  await waitFor(()=>expect(frame.contentWindow?.__fixture).toBeTruthy());
  const doc=frame.contentDocument,ui=within(doc.body),state=parameters.desktop;
  await waitFor(()=>expect(doc.querySelectorAll("#projects .project-group").length).toBe(2));
  await doc.fonts.ready;
  await waitFor(()=>expect(doc.documentElement.dataset.theme).toBe(globals.appearance??"dark"));
  await state.arrange?.({doc,ui,window:frame.contentWindow,userEvent:userEvent.setup({document:doc}),expect,waitFor});
  await waitFor(()=>state.verify({doc,ui,expect,visible}));
  frame.contentWindow.__verifyStory=()=>state.verify({doc,ui,expect,visible});
  frame.contentWindow.__storyState={
    id,title,name,caption:state.caption,task:state.task,
    appearance:doc.documentElement.dataset.theme,
    viewport:{width:frame.contentWindow.innerWidth,height:frame.contentWindow.innerHeight},
    visible_heading:visible(doc,"#view-title")?doc.querySelector("#view-title").textContent:null,
    selected_view:visible(doc,'[role="tab"][aria-selected="true"]')?doc.querySelector('[role="tab"][aria-selected="true"]').textContent:null,
  };
  frame.dataset.storyReady=id;
  } catch(error) { frame.dataset.storyError=error.message; throw error; }
}

export const desktopMeta={render:renderDesktop,play:verifyDesktop};
export async function openRelease({ui,userEvent,waitFor,doc,expect}) {
  await userEvent.click(ui.getByRole("button",{name:"Continue Review the release plan in Magistr",exact:true}));
  await waitFor(()=>expect(visible(doc,"#workspace")).toBe(true));
}
export function conversationVisible({doc,expect,visible}) {
  expect(visible(doc,"#workspace")).toBe(true);
  expect(doc.querySelector("#work-tab-conversation").getAttribute("aria-selected")).toBe("true");
  expect(doc.querySelector("#view-title").textContent).toBe("Review the release plan");
}