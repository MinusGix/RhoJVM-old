use rhojvm_base::code::method::MethodDescriptor;

use crate::{
    class_instance::{
        ClassInstance, Fields, MethodHandleInfoInstance, MethodHandleInstance, MethodHandleType,
        StaticFormInstance,
    },
    eval::internal_repl::method_type::{method_type_to_desc_string, MethodTypeWrapper},
    gc::GcRef,
    initialize_class,
    jni::{JClass, JObject},
    resolve_derive,
    rv::{RuntimeType, RuntimeTypeVoid},
    util::{construct_method_handle, get_string_contents_as_rust_string, make_class_form_of, Env},
};

pub(crate) extern "C" fn mh_lookup_reveal_direct(
    env: *mut Env,
    _lookup: JObject,
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
    resolve_derive(env, mh_info_id, mh_info_id).unwrap();

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

pub(crate) extern "C" fn mhs_lookup_lookup_class(env: *mut Env, _this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    if env.call_stack.len() < 2 {
        panic!("MethodHandles.Lookup#lookupClass called from outside of a method");
    }

    let cstack_entry = &env.call_stack[env.call_stack.len() - 2];
    let Some((caller_class_id, _)) = cstack_entry.called_from.decompose() else {
        panic!("MethodHandles.Lookup#lookupClass called from non-normal method");
    };

    let mhl_class_id = env
        .class_names
        .gcid_from_bytes(b"java/lang/invoke/MethodHandles$Lookup");

    let class_inst = make_class_form_of(env, mhl_class_id, caller_class_id).unwrap();
    let Some(class_inst) = env.state.extract_value(class_inst) else {
        // exception
        return JClass::null();
    };

    unsafe { env.get_local_jobject_for(class_inst.into_generic()) }
}

pub(crate) unsafe extern "C" fn mhs_lookup_find_static(
    env: *mut Env,
    _this: JObject,
    target: JObject,
    name: JObject,
    method_type: JObject,
) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let target: GcRef<_> = unsafe { env.get_jobject_as_gcref(target) }.unwrap();
    let target: GcRef<StaticFormInstance> = target.unchecked_as();
    let target_class_id = env
        .state
        .gc
        .deref(target)
        .unwrap()
        .of
        .into_reference()
        .unwrap();

    let name: GcRef<_> = unsafe { env.get_jobject_as_gcref(name) }.unwrap();
    let name = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        name,
    )
    .unwrap();

    let method_type = unsafe { env.get_jobject_as_gcref(method_type) }.unwrap();
    let method_type = method_type.unchecked_as();
    // TODO: We could more directly convert it to a MethodDescriptor
    let desc = method_type_to_desc_string(
        &mut env.class_names,
        &env.class_files,
        &env.state.gc,
        method_type,
    );
    println!(
        "findStatic: {:?}; desc: {} on {}",
        name,
        desc,
        env.class_names.tpath(target_class_id)
    );

    let desc = MethodDescriptor::from_text(desc.as_bytes(), &mut env.class_names).unwrap();

    // TODO: check that the class and arg types are accessible to the lookup inst
    let method_id = env
        .methods
        .load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            target_class_id,
            // TODO: CESU8
            name.as_bytes(),
            &desc,
        )
        .unwrap();

    // TODO: variable arity flag?
    let method_handle =
        construct_method_handle(env, MethodHandleType::InvokeStatic(method_id)).unwrap();
    let Some(method_handle) = env.state.extract_value(method_handle) else {
        // exception
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(method_handle.into_generic()) }
}

/// `MethodHandle constant(Class<?> type, Object value)`
pub(crate) extern "C" fn mhs_constant(
    env: *mut Env<'_>,
    _this: JClass,
    typ: JObject,
    value: JObject,
) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let typ_ref = unsafe { env.get_jobject_as_gcref(typ) }.expect("type was null");
    let typ_ref: GcRef<StaticFormInstance> = env.state.gc.checked_as(typ_ref).unwrap();

    let value = unsafe { env.get_jobject_as_gcref(value) };
    let value = value.map(GcRef::unchecked_as);

    let typ_of = env.state.gc.deref(typ_ref).unwrap().of;
    let typ_of = match typ_of {
        RuntimeTypeVoid::Primitive(_) => todo!("We need to actually verify that the class is a primitive wrapper"),
        RuntimeTypeVoid::Void => todo!("IllegalArgumentException. Cannot create a constant-returning function with return type void"),
        RuntimeTypeVoid::Reference(class_id) => {
            // TODO: Check that the value we have actually extends the class_id
            RuntimeType::Reference(class_id)
        },
    };

    let mh = construct_method_handle(
        env,
        MethodHandleType::Constant {
            value,
            return_ty: typ_of,
        },
    )
    .unwrap();
    let Some(mh) = env.state.extract_value(mh) else {
        // exception
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(mh.into_generic()) }
}
