use rhojvm_base::{
    code::method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
    data::class_file_loader::Resource::Buffer,
};

use crate::{
    class_instance::{ClassInstance, Fields},
    eval::{
        eval_method, instances::make_instance_fields, internal_repl::class::get_class_name_id_for,
        EvalMethodValue, Frame, Locals, ValueException,
    },
    initialize_class,
    jni::{JClass, JObject, JString},
    rv::RuntimeValue,
    util::{
        construct_byte_array_input_stream, construct_string_r, construct_url_from_string,
        get_string_contents_as_rust_string, make_class_form_of, ref_info, Env,
    },
};

pub(crate) extern "C" fn system_class_loader_init(env: *mut Env<'_>, _this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let scl_id = env.class_names.gcid_from_bytes(b"rho/SystemClassLoader");

    // We're probably in this already.
    let scl_ref = initialize_class(env, scl_id).unwrap().into_value();
    let scl_ref = match scl_ref {
        ValueException::Value(re) => re,
        ValueException::Exception(_) => {
            todo!("There was an exception when initializing the System ClassLoader class")
        }
    };

    let fields = make_instance_fields(env, scl_id).unwrap();
    let Some(fields) = env.state.extract_value(fields) else {
        todo!("Return Null? Throw an exception?")
    };

    let inst = ClassInstance {
        instanceof: scl_id,
        static_ref: scl_ref,
        fields,
    };

    let inst_ref = env.state.gc.alloc(inst);

    unsafe { env.get_local_jobject_for(inst_ref.into_generic()) }
}

pub(crate) extern "C" fn system_class_loader_load_class(
    env: *mut Env<'_>,
    _: JObject,
    name: JString,
) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let name = unsafe { env.get_jobject_as_gcref(name) };
    let name_ref = name.expect("NPE");

    let name = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        name_ref,
    )
    .unwrap();
    tracing::info!("load_class: {}", name);

    let class_id = get_class_name_id_for(env, name_ref).unwrap();

    // TODO: This is intended to do the implementation in a specific manner, and it should depend on the actual class loader!
    let static_class = initialize_class(env, class_id).unwrap().into_value();
    let Some(_static_class) = env.state.extract_value(static_class) else {
        return JObject::null();
    };

    let sys_cl_id = env.class_names.gcid_from_bytes(b"rho/SystemClassLoader");

    // TODO: The from_class_id should be from the caller
    let class = make_class_form_of(env, sys_cl_id, class_id).unwrap();
    let class = env.state.extract_value(class).unwrap();

    unsafe { env.get_local_jobject_for(class.into_generic()) }
}

pub(crate) extern "C" fn system_class_loader_get_system_resource_as_stream(
    env: *mut Env<'_>,
    _: JClass,
    resource_name: JString,
) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let resource_name = unsafe { env.get_jobject_as_gcref(resource_name) };
    let resource_name = resource_name.expect("null pointer exception");

    let resource_name = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        resource_name,
    )
    .unwrap();

    tracing::info!("get_system_resource_as_stream: {}", resource_name);

    let resource = env
        .class_files
        .loader
        .load_resource(&resource_name)
        .unwrap();
    match resource {
        Buffer(data) => {
            let bai = construct_byte_array_input_stream(env, &data).unwrap();
            if let Some(bai) = env.state.extract_value(bai) {
                unsafe { env.get_local_jobject_for(bai.into_generic()) }
            } else {
                // Exception
                JObject::null()
            }
        }
    }
}

pub(crate) extern "C" fn system_class_loader_get_resource_as_stream(
    env: *mut Env<'_>,
    this: JClass,
    resource_name: JString,
) -> JObject {
    system_class_loader_get_system_resource_as_stream(env, this, resource_name)
}

