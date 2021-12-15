use rhojvm_base::{code::stack_map::StackMapFrames, id::MethodId, ProgramInfo, StepError};

use crate::{GeneralError, State};

pub(crate) fn verify_type_safe_method_stack_map(
    prog: &mut ProgramInfo,
    state: &mut State,
    method_id: MethodId,
) -> Result<(), GeneralError> {
    let (class_id, _) = method_id.decompose();
    prog.class_files
        .load_by_class_path_id(&prog.class_directories, &mut prog.class_names, class_id)
        .map_err(StepError::from)?;
    prog.load_method_from_id(method_id)?;
    prog.load_method_code(method_id)?;

    let class_file = prog.class_files.get(&class_id).unwrap();
    let method = prog.methods.get(&method_id).unwrap();

    let stack_frames = if let Some(stack_frames) =
        StackMapFrames::parse_frames(&mut prog.class_names, class_file, method)?
    {
        stack_frames
    } else {
        // If there were no stack frames then there is no need to verify them
        return Ok(());
    };

    // TODO: Verify max stack size from code
    // TODO: Verify max stack size from state
    // TODO: If we are verifying max stack size usage above, then it would be nice if the stack map
    // frame parsing let us do some checks for each iteration of it, so that we could produce
    // an error without parsing everything.

    Ok(())
}
