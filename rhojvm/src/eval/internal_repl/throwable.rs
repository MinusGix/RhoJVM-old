use rhojvm_base::code::method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor};
use smallvec::smallvec;

use crate::{
    class_instance::ReferenceInstance,
    eval::{eval_method, func::find_virtual_method, EvalMethodValue, Frame, Locals},
    jni::JObject,
    rv::RuntimeValue,
    util::{construct_string_r, Env},
};

pub(crate) extern "C" fn throwable_print_stack_trace(env: *mut Env, _this: JObject, out: JObject) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let out = unsafe { env.get_jobject_as_gcref(out) };
    // TODO: NPE
    let out = out.unwrap();
    let out = out.unchecked_as::<ReferenceInstance>();

    let out_id = env.state.gc.deref(out).unwrap().instanceof();

    let stack_trace = env.pretty_call_stack(false);
    let stack_trace = construct_string_r(env, &stack_trace).unwrap();
    let Some(stack_trace) = env.state.extract_value(stack_trace) else {
        return;
    };

    // Find PrintStream#println(String)
    let print_stream_id = env.class_names.gcid_from_bytes(b"java/io/PrintStream");
    let string_id = env.class_names.gcid_from_bytes(b"java/lang/String");
    let desc = MethodDescriptor::new(
        smallvec![DescriptorType::Basic(DescriptorTypeBasic::Class(string_id)),],
        None,
    );

    let method_id = find_virtual_method(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.methods,
        print_stream_id,
        out_id,
        b"println",
        &desc,
    )
    .unwrap();

    let frame = Frame::new_locals(Locals::new_with_array([
        RuntimeValue::Reference(out.into_generic()),
        RuntimeValue::Reference(stack_trace.into_generic()),
    ]));

    match eval_method(env, method_id.into(), frame).unwrap() {
        EvalMethodValue::ReturnVoid | EvalMethodValue::Return(_) => (),
        EvalMethodValue::Exception(exc) => {
            env.state.fill_native_exception(exc);
        }
    }
}
