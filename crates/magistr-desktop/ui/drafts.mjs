// Serialize native editor saves; only the newest unsaved edit waits behind a write.
export class Drafts {
  constructor(save, changed = () => {}) { this.save = save; this.changed = changed; this.entries = new Map(); }
  key(address) { return JSON.stringify(address); }
  entry(address) {
    const key = this.key(address);
    if (!this.entries.has(key)) this.entries.set(key, {address, revision: 0, saved_revision: 0, save_attempt: 0, error: null, desired: null, running: null});
    return this.entries.get(key);
  }
  merge(entry, status) {
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
    this.merge(this.entry(frame.address), {
      revision: frame.draft_revision, saved_revision: frame.draft_saved_revision,
      save_attempt: frame.draft_save_attempt, error: frame.draft_save_error,
    });
    this.changed(frame.address);
  }
  edit(address, text) {
    const entry = this.entry(address);
    entry.desired = {text};
    entry.error = null;
    this.start(entry);
  }
  start(entry) {
    if (entry.running) return;
    entry.running = this.drain(entry).finally(() => { entry.running = null; this.changed(entry.address); });
    this.changed(entry.address);
  }
  async drain(entry) {
    while (entry.desired) {
      const edit = entry.desired; entry.desired = null;
      try {
        const status = await this.save(entry.address, entry.revision, edit.text);
        if (this.merge(entry, status) && status.error) throw new Error(status.error);
      } catch (error) {
        if (!entry.desired) entry.desired = edit;
        entry.error = error?.message ?? String(error);
        this.changed(entry.address);
        return;
      }
      this.changed(entry.address);
    }
  }
  async flush(address) {
    const entry = this.entry(address);
    if (entry.desired && !entry.running) this.start(entry);
    if (entry.running) await entry.running;
    if (entry.error || entry.desired) throw new Error(entry.error || "Your draft has not been saved.");
  }
  forget(address) { this.entries.delete(this.key(address)); }
  async flushAll() {
    await Promise.all([...this.entries.values()].map(entry => this.flush(entry.address)));
  }
  status(address) {
    const entry = this.entry(address);
    return {revision: entry.revision, saved_revision: entry.saved_revision, save_attempt: entry.save_attempt, error: entry.error,
      saving: !!entry.running || !!entry.desired || entry.saved_revision < entry.revision};
  }
  accept(address, status) {
    const entry = this.entry(address);
    this.merge(entry, status);
    this.changed(address);
  }
}