pub(crate) extern "C" fn system_class_loader_get_resources(
    env: *mut Env<'_>,
    _: JObject,
    name: JString,
) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let name = unsafe { env.get_jobject_as_gcref(name) };
    let resource_name_ref = if let Some(name) = name {
        name
    } else {
        todo!("NPE")
    };
    let resource_name = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        resource_name_ref,
    )
    .unwrap();

    // TODO: Resources with the same name?
    if env.class_files.loader.has_resource(&resource_name) {
        let url = {
            let protocol = env
                .class_files
                .loader
                .resource_protocol(&resource_name)
                .unwrap();
            let url = protocol.format_resource_name(&resource_name);
            let url = construct_string_r(env, &url, false).unwrap();
            let Some(url) = env.state.extract_value(url) else {
            return JObject::null()
        };

            let url = construct_url_from_string(env, url).unwrap();

            let Some(url) = env.state.extract_value(url) else {
            return JObject::null()
        };

            tracing::info!("URL: {}", ref_info(env, Some(url.into_generic())));

            url
        };

        let single_enumeration_id = env
            .class_names
            .gcid_from_bytes(b"rho/util/SingleEnumeration");
        let static_ref = initialize_class(env, single_enumeration_id)
            .unwrap()
            .into_value();
        if let Some(static_ref) = env.state.extract_value(static_ref) {
            let fields = make_instance_fields(env, single_enumeration_id).unwrap();
            let Some(fields) =env.state.extract_value(fields) else {
                todo!("Return Null? Throw an exception?")
            };

            let instance = ClassInstance::new(single_enumeration_id, static_ref, fields);
            let instance_ref = env.state.gc.alloc(instance);

            let descriptor = MethodDescriptor::new(
                smallvec::smallvec![DescriptorType::Basic(DescriptorTypeBasic::Class(
                    env.class_names.object_id()
                ))],
                None,
            );
            let method_id = env
                .methods
                .load_method_from_desc(
                    &mut env.class_names,
                    &mut env.class_files,
                    single_enumeration_id,
                    b"<init>",
                    &descriptor,
                )
                .unwrap();

            let frame = Frame::new_locals(Locals::new_with_array([
                RuntimeValue::Reference(instance_ref.into_generic()),
                RuntimeValue::Reference(url.unchecked_as()),
            ]));
            match eval_method(env, method_id.into(), frame).unwrap() {
                EvalMethodValue::ReturnVoid | EvalMethodValue::Return(_) => unsafe {
                    env.get_local_jobject_for(instance_ref.into_generic())
                },
                EvalMethodValue::Exception(exc) => {
                    env.state.fill_native_exception(exc);
                    JObject::null()
                }
            }
        } else {
            JObject::null()
        }
    } else {
        let empty_enumeration_id = env
            .class_names
            .gcid_from_bytes(b"rho/util/EmptyEnumeration");
        let static_ref = initialize_class(env, empty_enumeration_id)
            .unwrap()
            .into_value();
        if let Some(static_ref) = env.state.extract_value(static_ref) {
            let instance = ClassInstance::new(empty_enumeration_id, static_ref, Fields::default());
            let instance_ref = env.state.gc.alloc(instance);
            unsafe { env.get_local_jobject_for(instance_ref.into_generic()) }
        } else {
            JObject::null()
        }
    }
}

pub(crate) extern "C" fn system_class_loader_get_resource(
    env: *mut Env<'_>,
    _: JObject,
    name: JString,
) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let name = unsafe { env.get_jobject_as_gcref(name) };
    let Some(resource_name_ref) = name else {
        todo!("NPE")
    };

    let resource_name = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        resource_name_ref,
    )
    .unwrap();

    tracing::info!("Get resource: {}", resource_name);

    if env.class_files.loader.has_resource(&resource_name) {
        let protocol = env
            .class_files
            .loader
            .resource_protocol(&resource_name)
            .unwrap();
        let url = protocol.format_resource_name(&resource_name);
        tracing::info!("Constructing url: {:?}", url);
        let url = construct_string_r(env, &url, false).unwrap();
        let Some(url) = env.state.extract_value(url) else {
            tracing::warn!("Failed to construct string for url");
            return JObject::null()
        };

        let url = construct_url_from_string(env, url).unwrap();

        let Some(url) = env.state.extract_value(url) else {
            tracing::info!("Failed to construct url from string");
            return JObject::null()
        };

        tracing::info!("URL: {}", ref_info(env, Some(url.into_generic())));

        unsafe { env.get_local_jobject_for(url.into_generic()) }
    } else {
        tracing::info!("Does not have resource");
        JObject::null()
    }
}
