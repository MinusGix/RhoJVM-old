//! Implements JNI functions, structures, and behavior for the `RhoJVM`.
//! This has to implement from the specification, since I found it ambiguous if one could use
//! bindgen on the openjdk `jni.h` files without inheriting the GPL license.

// For now, while many of these are unimplemented,
#![allow(unused_variables)]

use crate::{class_instance::Instance, gc::GcRef, util::Env};

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

/// A method for a class that is 'opaque' in that not all of its arguments are known
/// This is primarily for storing, where get the method descriptor but we can't really
/// statically represent the type in a good manner.
/// So, there may be more parameters and/or a return type for this function.
/// It is not allowed to be a null pointer, since it is an `fn`!
#[derive(Clone)]
pub struct OpaqueClassMethod(MethodClassNoArguments);
impl OpaqueClassMethod {
    #[must_use]
    pub fn new(sym: MethodClassNoArguments) -> OpaqueClassMethod {
        OpaqueClassMethod(sym)
    }

    /// Get the held function pointer
    /// Remember, this is an opaque method, so for added **safety** you *must* check the descriptor
    /// for the associated method and cast it to an appropriate version!
    #[must_use]
    pub fn get(&self) -> MethodClassNoArguments {
        self.0
    }
}
impl From<MethodClassNoArguments> for OpaqueClassMethod {
    fn from(method: MethodClassNoArguments) -> OpaqueClassMethod {
        OpaqueClassMethod(method)
    }
}
impl std::fmt::Debug for OpaqueClassMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("OpaqueClassMethod(0x{:X?})", self.0 as usize))
    }
}

/// A JNI function that only takes the environment
pub type MethodNoArguments = unsafe extern "C" fn(env: *mut Env);
/// A JNI function that only takes the environment and the static class it is in
/// For example, `registerNatives` is typically like this
pub type MethodClassNoArguments = unsafe extern "C" fn(env: *mut Env, class: JClass);
