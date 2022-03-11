use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    rc::Rc,
};

use classfile_parser::class_parser_opt;
use rhojvm_base::{
    class::ClassFileData,
    data::{
        class_file_loader::{ClassFileLoader, LoadClassFileError},
        class_names::ClassNames,
    },
    id::ClassId,
    util,
};

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
        let rel_path = class_path_iter_to_relative_path(path);
        self.direct_load_class_file_from_rel_path(class_file_id, rel_path)
            .map(Some)
    }
}

pub(crate) fn class_path_iter_to_relative_path<'a>(
    class_path: impl Iterator<Item = &'a str> + Clone,
) -> PathBuf {
    let count = class_path.clone().count();
    let mut path = PathBuf::with_capacity(count);
    for path_part in class_path {
        path.push(path_part);
    }

    path.set_extension("class");

    path
}
