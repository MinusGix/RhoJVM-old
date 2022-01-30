use rhojvm_base::{
    package::Packages, util::MemorySize, ClassDirectories, ClassFiles, ClassNames, Classes, Methods,
};

use crate::State;

/// A struct that holds references to several of the important structures in their typical usage
pub struct PrimaryGroup<'cdir, 'cnam, 'cfil, 'clas, 'pack, 'meth, 'stat> {
    pub class_directories: &'cdir ClassDirectories,
    pub class_names: &'cnam mut ClassNames,
    pub class_files: &'cfil mut ClassFiles,
    pub classes: &'clas mut Classes,
    pub packages: &'pack mut Packages,
    pub methods: &'meth mut Methods,
    pub(crate) state: &'stat mut State,
}

// TODO: A JavaString is obviously not exactly equivalent to a Rust string..
#[derive(Debug, Clone)]
pub struct JavaString(pub String);
impl MemorySize for JavaString {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}
