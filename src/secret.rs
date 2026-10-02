//! Refuses input that looks like a private key or recovery phrase, before it
//! leaves the machine. Lookups only ever need public data (block heights and
//! hashes, transaction ids, addresses), so a secret is never sent anywhere.
//!
//! A bare 64-character hex string is not caught here: it is also what a
//! transaction id or block hash looks like. Dogechain.com rejects key-shaped
//! input of that kind itself.

use crate::error::{CliError, Result};

pub const WARNING: &str = "that looks like a private key or recovery phrase, so it wasn't \
                           looked up; never send one to any website or API";

const BASE58: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Fails with a bad-input error (exit 2) if `input` looks like a secret.
pub fn refuse_secrets(input: &str) -> Result<()> {
    if looks_secret(input) {
        Err(CliError::BadInput(WARNING.into()))
    } else {
        Ok(())
    }
}

pub fn looks_secret(input: &str) -> bool {
    let s = input.trim();
    is_wif(s) || is_extended_private_key(s) || is_recovery_phrase(s)
}

fn is_base58(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| BASE58.contains(c))
}

/// Wallet import format: 51 (uncompressed) or 52 (compressed) base58
/// characters. Addresses are 33–34, so the lengths never overlap.
/// Prefixes: Dogecoin 6/Q, Bitcoin 5/K/L, testnets 9/c.
fn is_wif(s: &str) -> bool {
    matches!(s.len(), 51 | 52) && is_base58(s) && s.starts_with(['6', 'Q', '5', 'K', 'L', '9', 'c'])
}

/// BIP32 extended private keys (Dogecoin dgpv, Bitcoin xprv/yprv/zprv and
/// testnet tprv/uprv/vprv).
fn is_extended_private_key(s: &str) -> bool {
    const PREFIXES: [&str; 7] = ["dgpv", "xprv", "yprv", "zprv", "tprv", "uprv", "vprv"];
    s.len() >= 100 && is_base58(s) && PREFIXES.iter().any(|p| s.starts_with(p))
}

/// 12 to 24 short lowercase words, the shape of a BIP39 recovery phrase.
fn is_recovery_phrase(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    (12..=24).contains(&words.len())
        && words
            .iter()
            .all(|w| (3..=8).contains(&w.len()) && w.bytes().all(|b| b.is_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_keys_and_phrases() {
        // Key-shaped sample strings (standard test vectors and made-up
        // examples); none controls any funds.
        assert!(looks_secret(
            "5HueCGU8rMjxEXxiPuD5BDku4MkFqeZyd4dZ1jvhTVqvbTLvyTJ"
        ));
        assert!(looks_secret(
            "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn"
        ));
        assert!(looks_secret(
            "6KbBFk8U7sxzfajBnm1JJWqLvdcpd97K9Sf5GPgP2z8A1nR4GZ7"
        ));
        assert!(looks_secret(
            "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi"
        ));
        assert!(looks_secret(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        ));
    }

    #[test]
    fn lets_public_data_through() {
        assert!(!looks_secret("DH5yaieqoZN36fDVciNyRueRGvGLR3mr7L"));
        assert!(!looks_secret("A8x177ySgB3BMr3LXYHWp7dLKNAjxo4hGJ"));
        assert!(!looks_secret(
            "2c916c8046224e19f865f288ebc9870fc75f93b5144595b7ec270b31e4863e7f"
        ));
        assert!(!looks_secret("6389673"));
        assert!(!looks_secret("latest"));
        assert!(!looks_secret("what is the dogecoin supply"));
    }
}
