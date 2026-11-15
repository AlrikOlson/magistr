use super::*;
use crate::config::ResolveOptions;

#[test]
fn a_source_file_deeper_than_the_windows_path_limit_is_written_and_replaced() {
    let root = tempfile::tempdir().unwrap();
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(root.path())).unwrap();
    let files = NativeFiles {
        root: policy
            .check_write("anchor")
            .unwrap()
            .parent()
            .unwrap()
            .into(),
    };
    let mut relative = String::new();
    while root.path().as_os_str().len() + relative.len() < 300 {
        relative.push_str(&"d".repeat(60));
        relative.push('/');
    }
    relative.push_str("source.rs");

    for text in ["fn first() {}\n", "fn second() {}\n"] {
        let admitted = files.resolve(&policy, &relative).unwrap();
        let mut created = Vec::new();
        files
            .replace(
                &policy,
                &relative,
                &admitted,
                Some(&FileContent::decode(text.as_bytes().to_vec())),
                None,
                &mut created,
            )
            .unwrap();
        assert!(admitted.as_os_str().len() > 260, "{}", admitted.display());
        assert_eq!(fs::read_to_string(&admitted).unwrap(), text);
        let siblings = fs::read_dir(admitted.parent().unwrap()).unwrap().count();
        assert_eq!(siblings, 1, "no temporary is left beside the source file");
    }
}
