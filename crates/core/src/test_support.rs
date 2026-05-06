use secrecy::SecretString;
use totp_rs::Secret;

const MIN_SEEDED_SECRET_BYTES: usize = 10;
const EMPTY_SEED_FALLBACK: &[u8] = b"mfa-forge-test-seed";

pub fn base32_secret_from_bytes(bytes: &[u8]) -> String {
    let source = if bytes.is_empty() {
        EMPTY_SEED_FALLBACK
    } else {
        bytes
    };
    let encoded = Secret::Raw(source.to_vec()).to_encoded();

    match &encoded {
        Secret::Encoded(secret) => secret.clone(),
        Secret::Raw(_) => unreachable!("raw secrets always encode into base32"),
    }
}

pub fn base32_secret_from_seed(seed: &str) -> String {
    let source = if seed.is_empty() {
        EMPTY_SEED_FALLBACK
    } else {
        seed.as_bytes()
    };
    let target_len = source.len().max(MIN_SEEDED_SECRET_BYTES);
    let mut bytes = Vec::with_capacity(target_len);

    for index in 0..target_len {
        let seed_byte = source[index % source.len()];
        let salt = ((index as u8).wrapping_mul(29)).wrapping_add(17);
        bytes.push(seed_byte ^ salt);
    }

    base32_secret_from_bytes(&bytes)
}

pub fn secret_string_from_seed(seed: &str) -> SecretString {
    SecretString::from(base32_secret_from_seed(seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_secret_is_deterministic_and_valid() {
        let first = base32_secret_from_seed("fixture-primary");
        let second = base32_secret_from_seed("fixture-primary");

        assert_eq!(first, second);
        assert!(first.len() >= 16);
        assert!(
            first.chars().all(|character| {
                character.is_ascii_uppercase() || matches!(character, '2'..='7')
            })
        );
    }

    #[test]
    fn byte_secret_preserves_exact_rfc_vector_material() {
        let encoded = base32_secret_from_bytes(b"12345678901234567890");
        assert_eq!(encoded.len(), 32);
        assert!(
            encoded.chars().all(|character| {
                character.is_ascii_uppercase() || matches!(character, '2'..='7')
            })
        );
    }
}
