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

    let code = if let Some(code) = method.code() {
        code
    } else {
        // We tried loading the code but there wasn't any.
        // Thus, there is no stack map to validate
        return Ok(());
    };

    let stack_frames = if let Some(stack_frames) =
        StackMapFrames::parse_frames(&mut prog.class_names, class_file, method.descriptor(), code)?
    {
        stack_frames
    } else {
        // If there were no stack frames then there is no need to verify them
        // This is because the types can be inferred easily, such as in a function
        // without control flow
        return Ok(());
    };

    // TODO: Verify max stack size from code
    // TODO: Verify max stack size from state
    // TODO: If we are verifying max stack size usage above, then it would be nice if the stack map
    // frame parsing let us do some checks for each iteration of it, so that we could produce
    // an error without parsing everything.

    // We don't bother with the merging of stack map and code, since we can get the associated stack
    // map relatively easily, and so there doesn't seem to be much point pairing them together
    // especially since, if we have some types associated with instructions, they'll be more
    // in-detail ones for analysis, rather than the relatively basic ones that stack maps supply
    // (See: mergeStackMapAndCode jvm ch. 4)

    Ok(())
}
