pub type Id = u64;
pub type ClassFileId = Id;
pub type ClassId = Id;
pub type GeneralClassId = Id;

pub type PackageId = Id;

/// This is an index into the methods
/// This is not meaningful without a class
pub type MethodIndex = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MethodId {
    class_id: ClassId,
    method_index: MethodIndex,
}
impl MethodId {
    #[must_use]
    pub fn unchecked_compose(class_id: ClassId, method_index: MethodIndex) -> Self {
        Self {
            class_id,
            method_index,
        }
    }

    #[must_use]
    pub fn decompose(self) -> (ClassId, MethodIndex) {
        (self.class_id, self.method_index)
    }
}

pub(crate) fn is_array_class(first: &str) -> bool {
    first.starts_with('[')
}

pub(crate) fn is_array_class_bytes(first: &[u8]) -> bool {
    first.starts_with(&[b'['])
}
