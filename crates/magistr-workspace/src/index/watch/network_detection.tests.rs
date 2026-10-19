use super::super::WatchMode;
use super::*;

#[test]
fn explicit_network_override_controls_journaling_and_auto_watching() {
    let forced = NetworkDetection::detect(Path::new("/local"), Some("1"));
    assert!(forced.network_fs);
    assert_eq!(forced.method, "env-override");
    assert_eq!(forced.journal_mode(), "TRUNCATE");
    assert_eq!(
        WatchMode::Auto.effective(forced.network_fs),
        WatchMode::Poll
    );
    let local = NetworkDetection::detect(Path::new("//server/share"), Some("0"));
    assert!(!local.network_fs);
    assert_eq!(local.journal_mode(), "WAL");
    assert_eq!(WatchMode::Auto.effective(false), WatchMode::Native);
    assert_eq!(WatchMode::Native.effective(true), WatchMode::Native);
    assert_eq!(WatchMode::Off.effective(true), WatchMode::Off);
}

#[test]
fn unc_paths_and_invalid_override_are_detected() {
    for path in ["//server/share", "\\\\server\\share"] {
        let detected = NetworkDetection::detect(Path::new(path), Some("yes"));
        assert!(detected.network_fs);
        assert_eq!(detected.method, "unc-path");
    }
}

#[test]
fn longest_component_mount_wins_and_proc_escapes_are_decoded() {
    let mounts = "disk / ext4 rw 0 0\nserver /work nfs4 rw 0 0\nlocal /work/local ext4 rw 0 0\ncloud /with\\040space fuse.s3fs rw 0 0\n";
    let network = NetworkDetection::from_mounts(Path::new("/work/project"), mounts).unwrap();
    assert!(network.network_fs);
    assert_eq!(network.fstype.as_deref(), Some("nfs4"));
    assert!(
        !NetworkDetection::from_mounts(Path::new("/work/local/app"), mounts)
            .unwrap()
            .network_fs
    );
    assert!(
        !NetworkDetection::from_mounts(Path::new("/workspace/app"), mounts)
            .unwrap()
            .network_fs
    );
    assert!(
        NetworkDetection::from_mounts(Path::new("/with space/app"), mounts)
            .unwrap()
            .network_fs
    );
}
