use abot_security::manifest::ManifestSigner;

#[test]
fn test_decode_signature_no_panic() {
    let signer = ManifestSigner::new();
    let mut manifest = signer.sign("test content").unwrap();

    // "ä" is 2 bytes in UTF-8, so "a".repeat(125) + "ä" + "a" is 125 + 2 + 1 = 128 bytes.
    // The slicing in manifest.rs goes in 2-byte chunks (0..2, 2..4, ..., 126..128).
    // The "ä" starts at byte 125 and ends at 127.
    // A slice from 126..128 will start inside the "ä" character, which would cause a panic
    // if the is_ascii() check is missing.
    let malicious_sig = "a".repeat(125) + "ä" + "a";
    manifest.signature = malicious_sig;

    let result = std::panic::catch_unwind(|| signer.verify(&manifest));

    // It should NOT panic, it should return an error gracefully
    assert!(result.is_ok(), "It should not panic");

    // The result should be an error
    let verification_result = result.unwrap();
    assert!(
        verification_result.is_err(),
        "Verification should fail because of invalid signature"
    );
}
