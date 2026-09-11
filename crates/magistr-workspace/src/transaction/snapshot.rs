use crate::file_content::FileContent;
use std::fs::Permissions;

pub(super) struct Snapshot {
    pub content: Option<FileContent>,
    pub permissions: Option<Permissions>,
}
impl Snapshot {
    pub fn same(&self, other: &Self) -> bool {
        self.content == other.content && self.permissions == other.permissions
    }
}
