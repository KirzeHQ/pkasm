//! Shared package-management domain logic for the CLI and registry website.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

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
            license: None,
            author: None,
            package_type: "binary".to_owned(),
            targets: Vec::new(),
            dependencies: BTreeMap::new(),
        }
    }
}

/// Errors returned by registry implementations.
#[derive(Debug, Eq, PartialEq)]
pub enum RegistryError {
    AlreadyPublished(PackageId),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyPublished(id) => {
                write!(
                    formatter,
                    "package {}@{} is already published",
                    id.name, id.version
                )
            }
        }
    }
}

impl Error for RegistryError {}

/// Storage-independent interface implemented by a package registry.
pub trait Registry {
    fn publish(&mut self, package: Package) -> Result<(), RegistryError>;
    fn get(&self, id: &PackageId) -> Option<Package>;
    fn search(&self, query: &str) -> Vec<Package>;
}

/// Package operations shared by the CLI and website.
pub struct PackageManager<R> {
    registry: R,
}

impl<R: Registry> PackageManager<R> {
    pub fn new(registry: R) -> Self {
        Self { registry }
    }

    pub fn publish(&mut self, package: Package) -> Result<(), RegistryError> {
        self.registry.publish(package)
    }

    pub fn get(&self, id: &PackageId) -> Option<Package> {
        self.registry.get(id)
    }

    pub fn search(&self, query: &str) -> Vec<Package> {
        self.registry.search(query)
    }

    pub fn into_registry(self) -> R {
        self.registry
    }
}

/// In-memory registry for local development and tests.
#[derive(Default)]
pub struct InMemoryRegistry {
    packages: BTreeMap<PackageId, Package>,
}

impl Registry for InMemoryRegistry {
    fn publish(&mut self, package: Package) -> Result<(), RegistryError> {
        if self.packages.contains_key(&package.id) {
            return Err(RegistryError::AlreadyPublished(package.id));
        }

        self.packages.insert(package.id.clone(), package);
        Ok(())
    }

    fn get(&self, id: &PackageId) -> Option<Package> {
        self.packages.get(id).cloned()
    }

    fn search(&self, query: &str) -> Vec<Package> {
        let query = query.to_lowercase();
        self.packages
            .values()
            .filter(|package| package.id.name.to_lowercase().contains(&query))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{InMemoryRegistry, Package, PackageId, PackageManager, Registry, RegistryError};

    #[test]
    fn publishes_and_finds_packages() {
        let mut manager = PackageManager::new(InMemoryRegistry::default());
        let package = Package::new("assembler-tools", "0.1.0");
        let id = package.id.clone();

        manager
            .publish(package.clone())
            .expect("publish should succeed");

        assert_eq!(manager.get(&id), Some(package.clone()));
        assert_eq!(manager.search("ASSEMBLER"), vec![package]);
    }

    #[test]
    fn rejects_duplicate_versions() {
        let mut registry = InMemoryRegistry::default();
        let package = Package::new("tool", "1.0.0");
        let id = package.id.clone();

        registry
            .publish(package.clone())
            .expect("first publish should succeed");

        assert_eq!(
            registry.publish(package),
            Err(RegistryError::AlreadyPublished(id))
        );
    }

    #[test]
    fn package_ids_are_versioned() {
        let mut registry = InMemoryRegistry::default();
        registry
            .publish(Package::new("tool", "1.0.0"))
            .expect("first version should publish");
        registry
            .publish(Package::new("tool", "2.0.0"))
            .expect("second version should publish");

        assert!(registry.get(&PackageId::new("tool", "1.0.0")).is_some());
        assert!(registry.get(&PackageId::new("tool", "2.0.0")).is_some());
    }
}
