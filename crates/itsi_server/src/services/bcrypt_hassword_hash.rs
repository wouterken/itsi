use itsi_error::ItsiError;
use magnus::error::Result;

pub fn create_password_hash(password: String) -> Result<String> {
    Ok(bcrypt::hash(&password, bcrypt::DEFAULT_COST).map_err(ItsiError::new)?)
}

pub fn verify_password_hash(password: String, hash: String) -> Result<bool> {
    Ok(bcrypt::verify(password, &hash).map_err(ItsiError::new)?)
}
