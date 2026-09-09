use super::*;

/// Every ground names its folder, and the folder's last segment is the
/// project's name until the survey says otherwise.
#[test]
fn every_ground_names_its_folder_and_the_project() {
    let folder = Ground::Folder("/Users/me/Code/tidewatch".to_owned());
    assert_eq!(folder.dir(), "/Users/me/Code/tidewatch");
    assert_eq!(folder.name(), "tidewatch");
    let empty = Ground::Empty("~/Code/harbor/".to_owned());
    assert_eq!(empty.name(), "harbor");
    let clone = Ground::Clone {
        url: "https://github.com/x/tidewatch.git".to_owned(),
        into: "~/Code/tidewatch".to_owned(),
    };
    assert_eq!(clone.dir(), "~/Code/tidewatch");
    assert_eq!(clone.name(), "tidewatch");
}

/// A clone lands beside the window's project, named after the repo.
#[test]
fn a_clone_lands_beside_the_project_named_after_the_repo() {
    assert_eq!(
        Ground::clone_into("https://github.com/x/tidewatch.git", "/Users/me/Code"),
        "/Users/me/Code/tidewatch"
    );
    assert_eq!(Ground::clone_into("git@github.com:x/harbor", ""), "harbor");
}
