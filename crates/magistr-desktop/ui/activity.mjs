import { ActivityMaterial } from "./activity_material.mjs";

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
    this.wheel = node("div", "activity-wheel");
    this.wheel.setAttribute("aria-hidden", "true");
    this.total = node("strong", "");
    const label = node("span", "", "open");
    const totalLine = node("div", "activity-total");totalLine.append(this.total, label);this.wheel.append(totalLine);
    const body = node("div", "activity-body");
    body.append(node("h2", "", "Across your projects"));
    const list = node("dl", "activity-counts");
    this.values = new Map();
    for (const [kind, title] of activityKinds) {
      const item = node("div", "activity-count"); item.dataset.kind = kind;
      const value = node("dd", "", "0");
      item.append(node("dt", "", title), value); list.append(item);
      this.values.set(kind, value);
    }
    body.append(list); root.append(this.wheel, body);
    this.material = new ActivityMaterial(this.wheel);
  }
  render(sessions) {
    const counts = catalogActivity(sessions);
    const total = Object.values(counts).reduce((sum, value) => sum + value, 0);
    this.total.textContent = total.toLocaleString();
    this.root.setAttribute("aria-label", total + " open conversations across your projects");
    let offset = 0;
    const stops = [];
    for (const [kind, , color] of activityKinds) {
      const count = counts[kind];
      this.values.get(kind).textContent = count.toLocaleString();
      this.values.get(kind).parentElement.dataset.empty = String(!count);
      this.values.get(kind).parentElement.style.setProperty("--share",total?`${count/total*100}%`:"0%");
      if (!count) continue;
      const next = offset + count / total * 100;
      stops.push("var(--" + color + ") " + offset + "% " + next + "%");
      offset = next;
    }
    this.wheel.style.setProperty("--activity-fill", stops.length ? "conic-gradient(" + stops.join(",") + ")" : "var(--raised)");
    this.root.dataset.empty = String(!total);
    this.material.update(activityKinds.map(([kind]) => counts[kind]));
  }
}
