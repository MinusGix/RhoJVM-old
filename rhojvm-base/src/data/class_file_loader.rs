use std::{error::Error, path::PathBuf};

use crate::{class::ClassFileData, id::ClassId, BadIdError};

use super::class_names::ClassNames;

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadClassFileError {
    /// The path given was empty
    EmptyPath,
    /// The file didn't exist with the relative path
    NonexistentFile(PathBuf),
    /// The class didn't exist
    Nonexistent,
    /// There was an error in reading the file
    ReadError(std::io::Error),
    /// There was an error in parsing the class file
    ClassFileParseError(String),
    /// There was a bad class file id
    BadId(BadIdError),
    OpaqueError(Box<dyn Error>),
}

// TODO: Remove clone
/// Note: Not exactly a class loader in the java sense, but does somewhat similar things
pub trait ClassFileLoader {
    /// Load the class file with the given id
    /// Note: It should only return `Ok(None)` if it had good reason to believe that it should not
    /// have a class file at all.
    /// Return `LoadClassFileError::Nonexistent` (or related) if it was not found.
    fn load_class_file_by_id(
        &self,
        class_names: &ClassNames,
        class_file_id: ClassId,
    ) -> Result<Option<ClassFileData>, LoadClassFileError>;
}
impl<'a, T: ClassFileLoader> ClassFileLoader for &'a T {
    fn load_class_file_by_id(
        &self,
        class_names: &ClassNames,
        class_file_id: ClassId,
    ) -> Result<Option<ClassFileData>, LoadClassFileError> {
        <T as ClassFileLoader>::load_class_file_by_id(self, class_names, class_file_id)
    }
}
