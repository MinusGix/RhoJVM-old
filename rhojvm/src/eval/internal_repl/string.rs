use crate::{class_instance::ClassInstance, gc::GcRef, jni::JObject, util::Env};

pub(crate) extern "C" fn string_intern(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("String intern's this ref was null");
    let this: GcRef<ClassInstance> = this.unchecked_as();

    let result = env
        .string_interner
        .intern(&mut env.class_names, &env.class_files, &mut env.state, this)
        .unwrap();

    unsafe { env.get_local_jobject_for(result.into_generic()) }
}
