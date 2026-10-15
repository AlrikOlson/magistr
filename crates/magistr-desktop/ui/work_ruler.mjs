// Position within an owner's bounded run, deliberately not a completion estimate.
export function visibleAttempts(current, limit) {
  if (!Number.isSafeInteger(limit) || limit < 1 || !Number.isSafeInteger(current) || current < 0 || current > limit) return [];
  if (limit <= 20) return Array.from({length:limit}, (_, index) => index + 1);
  return [...new Set([1, current - 1, current, current + 1, limit])].filter(n => n >= 1 && n <= limit).sort((a,b) => a-b);
}
export class AttemptRuler {
  constructor(root) { this.root = root; this.key = ""; }
  render(attempt, phase) {
    const marks = attempt ? visibleAttempts(attempt.current, attempt.limit) : [];
    const key = JSON.stringify([marks, attempt?.current, phase]);
    if (key === this.key) return;
    this.key = key; this.root.hidden = !marks.length; this.root.dataset.phase = phase ?? "idle";this.root.dataset.circular=String(!!marks.length&&attempt.limit<=20);
    const document = this.root.ownerDocument, nodes = [];
    let previous = 0;
    for (const number of marks) {
      if (number > previous + 1) {
        const gap = document.createElement("span"); gap.className = "attempt-gap"; gap.textContent = "…"; nodes.push(gap);
      }
      const mark = document.createElement("span");
      mark.className = "attempt-mark"; mark.textContent = String(number);
      mark.dataset.current = String(number === attempt.current);
      mark.style.setProperty("--attempt-angle",((number-1)/attempt.limit*360)+"deg");
      nodes.push(mark); previous = number;
    }
    this.root.style.setProperty("--attempt-step",marks.length?(360/attempt.limit)+"deg":"360deg");
    this.root.style.setProperty("--current-angle",marks.length&&attempt.current>0?((attempt.current-1)/attempt.limit*360)+"deg":"0deg");
    this.root.dataset.current=String(!!marks.length&&attempt.current>0);
    this.root.replaceChildren(...nodes);
  }
}
