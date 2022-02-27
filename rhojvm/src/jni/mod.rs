//! Implements JNI functions, structures, and behavior for the `RhoJVM`.
//! This has to implement from the specification, since I found it ambiguous if one could use
//! bindgen on the openjdk `jni.h` files without inheriting the GPL license.

// For now, while many of these are unimplemented,
#![allow(unused_variables)]

use std::ffi::c_void;

use rhojvm_base::code::{
    method::{DescriptorType, DescriptorTypeBasic},
    types::JavaChar,
};

use crate::{
    class_instance::Instance,
    const_assert,
    gc::GcRef,
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::Env,
};

use self::native_interface::NullMethod;

pub mod name;
pub mod native_interface;
pub mod native_lib;

// The JNI types are custom aliases that are not necessarily their C versions
pub type JBoolean = u8;
pub type JByte = i8;
pub type JChar = u16;
pub type JShort = i16;
pub type JInt = i32;
pub type JLong = i64;
pub type JFloat = f32;
pub type JDouble = f64;

pub type JSize = JInt;

// TODO: Give these actually useable types
pub type JFieldId = *const ();
pub type JMethodId = *const ();

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
const_assert!(std::mem::size_of::<JValue>() == 8);
impl JValue {
    /// Convert the value into the value it would be as decided by a [`DescriptorType`]
    /// Note that this does not check if it is a proper instance for a class
    /// # Safety
    /// The type should match what it was originally stored as
    #[must_use]
    pub unsafe fn narrow_from_desc_type_into_value(
        self,
        typ: DescriptorType,
    ) -> RuntimeValue<Instance> {
        match typ {
            DescriptorType::Basic(b) => match b {
                DescriptorTypeBasic::Byte => RuntimeValuePrimitive::I8(self.b).into(),
                DescriptorTypeBasic::Char => RuntimeValuePrimitive::Char(JavaChar(self.c)).into(),
                DescriptorTypeBasic::Double => RuntimeValuePrimitive::F64(self.d).into(),
                DescriptorTypeBasic::Float => RuntimeValuePrimitive::F32(self.f).into(),
                DescriptorTypeBasic::Int => RuntimeValuePrimitive::I32(self.i).into(),
                DescriptorTypeBasic::Long => RuntimeValuePrimitive::I64(self.j).into(),
                DescriptorTypeBasic::Short => RuntimeValuePrimitive::I16(self.s).into(),
                // We treat the any invalid bool values as if they were true
                DescriptorTypeBasic::Boolean => RuntimeValuePrimitive::Bool(self.z != 0).into(),
                DescriptorTypeBasic::Class(_) => {
                    if self.l.is_null() {
                        RuntimeValue::NullReference
                    } else {
                        // TODO: If we keep the pointer around we can actually do a check for if it
                        // is a pointer we gave out, since it can't forge instances!
                        // Safety: All we can really do is assume the pointer is valid
                        RuntimeValue::Reference(*self.l)
                    }
                }
            },
            DescriptorType::Array { .. } => {
                if self.l.is_null() {
                    RuntimeValue::NullReference
                } else {
                    // TODO: If we keep the pointer around we can actually do a check for if it
                    // is a pointer we gave out, since it can't forge instances!
                    // Safety: All we can really do is assume the pointer is valid
                    RuntimeValue::Reference(*self.l)
                }
            }
        }
    }

