use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, PasswordHash,
};

use itsi_error::ItsiError;
use magnus::{error::Result, Value};
use serde::Deserialize;
use serde_magnus::deserialize;
use sha_crypt::{
    sha256_check, sha256_simple, sha512_check, sha512_simple, Sha256Params, Sha512Params,
};

#[derive(Debug, Deserialize)]
pub enum HashAlgorithm {
    #[serde(rename(deserialize = "bcrypt"))]
    Bcrypt,
    #[serde(rename(deserialize = "sha256"))]
    Sha256Crypt,
    #[serde(rename(deserialize = "sha512"))]
    Sha512Crypt,
    #[serde(rename(deserialize = "argon2"))]
    Argon2,
    #[serde(rename(deserialize = "none"))]
    None,
}

pub fn create_password_hash(password: String, algo: Value) -> Result<String> {
    let hash_algorithm: HashAlgorithm = deserialize(algo)?;
    match hash_algorithm {
        HashAlgorithm::Bcrypt => {
            // Use the bcrypt crate for password hashing.
            bcrypt::hash(&password, bcrypt::DEFAULT_COST)
                .map_err(ItsiError::new)
                .map(Ok)?
        }
        HashAlgorithm::Sha256Crypt => {
            let params = Sha256Params::new(1000).unwrap();
            let hash = sha256_simple(&password, &params)
                .map_err(|_| ItsiError::new("SHA256 hashing failed"))?;
            Ok(hash)
        }
        HashAlgorithm::Sha512Crypt => {
            let params = Sha512Params::new(1000).unwrap();
            let hash = sha512_simple(&password, &params)
                .map_err(|_| ItsiError::new("SHA512 hashing failed"))?;
            Ok(hash)
        }
        HashAlgorithm::Argon2 => {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();
            let password_hash = argon2
                .hash_password(password.as_bytes(), &salt)
                .map_err(|_| ItsiError::new("Argon2 hashing failed"))?
                .to_string();
            Ok(password_hash)
        }
        HashAlgorithm::None => Ok(format!("$none${}", password)),
    }
}

pub fn verify_password_hash(password: &str, hash: &str) -> Result<bool> {
    if hash.starts_with("$2a$") || hash.starts_with("$2b$") || hash.starts_with("$2y$") {
        Ok(bcrypt::verify(password, hash).map_err(ItsiError::new)?)
    } else if hash.starts_with("$5$") {
        Ok(sha256_check(password, hash).is_ok())
    } else if hash.starts_with("$6$") {
        Ok(sha512_check(password, hash).is_ok())
    } else if hash.starts_with("$argon2") {
        let parsed_hash =
            PasswordHash::new(hash).map_err(|_| ItsiError::new("Argon2 hash parsing failed"))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    } else if hash
        .strip_prefix("$none$")
        .is_some_and(|stripped| stripped == password)
    {
        Ok(true)
    } else {
        Err(ItsiError::new("Unsupported hash algorithm").into())
    }
}
