// Enhance the static reading order into keyboard-operable tabs.
for (const group of document.querySelectorAll<HTMLElement>("[data-tabset]")) {
  const list = group.querySelector<HTMLElement>("[data-tablist]")!;
  const tabs = [...list.querySelectorAll<HTMLButtonElement>("[data-tab]")];
  const panels = [...group.querySelectorAll<HTMLElement>(":scope > [data-panel]")];
  const select = (index: number, focus = false) => {
    tabs.forEach((tab, current) => {
      tab.setAttribute("aria-selected", String(current === index));
      tab.tabIndex = current === index ? 0 : -1;
      panels[current].hidden = current !== index;
    });
    if (focus) tabs[index].focus();
  };
  list.hidden = false;
  list.setAttribute("role", "tablist");
  tabs.forEach((tab, index) => {
    tab.setAttribute("role", "tab");
    panels[index].setAttribute("role", "tabpanel");
    panels[index].setAttribute("aria-labelledby", tab.id);
    panels[index].tabIndex = 0;
    tab.addEventListener("click", () => select(index));
    tab.addEventListener("keydown", (event) => {
      let next: number;
      if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
      else if (event.key === "ArrowLeft") next = (index - 1 + tabs.length) % tabs.length;
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = tabs.length - 1;
      else return;
      event.preventDefault();
      select(next, true);
    });
  });
  select(0);
}
