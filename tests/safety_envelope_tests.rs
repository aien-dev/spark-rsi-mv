use aien_evaluation_protocol::Verdict;
use aien_protocol_types::{ArtifactRef, Digest32, Timestamp};
use spark_rsi::safety_envelope::CanarySafetyEnvelope;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn test_canary_safety_envelope_evaluation_and_rollback_enforcement() {
    let envelope = CanarySafetyEnvelope::new_reference_for_tests("rsi-daemon-01");

    let subject = ArtifactRef {
        artifact_id: uuid::Uuid::new_v4(),
        digest: Digest32([0x99; 32]),
        media_type: "application/rust".to_string(),
        byte_size: 4096,
    };

    let rollback_executed = Arc::new(AtomicBool::new(false));
    let rollback_flag = rollback_executed.clone();

    let receipt = envelope
        .evaluate_and_enforce(&subject, "rsi-production", Timestamp(1000), || async move {
            rollback_flag.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await
        .unwrap();

    // If the gate failed, rollback must be true and verdict Fail; if passed, verdict Pass and rollback false
    if receipt.receipt.verdict == Verdict::Pass {
        assert!(!rollback_executed.load(Ordering::SeqCst));
    } else {
        assert_eq!(receipt.receipt.verdict, Verdict::Fail);
        assert!(rollback_executed.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn test_wrong_key_receipt_never_promotes() {
    // Negative test: a valid looking receipt verified under the wrong key
    // must fail. Promotion is impossible without the production signature.
    let envelope = CanarySafetyEnvelope::new_reference_for_tests("rsi-daemon-01");

    let subject = ArtifactRef {
        artifact_id: uuid::Uuid::new_v4(),
        digest: Digest32([0x99; 32]),
        media_type: "application/rust".to_string(),
        byte_size: 4096,
    };

    let receipt = envelope
        .evaluate_and_enforce(&subject, "rsi-production", Timestamp(1000), || async {
            Ok(())
        })
        .await
        .unwrap();

    let wrong_signing = p256::ecdsa::SigningKey::from_slice(&[0x99; 32]).expect("valid p256 key");
    let wrong_verifier = p256::ecdsa::VerifyingKey::from(&wrong_signing);
    let verified = receipt.verify(&wrong_verifier);
    assert!(
        verified.is_err(),
        "receipt from the reference key must not verify under a wrong key"
    );
}

#[tokio::test]
async fn test_envelope_receipt_uses_framed_v3_digest_contract() {
    use aien_evaluation_protocol::{
        CryptographicTier, EvaluationError, VerifierSigner, FRAMED_RECEIPT_PROTOCOL_VERSION,
    };

    let envelope = CanarySafetyEnvelope::new_reference_for_tests("rsi-daemon-01");
    let subject = ArtifactRef {
        artifact_id: uuid::Uuid::new_v4(),
        digest: Digest32([0x99; 32]),
        media_type: "application/rust".to_string(),
        byte_size: 4096,
    };

    let plan = envelope.build_plan(&subject, "rsi-production", Vec::new());
    assert!(plan.verify_plan_digest());
    let mut shifted = plan.clone();
    shifted.profile.push('x');
    assert!(!shifted.verify_plan_digest());

    let receipt = envelope
        .evaluate_and_enforce(&subject, "rsi-production", Timestamp(1000), || async {
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(
        receipt.receipt.protocol_version,
        FRAMED_RECEIPT_PROTOCOL_VERSION
    );
    assert_eq!(receipt.receipt.protocol_version, 3);
    assert_eq!(receipt.receipt.subject_digest, subject.digest);

    let reference_signer = aien_evaluation_protocol::SoftwareP256Signer::new(
        p256::ecdsa::SigningKey::from_slice(&[0x42; 32]).expect("valid p256 key"),
    );
    let key = reference_signer.verifying_key();
    assert!(receipt
        .verify_with(&key, CryptographicTier::Tier2SoftwareKey)
        .unwrap());

    let mut downgraded = receipt.clone();
    downgraded.receipt.protocol_version = 1;
    assert!(matches!(
        downgraded.verify_with(&key, CryptographicTier::Tier2SoftwareKey),
        Err(EvaluationError::UnsupportedProtocolVersion(1))
    ));
}
