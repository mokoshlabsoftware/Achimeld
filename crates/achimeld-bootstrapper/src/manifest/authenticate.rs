//! Signature verification binds the exact received bytes, never a reserialized object.
use super::{
    MAX_MANIFEST,
    model::{Manifest, TrustConfig},
    policy,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rsa::{
    RsaPublicKey,
    pkcs8::DecodePublicKey,
    pss::{Signature, VerifyingKey},
    signature::Verifier,
};
use sha2::Sha256;
pub fn parse_and_authenticate(
    bytes: &[u8],
    signature: &[u8],
    trust: &TrustConfig,
) -> Result<Manifest, String> {
    if trust.mode == "official" {
        return Err("official releases are disabled pending publisher and release gates".into());
    }
    if bytes.len() > MAX_MANIFEST || trust.mode == "unconfigured" {
        return Err("release trust is not configured".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| "invalid manifest schema")?;
    let key = trust
        .keys
        .iter()
        .find(|key| key.key_id == manifest.key_id && !key.revoked)
        .ok_or("unknown or revoked release key")?;
    let public = STANDARD
        .decode(&key.public_key)
        .map_err(|_| "invalid trusted key encoding")?;
    let public =
        RsaPublicKey::from_public_key_der(&public).map_err(|_| "invalid trusted RSA key")?;
    use rsa::traits::PublicKeyParts;
    if public.n().bits() < 3072 {
        return Err("release key is too small".into());
    }
    VerifyingKey::<Sha256>::new(public)
        .verify(
            bytes,
            &Signature::try_from(signature).map_err(|_| "invalid signature encoding")?,
        )
        .map_err(|_| "manifest signature rejected")?;
    policy::validate(&manifest, trust)?;
    Ok(manifest)
}
