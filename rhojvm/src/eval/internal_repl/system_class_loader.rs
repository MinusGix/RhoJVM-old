use classfile_parser::field_info::FieldAccessFlags;
use either::Either;

use crate::{
    class_instance::ClassInstance,
    eval::{instances::make_fields, ValueException},
    initialize_class,
    jni::JObject,
    util::Env,
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

    let fields = match make_fields(env, scl_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })
    .unwrap()
    {
        Either::Left(fields) => fields,
        Either::Right(_) => {
            todo!("There was an exception when initializing the System ClassLoader's fields")
        }
    };

    let inst = ClassInstance {
        instanceof: scl_id,
        static_ref: scl_ref,
        fields,
    };

    let inst_ref = env.state.gc.alloc(inst);

    unsafe { env.get_local_jobject_for(inst_ref.into_generic()) }
}
