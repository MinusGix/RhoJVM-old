use crate::{
    class_instance::{Instance, ReferenceInstance},
    jni::{JClass, JInt, JObject},
    util::Env,
};

pub(crate) extern "C" fn mh_info_get_declaring_class(env: *mut Env, this: JObject) -> JClass {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Null reference");
    if let Instance::Reference(ReferenceInstance::MethodHandleInfo(mh_info)) =
        env.state.gc.deref(this).unwrap()
    {
        // let mh = mh_info.method_handle;
        // let mh = env.state.gc.deref(mh).unwrap();
        todo!()
    } else {
        unreachable!()
    }
}

pub(crate) extern "C" fn mh_info_get_reference_kind(env: *mut Env, this: JObject) -> JInt {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Null reference");
    if let Instance::Reference(ReferenceInstance::MethodHandleInfo(mh_info)) =
        env.state.gc.deref(this).unwrap()
    {
        let mh = mh_info.method_handle;
        let mh = env.state.gc.deref(mh).unwrap();
        mh.typ.kind().into()
    } else {
        unreachable!()
    }
}
