//! Implements JNI functions, structures, and behavior for the RhoJVM.
//! This has to implement from the specification, since I found it ambiguous if one could use
//! bindgen on the openjdk `jni.h` files without inheriting the GPL license.

// For now, while many of these are unimplemented,
#![allow(unused_variables)]

use crate::{
    class_instance::{Instance, ReferenceInstance},
    gc::GcRef,
};

use self::native_interface::JNINativeInterface;

pub mod name;
pub mod native_interface;
pub mod native_lib;

// The JNI types are custom aliases that are not necessarily their C versions
pub type JBoolean = u8;
pub type JByte = i8;
pub type JChar = u16;
pub type JShort = i16;
pub type JInt = i32;
pub type JLong = i32;
pub type JFloat = f32;
pub type JDouble = f64;

pub type JSize = JInt;

#[repr(C)]
pub union JValue {
    pub z: JBoolean,
    pub b: JByte,
    pub c: JChar,
    pub s: JShort,
    pub i: JInt,
    pub j: JLong,
    pub f: JFloat,
    pub d: JDouble,
    pub l: JObject,
}

pub type JObject = *const GcRef<Instance>;
// Note: All of these must be able to be treated as the same type as JObject
pub type JClass = JObject;
pub type JString = JObject;
pub type JArray = JObject;
pub type JObjectArray = JObject;
pub type JBooleanArray = JObject;
pub type JByteArray = JObject;
pub type JCharArray = JObject;
pub type JShortArray = JObject;
pub type JIntArray = JObject;
pub type JLongArray = JObject;
pub type JFloatArray = JObject;
pub type JDoubleArray = JObject;
pub type JThrowable = JObject;

/// A `JNIEnv*` is passed into most functions
/// The first field _must_ be a pointer to some `JNINativeInterface`
/// It can be assumed to only be accessed from one thread
#[repr(C)]
pub struct JNIEnv<'a> {
    pub interface: &'a JNINativeInterface,
}
impl<'a> JNIEnv<'a> {
    /// Construct a JNI environment with the given native interface virtual table
    #[must_use]
    pub fn new(interface: &'a JNINativeInterface) -> JNIEnv<'a> {
        JNIEnv { interface }
    }
}

/// A JNI function that only takes the environment
pub type MethodNoArguments = unsafe extern "C" fn(env: *mut JNIEnv);
/// A JNI function that only takes the environment and the static class it is in
/// For example, `registerNatives` is typically like this
pub type MethodClassNoArguments = unsafe extern "C" fn(env: *mut JNIEnv, class: JClass);
