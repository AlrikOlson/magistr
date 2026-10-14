import { addressKey, targetKey } from "./context.mjs";
// Serialize native editor saves; only the newest unsaved edit waits behind a write.
export class Drafts {
  constructor(save, changed = () => {}) { this.save = save; this.changed = changed; this.entries = new Map(); }
  key(target) { return targetKey(target); }
  entry(target) {
    const key = this.key(target);
    if (!this.entries.has(key)) this.entries.set(key, {target, revision: 0, saved_revision: 0, save_attempt: 0, error: null, desired: null, active: null, acknowledged: null, observed_revision: 0, running: null});
    return this.entries.get(key);
  }
  merge(entry, status) {
    if (status.conversation !== entry.target.conversation) return false;
    if (status.revision < entry.revision) return false;
    entry.revision = status.revision;
    // Frames and RPC replies have independent delivery order. A newer edit can
    // still carry an older completed save; keep its revision without that status.
    if (status.save_attempt < entry.save_attempt || status.saved_revision < entry.saved_revision) return false;
    entry.save_attempt = status.save_attempt;
    entry.saved_revision = status.saved_revision;
    entry.error = status.error ?? (entry.desired ? entry.error : null);
    return true;
  }
  observe(frame) {
    if (!frame.target) return;
    const entry = this.entry(frame.target);
    entry.observed_revision = Math.max(entry.observed_revision, frame.draft_revision);
    if (entry.acknowledged && frame.draft_revision >= entry.acknowledged.revision) entry.acknowledged = null;
    this.merge(entry, {
      conversation: frame.target.conversation, revision: frame.draft_revision, saved_revision: frame.draft_saved_revision,
      save_attempt: frame.draft_save_attempt, error: frame.draft_save_error,
    });
    this.changed(frame.target);
  }
  edit(target, text) {
    const entry = this.entry(target);
    entry.desired = {text};
    entry.error = null;
    this.start(entry);
  }
  start(entry) {
    if (entry.running) return;
    entry.running = this.drain(entry).finally(() => { entry.running = null; this.changed(entry.target); });
    this.changed(entry.target);
  }
  async drain(entry) {
    while (entry.desired) {
      const edit = entry.desired; entry.desired = null;
      entry.active = {...edit, revision: entry.revision};
      try {
        const status = await this.save(entry.target, entry.active.revision, edit.text);
        if (status.conversation !== entry.target.conversation) throw new Error("The draft reply belongs to a different conversation. Try again.");
        if (this.accept(entry.target, status, edit.text) && status.error) throw new Error(status.error);
      } catch (error) {
        if (!entry.desired) entry.desired = edit;
        entry.error = error?.message ?? String(error);
        this.changed(entry.target);
        return;
      } finally { entry.active = null; }
      this.changed(entry.target);
    }
  }
  async flush(target) {
    const entry = this.entry(target);
    if (entry.desired && !entry.running) this.start(entry);
    if (entry.running) await entry.running;
    if (entry.error || entry.desired) throw new Error(entry.error || "Your draft has not been saved.");
  }
  restore(frame) {
    if (!frame.target) return frame.draft;
    const entry = this.entry(frame.target);
    if (entry.desired) return entry.desired.text;
    if (entry.active && frame.draft_revision <= entry.active.revision) return entry.active.text;
    if (entry.acknowledged && frame.draft_revision < entry.acknowledged.revision) return entry.acknowledged.text;
    return frame.draft;
  }
  async flushSession(address) {
    await Promise.all([...this.entries.values()]
      .filter(entry => addressKey(entry.target.address) === addressKey(address))
      .map(entry => this.flush(entry.target)));
  }
  forgetSession(address) {
    for (const [key, entry] of this.entries)
      if (addressKey(entry.target.address) === addressKey(address)) this.entries.delete(key);
  }
  async flushAll() {
    await Promise.all([...this.entries.values()].map(entry => this.flush(entry.target)));
  }
  status(target) {
    const entry = this.entry(target);
    return {conversation: target.conversation, revision: entry.revision, saved_revision: entry.saved_revision, save_attempt: entry.save_attempt, error: entry.error,
      saving: !!entry.running || !!entry.desired || entry.saved_revision < entry.revision};
  }
  accept(target, status, text) {
    const entry = this.entry(target);
    const accepted = this.merge(entry, status);
    if (accepted && !status.error && text !== undefined && status.revision > entry.observed_revision)
      entry.acknowledged = {text, revision: status.revision};
    this.changed(target);
    return accepted;
  }
}
