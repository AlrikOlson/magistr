use super::*;

#[test]
fn only_explicit_web_destinations_are_openable() {
    for allowed in [
        "https://example.com/日本?q=a#part",
        "http://localhost:8080/",
    ] {
        assert!(web_address(allowed).is_some(), "{allowed}");
    }
    for denied in [
        "javascript:alert(1)",
        "data:text/html,<script>x</script>",
        "file:///etc/passwd",
        "mailto:a@example.com",
        "/tmp/a",
        "//example.com",
        "https://a:b@example.com/",
        " https://example.com",
        "https://example.com\n",
        "https://example.com\u{0000}",
    ] {
        assert!(web_address(denied).is_none(), "{denied:?}");
    }
}

#[test]
fn authored_navigation_accepts_only_the_configured_platform_bundle() {
    let custom = tauri::Url::parse("tauri://localhost").unwrap();
    assert_eq!(
        custom.path(),
        "",
        "Tauri simplifies its bundled index.html URL"
    );
    let web_protocol = cfg!(any(windows, target_os = "android"));
    for use_https in [false, true] {
        for (origin, expected) in [
            ("tauri://localhost", !web_protocol),
            ("http://tauri.localhost", web_protocol && !use_https),
            ("https://tauri.localhost", web_protocol && use_https),
        ] {
            for path in ["", "/", "/index.html"] {
                let url = tauri::Url::parse(&format!("{origin}{path}")).unwrap();
                assert_eq!(local_document(&url, use_https), expected, "{url}");
            }
        }
        for denied in [
            "https://example.com/",
            "http://tauri.localhost.evil/",
            "file:///tmp/a.html",
            "tauri://localhost/other",
            "tauri://user:password@localhost",
            "tauri://localhost:8080",
            "tauri://localhost/?remote=true",
            "http://tauri.localhost/other",
            "http://user:password@tauri.localhost/",
            "http://tauri.localhost:8080/",
            "https://tauri.localhost/?remote=true",
        ] {
            assert!(
                !local_document(&tauri::Url::parse(denied).unwrap(), use_https),
                "{denied}"
            );
        }
    }
}
