use std::{fs::File, io::Read, path::PathBuf, rc::Rc};

use classfile_parser::{class_parser_opt, parser::ParseData};
use rhojvm_base::{
    class::ClassFileData,
    data::{
        class_file_loader::{ClassFileLoader, LoadClassFileError},
        class_names::ClassNames,
    },
    id::ClassId,
    util::{access_path_iter, convert_classfile_text},
};
use zip::ZipArchive;

use crate::class_path_iter_to_relative_path_string;

#[derive(Debug)]
pub enum LoadManifestError {
    /// An error in getting it from the zip
    /// This might be an error in decoding it or it might just not exist.
    Zip(zip::result::ZipError),
    /// An error while reading the file out
    Read(std::io::Error),
    /// The manifest was not a file
    NotFile,
    Parse(kv_parser::KeyValueParseError),
}

const MANIFEST_PATH: &str = "META-INF/MANIFEST.MF";

/// A class file loader specifically for loading classes from a given jar file
#[derive(Debug)]
pub struct JarClassFileLoader {
    jar_path: PathBuf,
    archive: ZipArchive<File>,
}
impl JarClassFileLoader {
    pub fn new(jar_path: PathBuf) -> std::io::Result<JarClassFileLoader> {
        let file = std::fs::File::open(&jar_path)?;
        let archive = zip::ZipArchive::new(file)?;

        Ok(JarClassFileLoader { jar_path, archive })
    }

    pub fn load_manifest(&mut self) -> Result<kv_parser::KeyValueData, LoadManifestError> {
        let mut manifest_file = self
            .archive
            .by_name(MANIFEST_PATH)
            .map_err(LoadManifestError::Zip)?;
        if !manifest_file.is_file() {
            return Err(LoadManifestError::NotFile);
        }

        let mut manifest = String::new();
        manifest_file
            .read_to_string(&mut manifest)
            .map_err(LoadManifestError::Read)?;

        let data = kv_parser::parse_keyvalue_data(&manifest, |warning| {
            tracing::warn!(
                "When parsing manifest file within jar '{:?}': {:?}",
                self.jar_path,
                warning
            );
        })
        .map_err(LoadManifestError::Parse)?;

        Ok(data)
    }
}
impl ClassFileLoader for JarClassFileLoader {
    fn load_class_file_by_id(
        &mut self,
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

        let path = convert_classfile_text(class_name.get());
        let path = access_path_iter(&path);

        let path = class_path_iter_to_relative_path_string(path);

        let mut file = self
            .archive
            .by_name(&path)
            .map_err(|x| LoadClassFileError::OpaqueError(x.into()))?;

        // Read the data out from the file
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(LoadClassFileError::ReadError)?;
        // Rc it, since class file data gets it
        let data = Rc::from(data);

        // TODO: better errors
        let (rem_data, class_file) = class_parser_opt(ParseData::new(&data))
            .map_err(|x| format!("{:?}", x))
            .map_err(LoadClassFileError::ClassFileParseError)?;
        debug_assert!(rem_data.is_empty());

        Ok(Some(ClassFileData::new(class_file_id, data, class_file)))
    }
}
