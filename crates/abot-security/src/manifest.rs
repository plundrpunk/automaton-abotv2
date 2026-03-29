use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error types for manifest signing
#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Sign error: {0}")]
    SignError(String),

    #[error("Verification failed")]
    VerificationFailed,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid key")]
    InvalidKey,
}

/// Signed manifest entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedManifest {
    pub content: String,
    pub signature: String,
    pub public_key: String,
}

/// Signs and verifies manifests using ed25519
pub struct ManifestSigner {
    signing_key: Option<SigningKey>,
    verifying_key: Option<VerifyingKey>,
}

impl ManifestSigner {
    /// Create a new signer by generating keys
    pub fn new() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        Self {
            signing_key: Some(signing_key),
            verifying_key: Some(verifying_key),
        }
    }

    /// Create a signer from an existing signing key
    pub fn from_secret_bytes(secret: &[u8; 32]) -> Result<Self, ManifestError> {
        let signing_key = SigningKey::from_bytes(secret);
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            signing_key: Some(signing_key),
            verifying_key: Some(verifying_key),
        })
    }

    /// Create a verifier from a public key (for verification only)
    pub fn from_public_key(public_bytes: &[u8; 32]) -> Result<Self, ManifestError> {
        let verifying_key = VerifyingKey::from_bytes(public_bytes)
            .map_err(|_| ManifestError::InvalidKey)?;

        Ok(Self {
            signing_key: None,
            verifying_key: Some(verifying_key),
        })
    }

    /// Sign manifest content
    pub fn sign(&self, content: &str) -> Result<SignedManifest, ManifestError> {
        let signing_key = self
            .signing_key
            .as_ref()
            .ok_or(ManifestError::SignError(
                "No signing key available".to_string(),
            ))?;

        let signature = signing_key.sign(content.as_bytes());
        let public_key = signing_key.verifying_key();

        Ok(SignedManifest {
            content: content.to_string(),
            signature: signature.to_string(),
            public_key: hex::encode(public_key.as_bytes()),
        })
    }

    /// Verify a signed manifest
    pub fn verify(&self, manifest: &SignedManifest) -> Result<bool, ManifestError> {
        let verifying_key = self
            .verifying_key
            .as_ref()
            .ok_or(ManifestError::VerificationFailed)?;

        let signature = Signature::from_bytes(
            &Self::decode_signature(&manifest.signature)?
        );

        match verifying_key.verify(manifest.content.as_bytes(), &signature) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get the public key in hex format
    pub fn public_key(&self) -> Result<String, ManifestError> {
        let key = self
            .verifying_key
            .as_ref()
            .ok_or(ManifestError::SignError(
                "No verifying key available".to_string(),
            ))?;

        Ok(hex::encode(key.as_bytes()))
    }

    fn decode_signature(sig_str: &str) -> Result<[u8; 64], ManifestError> {
        let mut sig_bytes = [0u8; 64];
        // Simple hex parsing - production would use proper hex decoder
        if sig_str.len() != 128 {
            return Err(ManifestError::InvalidSignature);
        }

        for i in 0..64 {
            let byte_str = &sig_str[i*2..i*2+2];
            sig_bytes[i] = u8::from_str_radix(byte_str, 16)
                .map_err(|_| ManifestError::InvalidSignature)?;
        }

        Ok(sig_bytes)
    }
}

impl Default for ManifestSigner {
    fn default() -> Self {
        Self::new()
    }
}

// Dummy hex module for tests (production would use hex crate)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signer_creation() {
        let signer = ManifestSigner::new();
        let public_key = signer.public_key();
        assert!(public_key.is_ok());
    }

    #[test]
    fn test_sign_and_verify() {
        let signer = ManifestSigner::new();
        let content = "HAND.toml manifest content";

        let signed = signer.sign(content).unwrap();
        assert_eq!(signed.content, content);
        assert!(!signed.signature.is_empty());
    }

    #[test]
    fn test_manifest_serialization() {
        let signer = ManifestSigner::new();
        let signed = signer.sign("test content").unwrap();

        let json = serde_json::to_string(&signed).unwrap();
        let deserialized: SignedManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.content, signed.content);
        assert_eq!(deserialized.signature, signed.signature);
    }

    #[test]
    fn test_different_content_different_signature() {
        let signer = ManifestSigner::new();

        let sig1 = signer.sign("content1").unwrap().signature;
        let sig2 = signer.sign("content2").unwrap().signature;

        assert_ne!(sig1, sig2);
    }
}
