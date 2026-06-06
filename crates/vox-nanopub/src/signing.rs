use crate::trig::NanopubDocument;
pub use vox_crypto::facades::SigningKey;
pub use vox_crypto::facades::VerifyingKey;
use vox_crypto::facades::{sign, verify};

pub struct SignedNanopub {
    pub document: NanopubDocument,
    pub signature_hex: String,
}

pub fn sign_nanopub(doc: NanopubDocument, signing_key: &SigningKey) -> SignedNanopub {
    let sig_bytes: [u8; 64] = sign(signing_key, doc.trig.as_bytes());
    let signature_hex = hex::encode(sig_bytes);
    SignedNanopub {
        document: doc,
        signature_hex,
    }
}

pub fn verify_nanopub(signed: &SignedNanopub, verifying_key: &VerifyingKey) -> bool {
    let sig_bytes = match hex::decode(&signed.signature_hex) {
        Ok(b) if b.len() == 64 => {
            let mut arr = [0u8; 64];
            arr.copy_from_slice(&b);
            arr
        }
        _ => return false,
    };
    verify(verifying_key, signed.document.trig.as_bytes(), &sig_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trig::build_nanopub;
    use vox_crypto::facades::generate_signing_keypair;

    #[test]
    fn sign_and_verify_round_trip() {
        let (sk, vk) = generate_signing_keypair();
        let doc = build_nanopub("round trip claim", "provider:test", 999);
        let signed = sign_nanopub(doc, &sk);
        assert!(verify_nanopub(&signed, &vk));
    }

    #[test]
    fn tampered_trig_fails_verify() {
        let (sk, vk) = generate_signing_keypair();
        let doc = build_nanopub("original claim", "provider:test", 999);
        let mut signed = sign_nanopub(doc, &sk);
        signed.document.trig.push_str("\n# tampered");
        assert!(!verify_nanopub(&signed, &vk));
    }
}
