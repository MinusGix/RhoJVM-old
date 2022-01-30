use rhojvm_base::{
    id::MethodId, package::Packages, ClassDirectories, ClassFiles, ClassNames, Classes, Methods,
};

use crate::{GeneralError, State};

#[derive(Debug, Clone)]
pub enum EvalError {
    /// It was expected that this method should be loaded
    /// Likely because it was given to the function to evaluate
    MissingMethod(MethodId),
}

struct Frame {}

/// `method_id` should already be loaded
/// # Errors
/// # Panics
pub fn eval_method(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    method_id: MethodId,
) -> Result<(), GeneralError> {
    let method = methods
        .get_mut(&method_id)
        .ok_or(EvalError::MissingMethod(method_id))?;
    method.load_code(class_files)?;

    // TODO: Handle native methods

    let inst_count = method.code().unwrap().instructions().len();

    Ok(())
}
