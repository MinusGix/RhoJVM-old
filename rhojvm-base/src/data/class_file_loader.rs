use std::{error::Error, path::PathBuf};

use crate::{class::ClassFileData, id::ClassId, BadIdError};

use super::class_names::ClassNames;

#[derive(Debug)]
#[non_exhaustive]
pub enum LoadClassFileError {
    /// The path given was empty
    EmptyPath,
    /// That class file has already been loaded.
    /// Note that the [`Command::LoadClassFile`] does not care if it already exists, but
    /// this helps be explicit about expectations in the code
    AlreadyExists,
    /// The file didn't exist with the relative path
    NonexistentFile(PathBuf),
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
