//! Session tokens in the system credential store (Windows Credential Manager,
//! macOS Keychain, Linux kernel keyring), never in Glint's own files.

use crate::ely::Session;
use crate::{Error, Result};

const SERVICE: &str = "net.glnt.launcher";

fn entry(uuid: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("ely:{uuid}")).map_err(|e| Error::Keychain(e.to_string()))
}

pub fn save_ely(session: &Session) -> Result<()> {
    let secret = serde_json::to_string(session).expect("session serializes");
    entry(&session.uuid)?
        .set_password(&secret)
        .map_err(|e| Error::Keychain(e.to_string()))
}

/// The stored session, or `None` when there is none (signed out or never signed in).
pub fn load_ely(uuid: &str) -> Result<Option<Session>> {
    match entry(uuid)?.get_password() {
        Ok(secret) => Ok(serde_json::from_str(&secret).ok()),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(Error::Keychain(e.to_string())),
    }
}

pub fn delete_ely(uuid: &str) -> Result<()> {
    match entry(uuid)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(Error::Keychain(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Touches the real credential store, so it only runs when asked:
    /// `cargo test -p glint-auth -- --ignored`.
    #[test]
    #[ignore = "uses the system credential store"]
    fn round_trip_in_the_system_store() {
        let session = Session {
            access_token: "test-access".into(),
            client_token: "test-client".into(),
            uuid: "glint-keychain-self-test".into(),
            name: "Test".into(),
        };
        save_ely(&session).unwrap();
        assert_eq!(load_ely(&session.uuid).unwrap(), Some(session.clone()));
        delete_ely(&session.uuid).unwrap();
        assert_eq!(load_ely(&session.uuid).unwrap(), None);
        delete_ely(&session.uuid).unwrap();
    }
}
