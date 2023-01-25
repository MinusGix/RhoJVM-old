use std::{collections::HashMap, hash::BuildHasherDefault};

use crate::{
    class::ClassFileInfo,
    id::ClassId,
    util::{self},
    StepError,
};

use super::class_file_loader::{ClassFileLoader, LoadClassFileError};
use super::class_names::ClassNames;

pub struct ClassFiles {
    pub loader: Box<dyn ClassFileLoader + 'static>,
    map: HashMap<
        ClassId,
        ClassFileInfo,
        <util::HashWrapper as util::HashWrapperTrait<ClassId>>::HashMapHasher,
    >,
}

impl ClassFiles {
    #[must_use]
    pub fn new(loader: impl ClassFileLoader + 'static) -> ClassFiles {
        ClassFiles {
            loader: Box::new(loader),
            map: HashMap::with_hasher(BuildHasherDefault::default()),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[must_use]
    pub fn contains_key(&self, key: &ClassId) -> bool {
        self.map.contains_key(key)
    }

    #[must_use]
    pub fn get(&self, key: &ClassId) -> Option<&ClassFileInfo> {
        self.map.get(key)
    }

    #[must_use]
    pub fn get_mut(&mut self, key: &ClassId) -> Option<&mut ClassFileInfo> {
        self.map.get_mut(key)
    }

    pub(crate) fn set_at(&mut self, key: ClassId, val: ClassFileInfo) {
        if self.map.insert(key, val).is_some() {
            tracing::warn!("Duplicate setting for Classes with {:?}", key);
            debug_assert!(false);
        }
    }

    /// This is primarily for the JVM impl to load classes from user input
    pub fn load_by_class_path_slice<T: AsRef<str>>(
        &mut self,
        class_names: &mut ClassNames,
        class_path: &[T],
    ) -> Result<ClassId, LoadClassFileError> {
        if class_path.is_empty() {
            return Err(LoadClassFileError::EmptyPath);
        }

        // TODO: This is probably not accurate for more complex utf8
        let class_file_id = class_names
            .gcid_from_iter_bytes(class_path.iter().map(AsRef::as_ref).map(str::as_bytes));
        if self.contains_key(&class_file_id) {
            return Ok(class_file_id);
        }

        let class_file = self
            .loader
            .load_class_file_by_id(class_names, class_file_id)?;
        if let Some(class_file) = class_file {
            self.set_at(class_file_id, ClassFileInfo::Data(class_file));
        }

        Ok(class_file_id)
    }

    /// Note: the id should already be registered
    pub fn load_by_class_path_id(
        &mut self,
        class_names: &mut ClassNames,
        class_file_id: ClassId,
    ) -> Result<(), LoadClassFileError> {
        if self.contains_key(&class_file_id) {
            return Ok(());
        }

        let class_file = self
            .loader
            .load_class_file_by_id(class_names, class_file_id)?;
        if let Some(class_file) = class_file {
            // If it has a class file, store it,
            // If it doesn't, whatever
            self.set_at(class_file_id, ClassFileInfo::Data(class_file));
        }

        Ok(())
    }
}
impl std::fmt::Debug for ClassFiles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassFiles")
            .field("loader", &"(unprintable)")
            .field("map", &self.map)
            .finish()
    }
}

/// Provides an 'iterator' over class files as it crawls up from the `class_id` given
/// Note that this *includes* the `class_id` given, and so you may want to skip over it.
#[must_use]
pub fn load_super_class_files_iter(class_file_id: ClassId) -> SuperClassFileIterator {
    SuperClassFileIterator::new(class_file_id)
}

pub struct SuperClassFileIterator {
    topmost: Option<Result<ClassId, StepError>>,
    had_error: bool,
}
impl SuperClassFileIterator {
    /// Construct the iterator, doing basic processing
    pub(crate) fn new(base_class_id: ClassId) -> SuperClassFileIterator {
        SuperClassFileIterator {
            topmost: Some(Ok(base_class_id)),
            had_error: false,
        }
    }

    // This isn't an iterator, unfortunately, because it needs state paseed into `next_item`
    // to make it usable.
    // We can't simply borrow the fields because the code that is using the iterator likely
    // wants to use them too!
    // TODO: We could maybe make a weird iterator trait that takes in:
    // type Args = (&'a ClassDirectories, &'a mut ClassNames, &'a mut ClassFiles)
    // for its next and then for any iterator methods which we need
    pub fn next_item(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
    ) -> Option<Result<ClassId, StepError>> {
        if self.had_error {
            return None;
        }

        // Get the id, returning the error if there is one
        let topmost = match self.topmost.take() {
            Some(Ok(topmost)) => topmost,
            Some(Err(err)) => {
                self.had_error = true;
                return Some(Err(err));
            }
            // We are now done.
            None => return None,
        };

        // Load the class file by the id
        if let Err(err) = class_files.load_by_class_path_id(class_names, topmost) {
            self.had_error = true;
            return Some(Err(err.into()));
        }

        // Not everything has a class file, such as arrays
        let (has_class_file, is_array) = match class_names.name_from_gcid(topmost) {
            Ok((_, info)) => (info.has_class_file(), info.is_array()),
            Err(err) => {
                self.had_error = true;
                return Some(Err(StepError::BadId(err)));
            }
        };
        if has_class_file {
            // We just loaded it
            let class_file = class_files.get(&topmost).unwrap();

            // Get the super class for next iteration, but we delay checking the error
            self.topmost = class_file
                .get_super_class_id(class_names)
                .map_err(StepError::ClassFileIndex)
                .transpose();

            // The class file was initialized
            Some(Ok(topmost))
        } else if is_array {
            // All arrays extend java/lang/Object
            self.topmost = Some(Ok(class_names.object_id()));
            Some(Ok(topmost))
        } else {
            // TODO: Should we do something better here?
            tracing::error!("SuperClassFileIterator ran into an entry that did not have a class file and so was uncertain how to handle it");
            None
        }
    }
}
