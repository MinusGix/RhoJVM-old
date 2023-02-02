use crate::{
    jni::JObject,
    util::{get_string_contents_as_rust_string, Env},
};

pub(crate) extern "C" fn info(env: *mut Env<'_>, _this: JObject, msg: JObject) {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let msg = unsafe { env.get_jobject_as_gcref(msg) };
    let msg = msg.expect("NPE");

    let msg = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        msg,
    )
    .unwrap();

    tracing::info!("{}", msg);
}

pub(crate) extern "C" fn warn(env: *mut Env<'_>, _this: JObject, msg: JObject) {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let msg = unsafe { env.get_jobject_as_gcref(msg) };
    let msg = msg.expect("NPE");

    let msg = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        msg,
    )
    .unwrap();

    tracing::warn!("{}", msg);
}

pub(crate) extern "C" fn error(env: *mut Env<'_>, _this: JObject, msg: JObject) {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let msg = unsafe { env.get_jobject_as_gcref(msg) };
    let msg = msg.expect("NPE");

    let msg = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        msg,
    )
    .unwrap();

    tracing::error!("{}", msg);
}
