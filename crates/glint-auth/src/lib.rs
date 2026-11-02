//! Accounts for Glint: offline names and Ely.by accounts now, Microsoft next.
//!
//! Neither offline names nor Ely.by prove that the player owns Minecraft, so both
//! become available once a Microsoft account that owns the game has signed in at
//! least once (the same rule Prism Launcher uses for offline names). Builds made
//! for development lift that requirement with [`OfflinePolicy::Unrestricted`].

pub mod ely;
pub mod keychain;

use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0:?} is not a valid Minecraft name: use 3–16 letters, digits or _")]
    InvalidName(String),

    #[error("sign in once with a Microsoft account that owns Minecraft to use offline names and Ely.by accounts")]
    OwnershipRequired,

    #[error("Ely.by: {0}")]
    Rejected(String),

    #[error("this Ely.by account uses two-factor authentication; enter the code from your app")]
    TwoFactorRequired,

    #[error("the Ely.by session has expired; sign in again in Settings")]
    SessionExpired,

    #[error("could not reach the sign-in server: {0}")]
    Network(String),

    #[error("could not use the system credential store: {0}")]
    Keychain(String),

    #[error("an account named {0:?} already exists")]
    Duplicate(String),

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid account file {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// Whether offline names and Ely.by accounts need a verified game owner first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflinePolicy {
    /// Release builds.
    RequireOwnership,
    /// Development builds, so the launcher can be tested before Microsoft sign-in works.
    Unrestricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Account {
    Offline { name: String, uuid: String },
    /// Tokens live in the system credential store, see [`keychain`].
    Ely { name: String, uuid: String },
}

impl Account {
    pub fn name(&self) -> &str {
        match self {
            Self::Offline { name, .. } | Self::Ely { name, .. } => name,
        }
    }

    pub fn uuid(&self) -> &str {
        match self {
            Self::Offline { uuid, .. } | Self::Ely { uuid, .. } => uuid,
        }
    }

    pub fn needs_session(&self) -> bool {
        matches!(self, Self::Ely { .. })
    }

    /// What the game needs to know about the player at launch. `access_token` is
    /// the account's current session token; offline accounts ignore it.
    pub fn identity(&self, access_token: Option<&str>) -> LaunchIdentity {
        match self {
            Self::Offline { name, uuid } => LaunchIdentity {
                name: name.clone(),
                uuid: uuid.clone(),
                access_token: "0".into(),
                user_type: "legacy".into(),
                xuid: "0".into(),
                auth_server: None,
            },
            Self::Ely { name, uuid } => LaunchIdentity {
                name: name.clone(),
                uuid: uuid.clone(),
                access_token: access_token.unwrap_or("0").to_owned(),
                user_type: "mojang".into(),
                xuid: "0".into(),
                auth_server: Some(ely::AUTHLIB_INJECTOR_TARGET.into()),
            },
        }
    }
}

/// Values substituted into `${auth_*}` launch arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchIdentity {
    pub name: String,
    /// Undashed UUID, as the game expects it.
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    pub xuid: String,
    /// authlib-injector target for accounts on an alternative auth server (`ely.by`).
    pub auth_server: Option<String>,
}

/// Minecraft's own rule for player names.
pub fn validate_name(name: &str) -> Result<()> {
    let ok = (3..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if ok { Ok(()) } else { Err(Error::InvalidName(name.to_owned())) }
}

/// The UUID the game itself assigns to an offline player:
/// a version-3 UUID of `OfflinePlayer:<name>`, returned without dashes.
pub fn offline_uuid(name: &str) -> String {
    let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    hex::encode(bytes)
}

/// A random 128-bit token in hex. Uses the OS random source through `std`'s hasher seeds
/// mixed with time, which is plenty for a non-secret client identifier.
fn new_client_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        chunk.copy_from_slice(&h.finish().to_le_bytes());
    }
    hex::encode(bytes)
}

/// `accounts.json`: saved accounts, the active one and whether ownership was verified.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AccountStore {
    /// Set after a Microsoft account that owns the game signs in. Kept even if
    /// that account is later removed, so offline names keep working.
    #[serde(default)]
    ownership_verified: bool,
    #[serde(default)]
    accounts: Vec<Account>,
    #[serde(default)]
    active: Option<String>,
    /// Identifies this Glint install to Yggdrasil-style auth servers such as Ely.by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_token: Option<String>,
}

