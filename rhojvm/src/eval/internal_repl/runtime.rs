use sysinfo::SystemExt;

use crate::{
    jni::{JInt, JLong, JObject},
    util::Env,
};

pub(crate) extern "C" fn runtime_available_processors(env: *mut Env, _this: JObject) -> JInt {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let count = env.system_info.cpus().len();
    i32::try_from(count).unwrap_or(i32::MAX)
}

pub(crate) extern "C" fn runtime_free_memory(env: *mut Env, _this: JObject) -> JLong {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    // Currently we're very aggressive with our memory usage, just using memory as we need it

    let free_memory = env.system_info.free_memory();

    free_memory.try_into().unwrap_or(JLong::MAX)
}

pub(crate) extern "C" fn runtime_total_memory(env: *mut Env, _this: JObject) -> JLong {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    // FIXME: Accurately report our memory usage
    // This is for the entire system, rather than our process.
    let total_memory = env.system_info.used_memory();

    total_memory.try_into().unwrap_or(JLong::MAX)
}

pub(crate) extern "C" fn runtime_max_memory(env: *mut Env, _this: JObject) -> JLong {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    // Currently we're very aggressive with our memory usage, just using memory as we need it
    let max_memory = env.system_info.total_memory();

    max_memory.try_into().unwrap_or(JLong::MAX)
}
