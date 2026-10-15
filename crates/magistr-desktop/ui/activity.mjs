// Summaries derive from the accepted session catalog; no activity is persisted here.
export const activityKinds = [
  ["working", "Working", "success"],
  ["waiting", "Need your answer", "warning"],
  ["idle", "Not running", "muted"],
  ["disconnected", "Disconnected", "danger"],
];
export function sessionActivity(session) {
  if (session.closed) return null;
  if (session.status === "Connection lost") return "disconnected";
  if (session.pending?.length) return "waiting";
  return session.activity?.active ? "working" : "idle";
}
export function catalogActivity(sessions) {
  const counts = Object.fromEntries(activityKinds.map(([kind]) => [kind, 0]));
  for (const session of sessions) {
    const kind = sessionActivity(session);
    if (kind) counts[kind]++;
  }
  return counts;
}
export class ActivityOverview {
  constructor(root) {
    this.root = root;
    const document = root.ownerDocument;
    const node = (tag, className, text = "") => {
      const element = document.createElement(tag);
      element.className = className; element.textContent = text; return element;
    };
    this.total = node("strong", "");
    const label = node("span", "", "open conversations");
    const totalLine = node("div", "activity-total");totalLine.append(this.total,label);
    const body = node("div", "activity-body"),heading=node("header","activity-heading");
    heading.append(node("h2","","Across your projects"),totalLine);body.append(heading);
    const list = node("dl", "activity-counts");
    this.values = new Map();
    for (const [kind, title] of activityKinds) {
      const item = node("div", "activity-count"); item.dataset.kind = kind;
      const value = node("dd", "", "0");
      item.append(node("dt", "", title), value); list.append(item);
      this.values.set(kind, value);
    }
    this.spectrum = node("div", "activity-spectrum"); this.spectrum.setAttribute("aria-hidden", "true");
    for (const [kind] of activityKinds) { const segment = node("span", ""); segment.dataset.kind = kind; this.spectrum.append(segment); }
    body.append(list, this.spectrum); root.append(body);
  }
  render(sessions) {
    const counts = catalogActivity(sessions);
    const total = Object.values(counts).reduce((sum, value) => sum + value, 0);
    this.total.textContent = total.toLocaleString();
    this.root.setAttribute("aria-label", total + " open conversations across your projects");
    for (const [kind] of activityKinds) {
      const count = counts[kind];
      this.values.get(kind).textContent = count.toLocaleString();
      this.values.get(kind).parentElement.dataset.empty = String(!count);
      this.values.get(kind).parentElement.style.setProperty("--share",total?`${count/total*100}%`:"0%");
    }
    this.root.dataset.empty = String(!total);
    for (const segment of this.spectrum.children) { const count = counts[segment.dataset.kind]; segment.hidden = !count; segment.style.flexGrow = String(count); }
  }
}