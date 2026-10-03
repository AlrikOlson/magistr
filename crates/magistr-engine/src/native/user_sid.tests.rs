use super::*;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;

fn unique() -> std::path::PathBuf {
    let tmp = tempfile::tempdir().expect("tempdir");
    super::super::owner_pipe::pipe_name("sid", tmp.path().as_os_str().as_encoded_bytes())
        .expect("name")
}

#[test]
fn the_current_user_is_a_string_sid() {
    let user = current_user().expect("user");
    assert!(user.starts_with("S-1-"), "{user}");
    assert_eq!(user, current_user().expect("user"), "stable");
}

#[test]
fn a_descriptor_for_a_malformed_sid_is_refused() {
    assert!(OwnerOnlyDescriptor::for_user("not-a-sid").is_err());
}

#[tokio::test]
async fn both_ends_of_an_owner_only_pipe_see_this_user() {
    let name = unique();
    let user = current_user().expect("user");
    let descriptor = OwnerOnlyDescriptor::for_user(&user).expect("descriptor");
    let mut server = descriptor.create(&name, true).expect("server");
    let mut client = ClientOptions::new().open(&name).expect("client");
    server.connect().await.expect("connected");
    assert_eq!(server_user(&client).expect("server user"), user);
    client.write_all(b"x").await.expect("write");
    let mut byte = [0_u8; 1];
    server.read_exact(&mut byte).await.expect("read");
    assert_eq!(client_user(&server).expect("client user"), user);
}

#[tokio::test]
async fn a_first_instance_on_a_taken_name_is_permission_denied() {
    let name = unique();
    let user = current_user().expect("user");
    let descriptor = OwnerOnlyDescriptor::for_user(&user).expect("descriptor");
    let _first = descriptor.create(&name, true).expect("first");
    let error = descriptor.create(&name, true).expect_err("taken");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "{error}");
}
