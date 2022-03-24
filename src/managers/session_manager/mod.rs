use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};

type HashRes<T> = Result<T,argon2::password_hash::Error>;
pub struct SessionManager {

}

pub fn hash_bytes(bytes: &[u8]) -> HashRes<String> {

    let salt = SaltString::generate(&mut OsRng);
    
    let argon2 = Argon2::default();

    let password_hash = argon2.hash_password(bytes, &salt)?.to_string();
    Ok(password_hash)
}

pub fn hash_match(password:&[u8], password_hash: &str) -> HashRes<()> {
    let password_hash = PasswordHash::new(&password_hash)?;
    Argon2::default().verify_password(password, &password_hash)
}