    /// Convert the value into the value it would be as decided by a [`RuntimeType`]
    /// # Safety
    /// The type should match what it was originally stored as
    /// We assume that the representation of a byte is equivalent to a bool
    #[must_use]
    pub unsafe fn narrow_from_runtime_type_into_value(
        self,
        typ: RuntimeType,
    ) -> Option<RuntimeValue> {
        Some(match typ {
            RuntimeType::Primitive(prim) => match prim {
                RuntimeTypePrimitive::I64 => RuntimeValuePrimitive::I64(self.j).into(),
                RuntimeTypePrimitive::I32 => RuntimeValuePrimitive::I32(self.i).into(),
                RuntimeTypePrimitive::I16 => RuntimeValuePrimitive::I16(self.s).into(),
                // Our code assumes that the representation of a bool and u8 are the same
                RuntimeTypePrimitive::I8 => RuntimeValuePrimitive::I8(self.b).into(),
                RuntimeTypePrimitive::F32 => RuntimeValuePrimitive::F32(self.f).into(),
                RuntimeTypePrimitive::F64 => RuntimeValuePrimitive::F64(self.d).into(),
                RuntimeTypePrimitive::Char => RuntimeValuePrimitive::Char(JavaChar(self.c)).into(),
            },
            RuntimeType::Reference(_) => return None,
        })
    }
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

// TODO: Should we check alignment requirements?
// JNI code expects the JObject as a pointer type, so at the very least it should be the same size
const_assert!(std::mem::size_of::<JObject>() == std::mem::size_of::<*const c_void>());

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

#[repr(C)]
pub struct JVMInterface {
    pub empty_0: NullMethod,
    pub empty_1: NullMethod,
    pub empty_2: NullMethod,

    pub destroy_java_vm: DestroyJVMFn,
    pub attach_current_thread: AttachCurrentThreadFn,
    pub detach_current_thread: DetachCurrentThreadFn,

    pub get_env: GetEnvFn,

    pub attach_current_thread_as_daemon: AttachCurrentThreadAsDaemonFn,
}
impl JVMInterface {
    pub(crate) fn make_typical() -> JVMInterface {
        JVMInterface {
            empty_0: NullMethod::default(),
            empty_1: NullMethod::default(),
            empty_2: NullMethod::default(),
            destroy_java_vm,
            attach_current_thread,
            detach_current_thread,
            get_env,
            attach_current_thread_as_daemon,
        }
    }
}

#[repr(C)]
pub struct JVMData {
    /// This must be the first field
    pub interface: JVMInterface,
}
pub type JVMNoArguments = unsafe extern "C" fn(vm: *mut JVMData);
pub type JVMNoArgumentsRet<R> = unsafe extern "C" fn(vm: *mut JVMData) -> R;
pub type JNIOnLoadFn = extern "C" fn(vm: *mut JVMData, reserved: *const ());

pub type DestroyJVMFn = unsafe extern "C" fn(vm: *mut JVMData) -> JInt;
unsafe extern "C" fn destroy_java_vm(vm: *mut JVMData) -> JInt {
    todo!("destroy_java_vm");
}

#[repr(C)]
pub struct JVMAttachArgs {
    version: JInt,
    name: *mut std::os::raw::c_char,
    group: JObject,
}

pub type AttachCurrentThreadFn =
    unsafe extern "C" fn(vm: *mut JVMData, *mut *mut Env, *mut JVMAttachArgs) -> JInt;
unsafe extern "C" fn attach_current_thread(
    vm: *mut JVMData,
    out_env: *mut *mut Env<'_>,
    thr_args: *mut JVMAttachArgs,
) -> JInt {
    todo!("attach_current_thread");
}

pub type AttachCurrentThreadAsDaemonFn =
    unsafe extern "C" fn(vm: *mut JVMData, *mut *mut Env, *mut JVMAttachArgs) -> JInt;
unsafe extern "C" fn attach_current_thread_as_daemon(
    vm: *mut JVMData,
    out_env: *mut *mut Env<'_>,
    thr_args: *mut JVMAttachArgs,
) -> JInt {
    todo!("attach_current_thread_as_daemon");
}

pub type DetachCurrentThreadFn = unsafe extern "C" fn(vm: *mut JVMData) -> JInt;
unsafe extern "C" fn detach_current_thread(vm: *mut JVMData) -> JInt {
    todo!("detach_current_thread");
}

pub type GetEnvFn =
    unsafe extern "C" fn(vm: *mut JVMData, out_env: *mut *mut Env, version: JInt) -> JInt;
unsafe extern "C" fn get_env(vm: *mut JVMData, out_env: *mut *mut Env, version: JInt) -> JInt {
    todo!("get_env")
}
