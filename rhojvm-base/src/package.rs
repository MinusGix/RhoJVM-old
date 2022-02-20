use crate::id::PackageId;

#[derive(Debug, Clone, Default)]
pub struct Packages {
    // TODO: Should we use a hashmap with a custom hasher?
    // the main issue is that we still need to do a bunch of manual handling to ensure correctness
    packages: Vec<Package>,
    next_id: u64,
}
impl Packages {
    fn get_new_id(&mut self) -> PackageId {
        let id = PackageId::new_unchecked(self.next_id);
        self.next_id += 1;
        id
    }

    /// Add the package unchecked to the list of packages
    fn add(&mut self, name: Vec<u8>) -> PackageId {
        let id = self.get_new_id();
        let package = Package::new(id, name);
        self.packages.push(package);
        id
    }

    #[must_use]
    pub fn get(&self, id: PackageId) -> Option<&Package> {
        self.packages.iter().find(|x| x.id() == id)
    }

    #[must_use]
    pub fn path_find(&self, name: &[u8]) -> Option<&Package> {
        self.packages.iter().find(|x| x.name == name)
    }

    #[must_use]
    pub fn vec_path_create_if_needed(&mut self, name: Vec<u8>) -> PackageId {
        if let Some(p) = self.path_find(&name) {
            p.id()
        } else {
            self.add(name)
        }
    }

    #[must_use]
    pub fn slice_path_create_if_needed(&mut self, name: &[u8]) -> PackageId {
        if let Some(p) = self.path_find(name) {
            p.id()
        } else {
            self.add(name.to_owned())
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
    /// `java`, `java/lang`, `java/io`
    name: Vec<u8>,
}
impl Package {
    #[must_use]
    fn new(id: PackageId, name: Vec<u8>) -> Self {
        Self { id, name }
    }

    #[must_use]
    pub fn id(&self) -> PackageId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &[u8] {
        &self.name
    }
}
