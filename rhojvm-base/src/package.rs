use crate::id::{self, PackageId};

#[derive(Debug, Clone, Default)]
pub struct Packages {
    // TODO: Should we use a hashmap with a custom hasher?
    // the main issue is that we still need to do a bunch of manual handling to ensure correctness
    packages: Vec<Package>,
}
impl Packages {
    fn add(&mut self, name: String) -> PackageId {
        let package = Package::new(name);
        let id = package.id();
        self.packages.push(package);
        id
    }

    #[must_use]
    pub fn get(&self, id: PackageId) -> Option<&Package> {
        self.packages.iter().find(|x| x.id() == id)
    }

    #[must_use]
    pub fn path_find(&self, name: &str) -> Option<&Package> {
        let id = id::hash_access_path(name);
        self.get(id)
    }

    #[must_use]
    /// Note: you may want to handle the empty case yourself
    pub fn parts_find<T: AsRef<str>>(&self, path: &[T]) -> Option<&Package> {
        let id = id::hash_access_path_slice(path);
        self.get(id)
    }

    #[must_use]
    pub fn iter_parts_find<'a>(
        &self,
        path: impl Iterator<Item = &'a str> + Clone,
    ) -> Option<&Package> {
        let id = id::hash_access_path_iter(path, false);
        self.get(id)
    }

    #[must_use]
    pub fn string_path_create_if_needed(&mut self, name: String) -> PackageId {
        if let Some(p) = self.path_find(&name) {
            p.id()
        } else {
            self.add(name)
        }
    }

    #[must_use]
    pub fn str_path_create_if_needed(&mut self, name: &str) -> PackageId {
        if let Some(p) = self.path_find(name) {
            p.id()
        } else {
            self.add(name.to_owned())
        }
    }

    #[must_use]
    pub fn iter_parts_create_if_needed<'a>(
        &mut self,
        path: impl Iterator<Item = &'a str> + Clone,
    ) -> PackageId {
        let id = id::hash_access_path_iter(path.clone(), false);
        if let Some(p) = self.get(id) {
            p.id()
        } else {
            self.add(path.collect::<Vec<&'a str>>().join("."))
        }
    }

    #[must_use]
    pub fn iter(&self) -> std::slice::Iter<'_, Package> {
        self.packages.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    id: PackageId,
    /// `java`, `java.lang`, `java.io`
    name: String,
}
impl Package {
    #[must_use]
    fn new(name: String) -> Self {
        let id = id::hash_access_path(&name);
        Self { id, name }
    }

    #[must_use]
    pub fn id(&self) -> PackageId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}
