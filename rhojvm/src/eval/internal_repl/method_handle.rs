use crate::{
    class_instance::{ClassInstance, Fields, MethodHandleInfoInstance, MethodHandleInstance},
    gc::GcRef,
    initialize_class,
    jni::{JObject, JObjectArray},
    resolve_derive,
    util::Env,
};

// Note: Variadics in java are just transformed into arrays, which makes this significantly easier
// to implement than if we had to manually pop things from the frame as we need them.
pub(crate) extern "C" fn method_handle_invoke(
    env: *mut Env,
    handle: JObject,
    args: JObjectArray,
) -> JObject {
    todo!()
}

pub(crate) extern "C" fn mh_lookup_reveal_direct(
    env: *mut Env,
    lookup: JObject,
    target: JObject,
) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let target: GcRef<_> = unsafe { env.get_jobject_as_gcref(target) }.expect("target was null");
    assert!(env.state.gc.deref(target).is_some());
    let target: GcRef<MethodHandleInstance> = target.unchecked_as();
    assert!(env.state.gc.deref(target).is_some());

    // TODO: Throw exception if target is not a direct method handle
    // TODO: exception if access checking fails
    // TODO: exception if security manager refuses

    let mh_info_id = env
        .class_names
        .gcid_from_bytes(b"rho/invoke/MethodHandleInfoInst");

    // TODO: Deriving from itself is bad
    resolve_derive(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
        &mut env.methods,
        &mut env.state,
        mh_info_id,
        mh_info_id,
    )
    .unwrap();

    let mh_info_static_ref = initialize_class(env, mh_info_id).unwrap().into_value();
    let mh_info_static_ref = if let Some(re) = env.state.extract_value(mh_info_static_ref) {
        re
    } else {
        // exception
        return JObject::null();
    };

    // We assume that there are no fields to initialize
    let class_instance = ClassInstance::new(mh_info_id, mh_info_static_ref, Fields::default());
    let mh_info_instance = MethodHandleInfoInstance::new(class_instance, target);

    let mh_info_ref = env.state.gc.alloc(mh_info_instance);

    unsafe { env.get_local_jobject_for(mh_info_ref.into_generic()) }
}
