use classfile_parser::{constant_info::ConstantInfo, constant_pool::ConstantPoolIndexRaw};
use rhojvm_base::id::ClassId;

use crate::{rv::RuntimeValue, util::Env, GeneralError};

use super::{func::constant_info_to_rv, EvalError, ValueException};

pub(crate) fn bootstrap_method_arg_to_rv(
    env: &mut Env,
    class_id: ClassId,
    barg_idx: ConstantPoolIndexRaw<ConstantInfo>,
) -> Result<ValueException<RuntimeValue>, GeneralError> {
    let class_file = env
        .class_files
        .get(&class_id)
        .ok_or(EvalError::MissingMethodClassFile(class_id))?;

    // Try to convert the arg into a runtime value
    let barg = class_file.getr(barg_idx)?.clone();
    constant_info_to_rv(env, class_id, &barg)
}
