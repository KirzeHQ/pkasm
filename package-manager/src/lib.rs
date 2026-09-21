//! Account and package-management domain logic shared by the CLI and website.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A package's identity in a registry.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PackageId {
    pub name: String,
    pub version: String,
}

impl PackageId {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }
}

/// Metadata required to publish and discover a package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Package {
    pub id: PackageId,
    pub owner: String,
    pub license: Option<String>,
    pub author: Option<String>,
    pub package_type: String,
    pub targets: Vec<String>,
    pub dependencies: BTreeMap<String, String>,
}

impl Package {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: PackageId::new(name, version),
            owner: String::new(),
            license: None,
            author: None,
            package_type: "binary".to_owned(),
            targets: Vec::new(),
            dependencies: BTreeMap::new(),
        }
    }
}

/// A package archive and its metadata returned by a download.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDownload {
    pub package: Package,
    pub archive: Vec<u8>,
}

/// A registered account.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Account {
    pub username: String,
}

/// A temporary authenticated session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub token: String,
    pub account: Account,
}

/// A credential intended for CLI and registry API access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key: String,
}

/// Non-secret API-key information shown in account interfaces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiKeySummary {
    pub id: String,
    pub name: String,
}

/// Errors returned by package and account operations.
#[derive(Debug, Eq, PartialEq)]
pub enum PackageManagerError {
    AccountAlreadyExists(String),
    InvalidCredentials,
    InvalidSession,
    InvalidApiKey,
    InvalidUsername,
    EmptyPassword,
    EmptyApiKeyName,
    PackageAlreadyPublished(PackageId),
}

impl fmt::Display for PackageManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountAlreadyExists(username) => {
                write!(formatter, "account {username} already exists")
            }
            Self::InvalidCredentials => write!(formatter, "invalid credentials"),
            Self::InvalidSession => write!(formatter, "invalid session"),
            Self::InvalidApiKey => write!(formatter, "invalid API key"),
            Self::InvalidUsername => write!(formatter, "username cannot be empty"),
            Self::EmptyPassword => write!(formatter, "password cannot be empty"),
            Self::EmptyApiKeyName => write!(formatter, "API key name cannot be empty"),
            Self::PackageAlreadyPublished(id) => {
                write!(
                    formatter,
                    "package {}@{} is already published",
                    id.name, id.version
                )
            }
        }
    }
}

impl Error for PackageManagerError {}

/// Storage-independent package archive interface.
pub trait Registry {
    fn publish(&mut self, package: Package, archive: Vec<u8>) -> Result<(), PackageManagerError>;
    fn get(&self, id: &PackageId) -> Option<Package>;
    fn download(&self, id: &PackageId) -> Option<PackageDownload>;
    fn search(&self, query: &str) -> Vec<Package>;
}

struct AccountRecord {
    account: Account,
    password_hash: String,
}

struct ApiKeyRecord {
    id: String,
    name: String,
    username: String,
}

/// Account and package operations shared by the CLI and website.
pub struct PackageManager<R> {
    registry: R,
    accounts: BTreeMap<String, AccountRecord>,
    sessions: BTreeMap<String, String>,
    api_keys: BTreeMap<String, ApiKeyRecord>,
}

impl<R: Registry> PackageManager<R> {
    pub fn new(registry: R) -> Self {
        Self {
            registry,
            accounts: BTreeMap::new(),
            sessions: BTreeMap::new(),
            api_keys: BTreeMap::new(),
        }
    }