impl AccountStore {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|source| Error::Json { path: path.to_owned(), source }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Error::Io { path: path.to_owned(), source }),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let io = |source| Error::Io { path: path.to_owned(), source };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).expect("accounts serialize");
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, path).map_err(io)
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    pub fn ownership_verified(&self) -> bool {
        self.ownership_verified
    }

    /// Called by Microsoft sign-in after it confirms the account owns Java Edition.
    pub fn mark_ownership_verified(&mut self) {
        self.ownership_verified = true;
    }

    pub fn active(&self) -> Option<&Account> {
        let uuid = self.active.as_deref()?;
        self.accounts.iter().find(|a| a.uuid() == uuid)
    }

    pub fn set_active(&mut self, uuid: &str) -> bool {
        let exists = self.accounts.iter().any(|a| a.uuid() == uuid);
        if exists {
            self.active = Some(uuid.to_owned());
        }
        exists
    }

    pub fn add_offline(&mut self, name: &str, policy: OfflinePolicy) -> Result<&Account> {
        if policy == OfflinePolicy::RequireOwnership && !self.ownership_verified {
            return Err(Error::OwnershipRequired);
        }
        validate_name(name)?;
        if self.accounts.iter().any(|a| a.name().eq_ignore_ascii_case(name)) {
            return Err(Error::Duplicate(name.to_owned()));
        }
        let account = Account::Offline { name: name.to_owned(), uuid: offline_uuid(name) };
        if self.active.is_none() {
            self.active = Some(account.uuid().to_owned());
        }
        self.accounts.push(account);
        Ok(self.accounts.last().expect("just pushed"))
    }

    /// Adds or refreshes an Ely.by account after a successful sign-in and makes it active.
    pub fn add_ely(&mut self, name: &str, uuid: &str, policy: OfflinePolicy) -> Result<&Account> {
        if policy == OfflinePolicy::RequireOwnership && !self.ownership_verified {
            return Err(Error::OwnershipRequired);
        }
        let account = Account::Ely { name: name.to_owned(), uuid: uuid.to_owned() };
        let index = match self.accounts.iter().position(|a| a.uuid() == uuid) {
            Some(i) => {
                self.accounts[i] = account;
                i
            }
            None => {
                self.accounts.push(account);
                self.accounts.len() - 1
            }
        };
        self.active = Some(uuid.to_owned());
        Ok(&self.accounts[index])
    }

    /// This install's client token, created on first use. Save the store afterwards.
    pub fn client_token(&mut self) -> String {
        self.client_token.get_or_insert_with(new_client_token).clone()
    }

    pub fn remove(&mut self, uuid: &str) {
        self.accounts.retain(|a| a.uuid() != uuid);
        if self.active.as_deref() == Some(uuid) {
            self.active = self.accounts.first().map(|a| a.uuid().to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_the_game() {
        // UUID.nameUUIDFromBytes("OfflinePlayer:Notch") in Java.
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
        assert_eq!(&offline_uuid("Steve")[12..13], "3", "version nibble");
    }

    #[test]
    fn names_follow_minecraft_rules() {
        for ok in ["abc", "Player_1", "ABCDEFGHIJKLMNOP"] {
            assert!(validate_name(ok).is_ok(), "{ok}");
        }
        for bad in ["ab", "ABCDEFGHIJKLMNOPQ", "with space", "кириллица", "dash-name", ""] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn offline_names_require_ownership_in_release_policy() {
        let mut store = AccountStore::default();
        assert!(matches!(
            store.add_offline("Player", OfflinePolicy::RequireOwnership),
            Err(Error::OwnershipRequired)
        ));

        store.mark_ownership_verified();
        let account = store.add_offline("Player", OfflinePolicy::RequireOwnership).unwrap().clone();
        assert_eq!(store.active(), Some(&account));
        assert!(matches!(
            store.add_offline("player", OfflinePolicy::RequireOwnership),
            Err(Error::Duplicate(_))
        ));
    }

    #[test]
    fn ely_accounts_follow_the_same_rule_and_replace_by_uuid() {
        let mut store = AccountStore::default();
        assert!(matches!(store.add_ely("Steve", "u1", OfflinePolicy::RequireOwnership), Err(Error::OwnershipRequired)));

        store.add_ely("Steve", "u1", OfflinePolicy::Unrestricted).unwrap();
        store.add_ely("Steve2", "u1", OfflinePolicy::Unrestricted).unwrap();
        assert_eq!(store.accounts().len(), 1);
        let account = store.active().unwrap();
        assert_eq!(account.name(), "Steve2");
        let identity = account.identity(Some("token"));
        assert_eq!((identity.access_token.as_str(), identity.user_type.as_str()), ("token", "mojang"));
        assert_eq!(identity.auth_server.as_deref(), Some("ely.by"));
    }

    #[test]
    fn client_token_is_stable_once_created() {
        let mut store = AccountStore::default();
        let token = store.client_token();
        assert_eq!(token.len(), 32);
        assert_eq!(store.client_token(), token);
        assert_ne!(AccountStore::default().client_token(), token);
    }

    #[test]
    fn development_policy_skips_the_ownership_check() {
        let mut store = AccountStore::default();
        assert!(store.add_offline("DevPlayer", OfflinePolicy::Unrestricted).is_ok());
    }

    #[test]
    fn store_round_trips_and_keeps_verification_after_removal() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("accounts.json");

        let mut store = AccountStore::load(&path).unwrap();
        store.mark_ownership_verified();
        let a = store.add_offline("Alpha", OfflinePolicy::RequireOwnership).unwrap().uuid().to_owned();
        let b = store.add_offline("Beta", OfflinePolicy::RequireOwnership).unwrap().uuid().to_owned();
        assert!(store.set_active(&b));
        store.remove(&b);
        assert_eq!(store.active().map(Account::uuid), Some(a.as_str()));
        store.save(&path).unwrap();

        let loaded = AccountStore::load(&path).unwrap();
        assert!(loaded.ownership_verified());
        assert_eq!(loaded.accounts().len(), 1);
        assert_eq!(loaded.active().unwrap().identity(None).access_token, "0");
    }
}
