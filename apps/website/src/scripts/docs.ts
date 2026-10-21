const guideList = document.querySelector<HTMLDetailsElement>(".docs-guide-list");
const compact = matchMedia("(max-width: 800px)");
const updateGuides = () => {
  if (!guideList) return;
  const focused = document.activeElement;
  const inNavigation = guideList.contains(focused);
  guideList.open = !compact.matches;
  if (!compact.matches) {
    const current = guideList.querySelector<HTMLAnchorElement>('[aria-current="page"]');
    if (current) {
      const bounds = guideList.getBoundingClientRect();
      const item = current.getBoundingClientRect();
      if (item.top < bounds.top || item.bottom > bounds.bottom) {
        guideList.scrollTop += item.top - bounds.top - (bounds.height - item.height) / 2;
      }
    }
  }
  if (inNavigation) {
    if (compact.matches) guideList.querySelector("summary")?.focus();
    else if (focused === guideList.querySelector("summary")) guideList.querySelector<HTMLAnchorElement>('[aria-current="page"]')?.focus();
  }
};
updateGuides();
compact.addEventListener("change", updateGuides);
guideList?.addEventListener("keydown", event => {
  if (event.key === "Escape" && compact.matches && guideList.open) {
    guideList.open = false;
    guideList.querySelector("summary")?.focus();
  }
});

const input = document.querySelector<HTMLInputElement>(".docs-search-input");
const results = document.querySelector<HTMLDivElement>(".docs-search-results");
if (input && results) {
  let timer: ReturnType<typeof setTimeout>;
  let revision = 0;
  let pagefindPromise: Promise<any> | undefined;
  const search = async (query: string, request: number) => {
    results.textContent = "Searching…";
    results.setAttribute("aria-busy", "true");
    try {
      const indexUrl = "/pagefind/pagefind.js";
      pagefindPromise ??= import(/* @vite-ignore */ indexUrl);
      const pagefind = await pagefindPromise;
      if (request !== revision) return;
      const found = await pagefind.search(query, { filters: { collection: "docs" } });
      const matches = await Promise.all(found.results.slice(0, 5).map((result: any) => result.data()));
      if (request !== revision) return;
      results.replaceChildren();
      if (!matches.length) results.textContent = "No results. Try a different word.";
      for (const data of matches) {
        const link = document.createElement("a");
        link.href = data.url;
        link.textContent = data.meta?.title || data.url;
        results.appendChild(link);
      }
    } catch {
      if (request === revision) {
        results.textContent = "Search couldn’t load. Try reloading, or browse documentation below.";
      }
    } finally {
      if (request === revision) results.setAttribute("aria-busy", "false");
    }
  };
  input.addEventListener("input", () => {
    clearTimeout(timer);
    const request = ++revision;
    const query = input.value.trim();
    results.replaceChildren();
    results.setAttribute("aria-busy", "false");
    if (query) timer = setTimeout(() => search(query, request), 180);
  });
  input.addEventListener("keydown", event => {
    if (event.key === "Escape") {
      input.value = "";
      input.dispatchEvent(new Event("input"));
    }
  });
  addEventListener("pagehide", () => {
    clearTimeout(timer);
    revision++;
    pagefindPromise?.then(pagefind => pagefind.destroy()).catch(() => {});
  });
  addEventListener("pageshow", event => {
    if (event.persisted && input.value.trim()) input.dispatchEvent(new Event("input"));
  });
}
