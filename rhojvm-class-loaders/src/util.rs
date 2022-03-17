use rhojvm_base::{
    class::ClassFileData,
    data::{
        class_file_loader::{ClassFileLoader, LoadClassFileError, LoadResourceError, Resource},
        class_names::ClassNames,
    },
    id::ClassId,
};

/// A loader that simply checks the 'left' variant for the class
/// If it doesn't exist there, it checks the right variant
/// It tries to be a bit smart about what error it returns if it isn't found
pub struct CombineLoader<L: ClassFileLoader, R: ClassFileLoader> {
    pub left: L,
    pub right: R,
}
impl<L: ClassFileLoader, R: ClassFileLoader> CombineLoader<L, R> {
    pub fn new(left: L, right: R) -> CombineLoader<L, R> {
        CombineLoader { left, right }
    }
}

impl<L: ClassFileLoader, R: ClassFileLoader> ClassFileLoader for CombineLoader<L, R> {
    fn load_class_file_by_id(
        &mut self,
        class_names: &ClassNames,
        class_file_id: ClassId,
    ) -> Result<Option<ClassFileData>, LoadClassFileError> {
        match self.left.load_class_file_by_id(class_names, class_file_id) {
            Ok(data) => {
                // We assume if it returned `None` it had a good reason
                Ok(data)
            }
            Err(first_err) => {
                match first_err {
                    // We don't ignore these errors, they're indicative that there was some more
                    // notable bug
                    LoadClassFileError::EmptyPath
                    | LoadClassFileError::ReadError(_)
                    | LoadClassFileError::ClassFileParseError(_)
                    | LoadClassFileError::BadId(_) => Err(first_err),
                    _ => match self.right.load_class_file_by_id(class_names, class_file_id) {
                        Ok(data) => Ok(data),
                        Err(right_err) => match right_err {
                            LoadClassFileError::EmptyPath
                            | LoadClassFileError::ReadError(_)
                            | LoadClassFileError::ClassFileParseError(_)
                            | LoadClassFileError::BadId(_) => Err(right_err),
                            _ => Err(first_err),
                        },
                    },
                }
            }
        }
    }

    fn load_resource(&mut self, resource_name: &str) -> Result<Resource, LoadResourceError> {
        match self.left.load_resource(resource_name) {
            Ok(resource) => Ok(resource),
            Err(left_err) => match left_err {
                LoadResourceError::ReadError(_) => Err(left_err),
                _ => match self.right.load_resource(resource_name) {
                    Ok(resource) => Ok(resource),
                    Err(right_err) => match right_err {
                        LoadResourceError::ReadError(_) => Err(right_err),
                        _ => Err(left_err),
                    },
                },
            },
        }
    }
}
