use rhojvm_base::code::method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor};

use crate::{
    class_instance::{ClassInstance, ThreadLocalData},
    eval::{eval_method, func::find_virtual_method, EvalMethodValue, Frame, Locals},
    gc::GcRef,
    jni::JObject,
    rv::RuntimeValue,
    util::{ref_info, Env},
};

pub(crate) extern "C" fn thread_local_get(env: *mut Env<'_>, this_ref: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this_ref = unsafe { env.get_jobject_as_gcref(this_ref) }.unwrap();
    let this_ref = this_ref.unchecked_as::<ClassInstance>();

    let thread_ref = env.tdata.thread_instance.expect("Thread instance not set");

    let thread = env.state.gc.deref(thread_ref).unwrap();

    if !thread.thread_locals.contains_key(&this_ref) {
        let data = make_initial_thread_local_data(env, this_ref);
        let thread = env.state.gc.deref_mut(thread_ref).unwrap();
        thread.thread_locals.insert(this_ref, data);
    }

    let thread = env.state.gc.deref(thread_ref).unwrap();
    let data = thread.thread_locals.get(&this_ref).unwrap();
    if let Some(value) = data.value {
        unsafe { env.get_local_jobject_for(value.into_generic()) }
    } else {
        JObject::null()
    }
}

fn make_initial_thread_local_data(
    env: &mut Env<'_>,
    this_ref: GcRef<ClassInstance>,
) -> ThreadLocalData {
    let thread_local_id = env.class_names.gcid_from_bytes(b"java/lang/ThreadLocal");
    let this_id = env.state.gc.deref(this_ref).unwrap().instanceof;

    let object_id = env.class_names.object_id();
    let desc =
        MethodDescriptor::new_ret(DescriptorType::Basic(DescriptorTypeBasic::Class(object_id)));

    let method_id = find_virtual_method(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.methods,
        thread_local_id,
        this_id,
        b"initialValue",
        &desc,
    )
    .unwrap();

    let frame = Frame::new_locals(Locals::new_with_array([RuntimeValue::Reference(
        this_ref.into_generic(),
    )]));
    let value = eval_method(env, method_id, frame).unwrap();
    let value = match value {
        EvalMethodValue::ReturnVoid => panic!("ThreadLocal::initialValue returned void"),
        EvalMethodValue::Return(value) => value,
        EvalMethodValue::Exception(exc) => {
            panic!(
                "Exception thrown in ThreadLocal::initialValue: {:?}",
                ref_info(env, exc)
            )
        }
    };
    let value = value.into_reference().unwrap();

    ThreadLocalData { value }
}

pub(crate) extern "C" fn thread_local_set(env: *mut Env<'_>, this: JObject, value: JObject) {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this_ref = unsafe { env.get_jobject_as_gcref(this) }.unwrap();
    let this_ref = this_ref.unchecked_as::<ClassInstance>();

    let value_ref = unsafe { env.get_jobject_as_gcref(value) };
    let value_ref = value_ref.map(|re| env.state.gc.checked_as(re).unwrap());

    let thread_ref = env.tdata.thread_instance.expect("Thread instance not set");

    let thread = env.state.gc.deref_mut(thread_ref).unwrap();

    let thread_data = thread
        .thread_locals
        .entry(this_ref)
        .or_insert_with(|| ThreadLocalData { value: None });
    thread_data.value = value_ref;
}

pub(crate) extern "C" fn thread_local_remove(env: *mut Env<'_>, this: JObject) {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this_ref = unsafe { env.get_jobject_as_gcref(this) }.unwrap();
    let this_ref = this_ref.unchecked_as::<ClassInstance>();

    let thread_ref = env.tdata.thread_instance.expect("Thread instance not set");

    let thread = env.state.gc.deref_mut(thread_ref).unwrap();

    thread.thread_locals.remove(&this_ref);
}
