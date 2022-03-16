use crate::{
    jni::{JClass, JObject},
    util::Env,
};

pub(crate) extern "C" fn thread_get_current_thread(env: *mut Env<'_>, _: JClass) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let thread = env
        .tdata
        .thread_instance
        .expect("Environment did not have an initialized thread instance!");

    unsafe { env.get_local_jobject_for(thread.into_generic()) }
}
