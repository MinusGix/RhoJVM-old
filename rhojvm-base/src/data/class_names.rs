use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::atomic::{self, AtomicU32};

use indexmap::{Equivalent, IndexMap};

use crate::{
    code::{method::DescriptorTypeBasic, types::PrimitiveType},
    id::{self, ClassId},
    util::{self},
    BadIdError,
};

#[derive(Debug, Clone)]
pub(crate) enum InternalKind {
    Array,
}
impl InternalKind {
    fn from_slice<T: AsRef<str>>(class_path: &[T]) -> Option<InternalKind> {
        class_path
            .get(0)
            .map(AsRef::as_ref)
            .and_then(InternalKind::from_str)
    }

    fn from_iter<'a>(mut class_path: impl Iterator<Item = &'a str>) -> Option<InternalKind> {
        class_path.next().and_then(InternalKind::from_str)
    }

    fn from_bytes(class_path: &[u8]) -> Option<InternalKind> {
        if id::is_array_class_bytes(class_path) {
            Some(InternalKind::Array)
        } else {
            None
        }
    }

    fn from_raw_class_name(class_path: RawClassNameSlice<'_>) -> Option<InternalKind> {
        Self::from_bytes(class_path.0)
    }

    fn from_str(class_path: &str) -> Option<InternalKind> {
        if id::is_array_class(class_path) {
            Some(InternalKind::Array)
        } else {
            None
        }
    }

    fn has_class_file(&self) -> bool {
        false
    }
}

/// An insert into [`ClassNames`] that is trusted, aka it has all the right values
/// and is computed to be inserted when we have issues getting borrowing right.
/// The variants are private
pub(crate) enum TrustedClassNameInsert {
    /// We already have it
    Id(ClassId),
    /// It needs to be inserted
    Data {
        class_name: RawClassName,
        kind: Option<InternalKind>,
    },
}

// TODO: Should this be using a smallvec? Probably a lot of them are less than 32 bytes, but 64
// would probably be a safer size?
#[derive(Clone)]
pub struct RawClassName(pub Vec<u8>);
impl RawClassName {
    #[must_use]
    pub fn get(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn as_slice(&self) -> RawClassNameSlice<'_> {
        RawClassNameSlice(self.0.as_slice())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl Eq for RawClassName {}
impl PartialEq for RawClassName {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Hash for RawClassName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}
impl std::fmt::Debug for RawClassName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "\"{}\"",
            util::convert_classfile_text(&self.0)
        ))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RawClassNameSlice<'a>(&'a [u8]);
impl<'a> RawClassNameSlice<'a> {
    #[must_use]
    pub fn get(&self) -> &'a [u8] {
        self.0
    }

    #[must_use]
    pub fn to_owned(&self) -> RawClassName {
        RawClassName(self.0.to_owned())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl<'a> Equivalent<RawClassName> for RawClassNameSlice<'a> {
    fn equivalent(&self, key: &RawClassName) -> bool {
        self.0 == key.0
    }
}
impl<'a> Hash for RawClassNameSlice<'a> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // This mimics normal slice hashing, but we explicitly decide how we hash slices
        // This is because in our iterator version, we can't rely on the hash_slice
        // iterating over each piece individually, so
        // [0, 1, 2, 3] might not be the same as hashing [0, 1] and then [2, 3]
        self.0.len().hash(state);
        for piece in self.0 {
            piece.hash(state);
        }
    }
}
impl<'a> std::fmt::Debug for RawClassNameSlice<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("\"{}\"", util::convert_classfile_text(self.0)))
    }
}

/// Used when you have an iterator over slices of bytes which form a single [`RawClassName`]
/// when considered together.
/// This does not insert `/`
#[derive(Clone)]
pub struct RawClassNameBuilderator<I> {
    iter: I,
    length: usize,
}
impl<I> RawClassNameBuilderator<I> {
    pub fn new_single<'a>(iter: I) -> RawClassNameBuilderator<I>
    where
        I: Iterator<Item = &'a [u8]> + Clone,
    {
        RawClassNameBuilderator {
            iter: iter.clone(),
            length: iter.fold(0, |acc, x| acc + x.len()),
        }
    }

    pub fn new_split<'a, J: Iterator<Item = &'a [u8]> + Clone>(
        iter: J,
    ) -> RawClassNameBuilderator<impl Iterator<Item = &'a [u8]> + Clone> {
        let iter = itertools::intersperse(iter, b"/");
        RawClassNameBuilderator::new_single(iter)
    }
}
impl<'a, I: Iterator<Item = &'a [u8]> + Clone> RawClassNameBuilderator<I> {
    /// Compute the kind that this would be
    pub(crate) fn internal_kind(&self) -> Option<InternalKind> {
        self.iter.clone().next().and_then(InternalKind::from_bytes)
    }

