use super::*;
use crate::cockpit::Size;

#[test]
fn a_seeded_model_is_a_hosted_session_on_the_project() {
    let seed = ProjectSeed {
        name: "sample".into(),
        dir: "/projects/sample".into(),
        title: "sample · acp".into(),
        debug: true,
        workspace_files: vec!["src/lib.rs".into()],
        ..ProjectSeed::default()
    };
    let model = seed.seed(Model::new(Size::new(80, 24)));
    assert!(
        model.hosts_work(),
        "its host starts work and opens projects in place"
    );
    assert_eq!(model.title(), Some("sample · acp"));
    assert!(model.is_debug());
    let here = &model.galaxy().projects()[0];
    assert!(here.here);
    assert_eq!(
        (here.name.as_str(), here.dir.as_deref()),
        ("sample", Some("/projects/sample"))
    );
    assert_eq!(model.session.workspace_files, ["src/lib.rs"]);
}
