/// Lifetime of the native cache lock, independent of duplicated OS descriptors.
/// Keep this field after the SQLite connection so checkpoint/close finishes first.
pub(super) struct CacheOwner(pub(super) std::fs::File);

impl Drop for CacheOwner {
    fn drop(&mut self) {
        // A concurrent process spawn can inherit this descriptor until exec.
        // Closing our descriptor alone leaves flock held by that inherited
        // reference. Explicit unlock ends ownership when this cache closes.
        // Drop cannot surface unlock errors; the owned descriptor still closes.
        let _ = crate::os_file_lock::unlock(&self.0);
    }
}