    pub(crate) fn into_raw_class_name(self) -> RawClassName {
        RawClassName(self.iter.flatten().copied().collect::<Vec<u8>>())
    }
}
impl<'a, I: Iterator<Item = &'a [u8]> + Clone> Hash for RawClassNameBuilderator<I> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // This mimics slice hashing, as done in RawClassNameSlice
        self.length.hash(state);
        for part in self.iter.clone() {
            for piece in part {
                piece.hash(state);
            }
        }
    }
}
impl<'a, I: Iterator<Item = &'a [u8]> + Clone> Equivalent<RawClassName>
    for RawClassNameBuilderator<I>
{
    fn equivalent(&self, key: &RawClassName) -> bool {
        // If they aren't of the same length t hen they're certainly not equivalent
        if self.length != key.get().len() {
            return false;
        }

        // Their length is known to be equivalent due to the earlier check
        let iter = key
            .get()
            .iter()
            .copied()
            .zip(self.iter.clone().flatten().copied());
        for (p1, p2) in iter {
            if p1 != p2 {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
pub struct ClassNameInfo {
    kind: Option<InternalKind>,
    anonymous: bool,
    id: ClassId,
}
impl ClassNameInfo {
    fn new_kind(kind: Option<InternalKind>, id: ClassId) -> Self {
        ClassNameInfo {
            kind,
            anonymous: false,
            id,
        }
    }

    #[must_use]
    pub fn has_class_file(&self) -> bool {
        if let Some(kind) = &self.kind {
            kind.has_class_file()
        } else {
            true
        }
    }

    #[must_use]
    pub fn is_array(&self) -> bool {
        matches!(self.kind, Some(InternalKind::Array))
    }

    #[must_use]
    pub fn is_anonymous(&self) -> bool {
        self.anonymous
    }
}

#[derive(Debug)]
pub struct ClassNames {
    next_id: AtomicU32,
    names: IndexMap<RawClassName, ClassNameInfo>,
}
impl ClassNames {
    #[must_use]
    pub fn new() -> Self {
        let mut class_names = ClassNames {
            next_id: AtomicU32::new(0),
            // TODO: We could probably choose a better and more accurate default
            // For a basic program, it might fit under this limit
            names: IndexMap::with_capacity(32),
        };

        // Reserve the first id, 0, so it is always for Object
        class_names.gcid_from_bytes(b"java/lang/Object");

        class_names
    }

    /// Construct a new unique id
    fn get_new_id(&mut self) -> ClassId {
        // Based on https://en.cppreference.com/w/cpp/atomic/memory_order in the Relaxed ordering
        // section, Relaxed ordering should work good for a counter that is only incrementing.
        ClassId::new_unchecked(self.next_id.fetch_add(1, atomic::Ordering::Relaxed))
    }

    pub fn init_new_id(&mut self, anonymous: bool) -> ClassId {
        let id = self.get_new_id();
        self.names.insert(
            RawClassName(Vec::new()),
            ClassNameInfo {
                kind: None,
                anonymous,
                id,
            },
        );
        id
    }

    /// Get the id of `b"java/lang/Object"`. Cached.
    #[must_use]
    pub fn object_id(&self) -> ClassId {
        ClassId::new_unchecked(0)
    }

    /// Check if the given id is for an array
    pub fn is_array(&self, id: ClassId) -> Result<bool, BadIdError> {
        self.name_from_gcid(id).map(|x| x.1.is_array())
    }

    /// Get the name and class info for a given id
    pub fn name_from_gcid(
        &self,
        id: ClassId,
    ) -> Result<(RawClassNameSlice<'_>, &ClassNameInfo), BadIdError> {
        // TODO: Can this be done better?
        self.names
            .iter()
            .find(|(_, info)| info.id == id)
            .map(|(data, info)| (data.as_slice(), info))
            .ok_or_else(|| {
                debug_assert!(false, "name_from_gcid: Got a bad id {:?}", id);
                BadIdError { id }
            })
    }

    pub fn gcid_from_bytes(&mut self, class_path: &[u8]) -> ClassId {
        let class_path = RawClassNameSlice(class_path);
        let kind = InternalKind::from_raw_class_name(class_path);

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        self.names
            .insert(class_path.to_owned(), ClassNameInfo::new_kind(kind, id));
        id
    }

    /// Similar to `gcid_from_bytes` but marginally more efficient in the case where you *had to*
    /// construct a vec (but perhaps you should be using a smallvec?) since it can then immediately
    /// store that in the hashmap.
    pub fn gcid_from_vec(&mut self, class_path: Vec<u8>) -> ClassId {
        let class_path = RawClassName(class_path);
        let kind = InternalKind::from_raw_class_name(class_path.as_slice());

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        self.names
            .insert(class_path, ClassNameInfo::new_kind(kind, id));
        id
    }

    pub fn gcid_from_cow(&mut self, class_path: Cow<[u8]>) -> ClassId {
        let kind = InternalKind::from_bytes(&class_path);

        let class_name = RawClassNameSlice(class_path.as_ref());
        if let Some(entry) = self.names.get(&class_name) {
            return entry.id;
        }

        let id = self.get_new_id();
        self.names.insert(
            RawClassName(class_path.into_owned()),
            ClassNameInfo::new_kind(kind, id),
        );
        id
    }

    pub fn gcid_from_iter_bytes<'a, I: Iterator<Item = &'a [u8]> + Clone>(
        &mut self,
        class_path: I,
    ) -> ClassId {
        let class_path = RawClassNameBuilderator::<I>::new_split(class_path);
        let kind = class_path.internal_kind();

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        self.names.insert(
            class_path.into_raw_class_name(),
            ClassNameInfo::new_kind(kind, id),
        );
        id
    }

    pub fn gcid_from_array_of_primitives(&mut self, prim: PrimitiveType) -> ClassId {
        let prefix = prim.as_desc_prefix();
        let class_path = [b"[", prefix];
        let class_path = RawClassNameBuilderator::new_single(class_path.into_iter());

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        let class_path = class_path.into_raw_class_name();
        self.names.insert(
            class_path,
            ClassNameInfo::new_kind(Some(InternalKind::Array), id),
        );

        id
    }

    pub fn gcid_from_level_array_of_primitives(
        &mut self,
        level: NonZeroUsize,
        prim: PrimitiveType,
    ) -> ClassId {
        let prefix = prim.as_desc_prefix();
        let class_path = std::iter::repeat(b"[" as &[u8]).take(level.get());
        let class_path = class_path.chain([prefix]);
        let class_path = RawClassNameBuilderator::new_single(class_path);

        if let Some(entry) = self.names.get(&class_path) {
            return entry.id;
        }

        let id = self.get_new_id();
        let class_path = class_path.into_raw_class_name();
        self.names.insert(
            class_path,
            ClassNameInfo::new_kind(Some(InternalKind::Array), id),
        );

        id
    }

    pub fn gcid_from_level_array_of_class_id(
        &mut self,
        level: NonZeroUsize,
        class_id: ClassId,
    ) -> Result<ClassId, BadIdError> {
        let (class_name, class_info) = self.name_from_gcid(class_id)?;

        let first_iter = std::iter::repeat(b"[" as &[u8]).take(level.get());

        // Different branches because the iterator will have a different type
        let class_path = if class_info.is_array() {
            // [[{classname} and the like
            let class_path = first_iter.chain([class_name.get()]);
            let class_path = RawClassNameBuilderator::new_single(class_path);

            // Check if it already exists
            if let Some(entry) = self.names.get(&class_path) {
                return Ok(entry.id);
            }

            class_path.into_raw_class_name()
        } else {
            // L{classname};
            let class_path = first_iter.chain([b"L", class_name.get(), b";"]);
            let class_path = RawClassNameBuilderator::new_single(class_path);

            // Check if it already exists
            if let Some(entry) = self.names.get(&class_path) {
                return Ok(entry.id);
            }

            class_path.into_raw_class_name()
        };

        // If we got here then it doesn't already exist
        let id = self.get_new_id();
        self.names.insert(
            class_path,
            ClassNameInfo::new_kind(Some(InternalKind::Array), id),
        );

        Ok(id)
    }

    pub fn gcid_from_level_array_of_desc_type_basic(
        &mut self,
        level: NonZeroUsize,
        component: DescriptorTypeBasic,
    ) -> Result<ClassId, BadIdError> {
        let name_iter = component.as_desc_iter(self)?;
        let class_path = std::iter::repeat(b"[" as &[u8])
            .take(level.get())
            .chain(name_iter);
        let class_path = RawClassNameBuilderator::new_single(class_path);

        if let Some(entry) = self.names.get(&class_path) {
            return Ok(entry.id);
        }

        let class_path = class_path.into_raw_class_name();
        let id = self.get_new_id();
        self.names.insert(
            class_path,
            ClassNameInfo::new_kind(Some(InternalKind::Array), id),
        );

        Ok(id)
    }

    pub(crate) fn insert_key_from_iter_single<'a>(
        &self,
        class_path: impl Iterator<Item = &'a [u8]> + Clone,
    ) -> TrustedClassNameInsert {
        let class_path = RawClassNameBuilderator::new_single(class_path);
        if let Some(entry) = self.names.get(&class_path) {
            TrustedClassNameInsert::Id(entry.id)
        } else {
            let kind = class_path.internal_kind();
            TrustedClassNameInsert::Data {
                class_name: class_path.into_raw_class_name(),
                kind,
            }
        }
    }

    pub(crate) fn insert_trusted_insert(&mut self, insert: TrustedClassNameInsert) -> ClassId {
        match insert {
            TrustedClassNameInsert::Id(id) => id,
            TrustedClassNameInsert::Data { class_name, kind } => {
                let id = self.get_new_id();
                self.names
                    .insert(class_name, ClassNameInfo::new_kind(kind, id));
                id
            }
        }
    }

    /// Get the information in a nice representation for logging
    /// The output of this function is not guaranteed
    #[must_use]
    pub fn tpath(&self, id: ClassId) -> &str {
        self.name_from_gcid(id)
            .map(|x| x.0)
            .map(|x| std::str::from_utf8(x.0))
            // It is fine for it to be invalid utf8, but at the current moment we don't bother
            // converting it
            .unwrap_or(Ok("[UNKNOWN CLASS NAME]"))
            .unwrap_or("[INVALID UTF8]")
    }
}

impl Default for ClassNames {
    fn default() -> Self {
        Self::new()
    }
}
