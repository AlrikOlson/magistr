use super::*;

#[test]
fn rejects_insecure_or_ambiguous_trust_anchors() {
    for endpoint in [
        "http://issuer.test",
        "//issuer.test",
        "issuer.test",
        "https://",
        "https://user:pass@issuer.test",
        "https://issuer.test/#fragment",
        "ftp://issuer.test",
        "https://issuer.test/\n",
    ] {
        assert!(secure_endpoint(endpoint, false).is_err(), "{endpoint:?}");
    }
    assert!(secure_endpoint("https://issuer.test/?tenant=x", true).is_err());
}

#[test]
fn https_paths_ports_and_endpoint_queries_are_supported() {
    for endpoint in [
        "https://issuer.test",
        "https://issuer.test/tenant/",
        "https://issuer.test:8443/token?tenant=x",
    ] {
        assert!(secure_endpoint(endpoint, false).is_ok(), "{endpoint}");
    }
}
