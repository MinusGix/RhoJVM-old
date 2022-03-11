use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use classfile_parser::class_parser_opt;

use crate::{
    class::ClassFileData,
    id::ClassId,
    util::{self},
    BadIdError,
};

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
}

#[derive(Debug, Default, Clone)]
pub struct ClassDirectories {
    directories: Vec<PathBuf>,
}
impl ClassDirectories {
    pub fn add(&mut self, path: &Path) -> std::io::Result<()> {
        self.directories.push(path.canonicalize()?);
        Ok(())
    }

    #[must_use]
    pub fn load_class_file_with_rel_path(&self, rel_path: &Path) -> Option<(PathBuf, File)> {
        for class_dir in &self.directories {
            // TODO: is it remotely feasible to not allocate without notable extra fs calls?
            let mut full_path = class_dir.clone();
            full_path.push(rel_path);

            if let Ok(file) = File::open(&full_path) {
                return Some((full_path, file));
            }
        }
        None
    }

    pub fn direct_load_class_file_from_rel_path(
        &self,
        class_file_id: ClassId,
        rel_path: PathBuf,
    ) -> Result<ClassFileData, LoadClassFileError> {
        use classfile_parser::parser::ParseData;

        if let Some((file_path, mut file)) = self.load_class_file_with_rel_path(&rel_path) {
            let mut data = Vec::new();
            file.read_to_end(&mut data)
                .map_err(LoadClassFileError::ReadError)?;
            let data = Rc::from(data);

            // TODO: Better errors
            let (rem_data, class_file) = class_parser_opt(ParseData::new(&data))
                .map_err(|x| format!("{:?}", x))
                .map_err(LoadClassFileError::ClassFileParseError)?;
            // TODO: Don't assert
            debug_assert!(rem_data.is_empty());

            Ok(ClassFileData::new(
                class_file_id,
                file_path,
                data,
                class_file,
            ))
        } else {
            Err(LoadClassFileError::NonexistentFile(rel_path))
        }
    }
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

impl ClassFileLoader for ClassDirectories {
    fn load_class_file_by_id(
        &self,
        class_names: &ClassNames,
        class_file_id: ClassId,
    ) -> Result<Option<ClassFileData>, LoadClassFileError> {
        let (class_name, class_info) = class_names
            .name_from_gcid(class_file_id)
            .map_err(LoadClassFileError::BadId)?;

        // It doesn't have a class file at all, so whatever
        if !class_info.has_class_file() {
            return Ok(None);
        }

        let path = util::convert_classfile_text(class_name.get());
        let path = util::access_path_iter(&path);
        let rel_path = util::class_path_iter_to_relative_path(path);
        self.direct_load_class_file_from_rel_path(class_file_id, rel_path)
            .map(Some)
    }
}