    pub fn register(
        &mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Account, PackageManagerError> {
        let username = username.into();
        let password = password.into();
        if username.trim().is_empty() {
            return Err(PackageManagerError::InvalidUsername);
        }
        if password.is_empty() {
            return Err(PackageManagerError::EmptyPassword);
        }
        if self.accounts.contains_key(&username) {
            return Err(PackageManagerError::AccountAlreadyExists(username));
        }

        let account = Account {
            username: username.clone(),
        };
        self.accounts.insert(
            username,
            AccountRecord {
                account: account.clone(),
                password_hash: hash_password(&password),
            },
        );
        Ok(account)
    }

    pub fn register_session(
        &mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Session, PackageManagerError> {
        let username = username.into();
        let password = password.into();
        self.register(username.clone(), password.clone())?;
        self.login(&username, &password)
    }

    pub fn login(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<Session, PackageManagerError> {
        let record = self
            .accounts
            .get(username)
            .filter(|record| record.password_hash == hash_password(password))
            .ok_or(PackageManagerError::InvalidCredentials)?;
        let token = Uuid::new_v4().to_string();
        self.sessions.insert(token.clone(), username.to_owned());

        Ok(Session {
            token,
            account: record.account.clone(),
        })
    }

    pub fn logout(&mut self, token: &str) -> bool {
        self.sessions.remove(token).is_some()
    }

    pub fn account_for_session(&self, token: &str) -> Option<Account> {
        self.sessions
            .get(token)
            .and_then(|username| self.accounts.get(username))
            .map(|record| record.account.clone())
    }

    pub fn create_api_key(
        &mut self,
        session_token: &str,
        name: impl Into<String>,
    ) -> Result<ApiKey, PackageManagerError> {
        let username = self
            .sessions
            .get(session_token)
            .ok_or(PackageManagerError::InvalidSession)?
            .clone();
        let name = name.into();
        if name.trim().is_empty() {
            return Err(PackageManagerError::EmptyApiKeyName);
        }

        let key = format!("pkasm_{}", Uuid::new_v4());
        let id = Uuid::new_v4().to_string();
        self.api_keys.insert(
            hash_secret(&key),
            ApiKeyRecord {
                id: id.clone(),
                name: name.clone(),
                username,
            },
        );
        Ok(ApiKey { id, name, key })
    }

    pub fn api_keys(&self, session_token: &str) -> Result<Vec<ApiKeySummary>, PackageManagerError> {
        let username = self
            .sessions
            .get(session_token)
            .ok_or(PackageManagerError::InvalidSession)?;
        Ok(self
            .api_keys
            .values()
            .filter(|record| record.username == *username)
            .map(|record| ApiKeySummary {
                id: record.id.clone(),
                name: record.name.clone(),
            })
            .collect())
    }

    pub fn publish_with_api_key(
        &mut self,
        api_key: &str,
        mut package: Package,
        archive: Vec<u8>,
    ) -> Result<(), PackageManagerError> {
        let username = self
            .api_keys
            .get(&hash_secret(api_key))
            .ok_or(PackageManagerError::InvalidApiKey)?
            .username
            .clone();
        package.owner = username;
        self.registry.publish(package, archive)
    }

    pub fn publish_basic_with_api_key(
        &mut self,
        api_key: &str,
        name: impl Into<String>,
        version: impl Into<String>,
        archive: Vec<u8>,
    ) -> Result<(), PackageManagerError> {
        self.publish_with_api_key(api_key, Package::new(name, version), archive)
    }

    pub fn revoke_api_key(
        &mut self,
        session_token: &str,
        id: &str,
    ) -> Result<bool, PackageManagerError> {
        let username = self
            .sessions
            .get(session_token)
            .ok_or(PackageManagerError::InvalidSession)?;
        let key_hash = self.api_keys.iter().find_map(|(key_hash, record)| {
            (record.id == id && record.username == *username).then(|| key_hash.clone())
        });
        Ok(key_hash.is_some_and(|key_hash| self.api_keys.remove(&key_hash).is_some()))
    }

    pub fn get(&self, id: &PackageId) -> Option<Package> {
        self.registry.get(id)
    }

    pub fn download(&self, id: &PackageId) -> Option<PackageDownload> {
        self.registry.download(id)
    }

    pub fn search(&self, query: &str) -> Vec<Package> {
        self.registry.search(query)
    }
}

fn hash_password(password: &str) -> String {
    hash_secret(password)
}

fn hash_secret(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// In-memory registry and account backend for local development.
#[derive(Default)]
pub struct InMemoryRegistry {
    packages: BTreeMap<PackageId, PackageDownload>,
}

impl Registry for InMemoryRegistry {
    fn publish(&mut self, package: Package, archive: Vec<u8>) -> Result<(), PackageManagerError> {
        if self.packages.contains_key(&package.id) {
            return Err(PackageManagerError::PackageAlreadyPublished(package.id));
        }

        let id = package.id.clone();
        self.packages
            .insert(id, PackageDownload { package, archive });
        Ok(())
    }

    fn get(&self, id: &PackageId) -> Option<Package> {
        self.packages
            .get(id)
            .map(|download| download.package.clone())
    }

    fn download(&self, id: &PackageId) -> Option<PackageDownload> {
        self.packages.get(id).cloned()
    }

    fn search(&self, query: &str) -> Vec<Package> {
        let query = query.to_lowercase();
        self.packages
            .values()
            .filter(|download| download.package.id.name.to_lowercase().contains(&query))
            .map(|download| download.package.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemoryRegistry, Package, PackageId, PackageManager, PackageManagerError};

    #[test]
    fn accounts_can_publish_and_download_packages() {
        let mut manager = PackageManager::new(InMemoryRegistry::default());
        manager
            .register("alice", "password")
            .expect("account should register");
        let session = manager
            .login("alice", "password")
            .expect("account should log in");
        let api_key = manager
            .create_api_key(&session.token, "test client")
            .expect("API key should be created");
        manager
            .publish_with_api_key(
                &api_key.key,
                Package::new("tool", "1.0.0"),
                b"package archive".to_vec(),
            )
            .expect("package should publish");

        let download = manager
            .download(&PackageId::new("tool", "1.0.0"))
            .expect("package should download");
        assert_eq!(download.package.owner, "alice");
        assert_eq!(download.archive, b"package archive");
    }

    #[test]
    fn invalid_api_keys_cannot_publish() {
        let mut manager = PackageManager::new(InMemoryRegistry::default());

        assert_eq!(
            manager.publish_with_api_key("invalid", Package::new("tool", "1.0.0"), Vec::new()),
            Err(PackageManagerError::InvalidApiKey)
        );
    }

    #[test]
    fn duplicate_accounts_and_packages_are_rejected() {
        let mut manager = PackageManager::new(InMemoryRegistry::default());
        manager
            .register("alice", "password")
            .expect("account should register");
        assert_eq!(
            manager.register("alice", "another"),
            Err(PackageManagerError::AccountAlreadyExists(
                "alice".to_owned()
            ))
        );

        let session = manager
            .login("alice", "password")
            .expect("account should log in");
        let api_key = manager
            .create_api_key(&session.token, "test client")
            .expect("API key should be created");
        manager
            .publish_with_api_key(&api_key.key, Package::new("tool", "1.0.0"), Vec::new())
            .expect("first package should publish");
        assert_eq!(
            manager.publish_with_api_key(&api_key.key, Package::new("tool", "1.0.0"), Vec::new()),
            Err(PackageManagerError::PackageAlreadyPublished(
                PackageId::new("tool", "1.0.0")
            ))
        );
        assert!(
            manager
                .revoke_api_key(&session.token, &api_key.id)
                .expect("API key deletion should succeed")
        );
        assert!(
            !manager
                .revoke_api_key(&session.token, &api_key.id)
                .expect("deleting the same API key should be a no-op")
        );
    }
}
