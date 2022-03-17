use rhojvm_base::id::MethodId;

use crate::{
    jni::JClass,
    util::{make_class_form_of, Env},
};

pub(crate) extern "C" fn get_caller_class(env: *mut Env<'_>, _: JClass) -> JClass {
    assert!(!env.is_null(), "Null env. Internal bug?");
    let env = unsafe { &mut *env };

    // The topmost cstack would be for this method getting called
    // then the next cstack would be for the method that called this
    // method getting called

    if let Some(entry) = env.call_stack.iter().rev().nth(1) {
        match entry.called_from {
            MethodId::Exact(called_from_method_id) => {
                let (called_from_id, _) = called_from_method_id.decompose();
                // TODO: Bad usage of make_class_form_of
                let form = make_class_form_of(env, called_from_id, called_from_id).unwrap();
                if let Some(form) = env.state.extract_value(form) {
                    unsafe { env.get_local_jobject_for(form.into_generic()) }
                } else {
                    // Exception occurred
                    JClass::null()
                }
            }
            // TODO: We don't keep track of the id that the array is for so we can't make a Class<T>
            // instance just from this...
            MethodId::ArrayClone => todo!(),
        }
    } else {
        tracing::warn!("There was no entry before getCallerClass, which is odd");
        JClass::null()
    }
}
