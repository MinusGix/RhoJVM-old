use crate::{
    class_instance::{Instance, ReferenceInstance},
    eval::ValueException,
    jni::{JInt, JObject},
    util::{self, Env},
};

pub(crate) extern "C" fn object_get_class(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.unwrap();

    let this = env.state.gc.deref(this).unwrap();
    let id = match this {
        Instance::StaticClass(_) => panic!("Should not be static class"),
        Instance::Reference(re) => re.instanceof(),
    };

    let class_form = util::make_class_form_of(env, id, id).unwrap();
    let class_form = match class_form {
        ValueException::Value(class_form) => class_form,
        ValueException::Exception(_) => todo!("There was an exception in Object#getClass"),
    };

    unsafe { env.get_local_jobject_for(class_form.into_generic()) }
}

pub(crate) extern "C" fn object_hashcode(env: *mut Env<'_>, this: JObject) -> JInt {
    // Hashcode impls require that if they're equal then they have the same hashcode
    // So that means the users must override the hashocde if they modify equals
    // And so, since this is for Object, and object's equal is a strict reference equality, we
    // just use the gc index as the value.

    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    if let Some(this) = this {
        let index = this.get_index_unchecked();
        // TODO: Is this fine? It is iffy on 64 bit platforms...
        (index as u32) as i32
    } else {
        // Can this even occur?
        todo!("Null pointer exception")
    }
}

pub(crate) extern "C" fn object_clone(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    if let Some(this) = this {
        let cloned = env.state.gc.shallow_clone(this);
        if let Some(cloned) = cloned {
            unsafe { env.get_local_jobject_for(cloned) }
        } else {
            todo!("NPE")
        }
    } else {
        todo!("NPE")
    }
}

pub(crate) extern "C" fn object_notify_all(env: *mut Env, this: JObject) {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) }.unwrap();
    let _this = this.unchecked_as::<ReferenceInstance>();

    tracing::warn!("Object#notifyAll is not implemented");
}
