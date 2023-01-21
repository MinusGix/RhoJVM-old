use crate::{
    class_instance::{Instance, ReferenceInstance},
    eval::class_util::get_init_method_type_from_mh,
    jni::{JClass, JInt, JObject},
    util::Env,
};

pub(crate) extern "C" fn mh_info_get_declaring_class(env: *mut Env, this: JObject) -> JClass {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let _this = this.expect("Null reference");
    todo!()
    // JClass::null()
    // if let Instance::Reference(ReferenceInstance::MethodHandleInfo(mh_info)) =
    //     env.state.gc.deref(this).unwrap()
    // {
    //     // let mh = mh_info.method_handle;
    //     // let mh = env.state.gc.deref(mh).unwrap();
    //     JClass::null()
    // } else {
    //     unreachable!()
    // }
}

pub(crate) extern "C" fn mh_info_get_method_type(env: *mut Env, this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Null reference");

    if let Instance::Reference(ReferenceInstance::MethodHandleInfo(mh_info)) =
        env.state.gc.deref(this).unwrap()
    {
        let mh = mh_info.method_handle;
        let method_type =
            get_init_method_type_from_mh(env, mh).expect("Failed to construct MethodType instance");
        let Some(method_type) = env.state.extract_value(method_type) else {
            // exception
            return JObject::null();
        };

        unsafe { env.get_local_jobject_for(method_type.into_generic()) }
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
