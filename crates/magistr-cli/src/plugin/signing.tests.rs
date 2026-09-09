use super::*;
use ed25519_dalek::{Signer, SigningKey};

fn generate_test_keypair() -> (SigningKey, VerifyingKey) {
    let mut rng = rand::rngs::OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

#[test]
fn verify_valid_signature() {
    let (signing_key, verifying_key) = generate_test_keypair();
    let data = b"plugin archive contents here";

    let signature = signing_key.sign(data);
    let sig_b64 = BASE64_STANDARD.encode(signature.to_bytes());
    let key_b64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    let result = verify_signature(data, &sig_b64, &key_b64);
    assert!(result.is_ok());
}

#[test]
fn reject_invalid_signature() {
    let (_signing_key, verifying_key) = generate_test_keypair();
    let data = b"plugin archive contents here";

    // Use a garbage signature
    let bad_sig = BASE64_STANDARD.encode([0u8; 64]);
    let key_b64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    let result = verify_signature(data, &bad_sig, &key_b64);
    assert!(result.is_err());
}

#[test]
fn reject_tampered_data() {
    let (signing_key, verifying_key) = generate_test_keypair();
    let data = b"original plugin data";
    let tampered = b"tampered plugin data";

    let signature = signing_key.sign(data);
    let sig_b64 = BASE64_STANDARD.encode(signature.to_bytes());
    let key_b64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    let result = verify_signature(tampered, &sig_b64, &key_b64);
    assert!(result.is_err());
}

#[test]
fn reject_wrong_key() {
    let (signing_key, _) = generate_test_keypair();
    let (_, wrong_verifying_key) = generate_test_keypair();
    let data = b"plugin archive contents";

    let signature = signing_key.sign(data);
    let sig_b64 = BASE64_STANDARD.encode(signature.to_bytes());
    let wrong_key_b64 = BASE64_STANDARD.encode(wrong_verifying_key.to_bytes());

    let result = verify_signature(data, &sig_b64, &wrong_key_b64);
    assert!(result.is_err());
}

#[test]
fn reject_malformed_signature_base64() {
    let (_, verifying_key) = generate_test_keypair();
    let data = b"some data";
    let key_b64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    let result = verify_signature(data, "not-valid-base64!!!", &key_b64);
    assert!(result.is_err());
}

#[test]
fn reject_malformed_public_key_base64() {
    let data = b"some data";
    let result = verify_signature(data, "AAAA", "not-valid-base64!!!");
    assert!(result.is_err());
}

#[test]
fn reject_wrong_length_signature() {
    let (_, verifying_key) = generate_test_keypair();
    let data = b"some data";
    let key_b64 = BASE64_STANDARD.encode(verifying_key.to_bytes());
    let short_sig = BASE64_STANDARD.encode([0u8; 32]); // 32 bytes, not 64

    let result = verify_signature(data, &short_sig, &key_b64);
    assert!(result.is_err());
}
