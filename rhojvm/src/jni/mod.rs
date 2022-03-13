//! Implements JNI functions, structures, and behavior for the `RhoJVM`.
//! This has to implement from the specification, since I found it ambiguous if one could use
//! bindgen on the openjdk `jni.h` files without inheriting the GPL license.

// For now, while many of these are unimplemented,
#![allow(unused_variables)]

use std::ffi::c_void;

use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic},
        types::JavaChar,
    },
    id::{ClassId, ExactMethodId, MethodIndex},
};
use usize_cast::{FromUsize, IntoUsize};

use crate::{
    class_instance::{FieldId, FieldIndex, Instance},
    const_assert,
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

#[repr(i32)]
pub enum Status {
    Ok = 0,
    Err = -1,
    Detached = -2,
    Version = -3,
    NoMemory = -4,
    Exist = -5,
    InvalidArguments = -6,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JFieldId(*const ());
impl JFieldId {
    #[must_use]
    pub fn null() -> JFieldId {
        JFieldId(std::ptr::null())
    }

    pub(crate) fn unchecked_from_i64(id: i64) -> JFieldId {
        let id = id as u64;
        let id = id.into_usize();
        let id = id as *const ();
        JFieldId(id)
    }

    /// This is primarily intended for java apis where we have to return some unique id for a field
    /// as a long.
    pub(crate) fn as_i64(self) -> i64 {
        // This relies on system ptrs being castable to usize and then those being castable to u64
        // -> i64
        u64::from_usize(self.0 as usize) as i64
    }

    /// # Safety
    /// It must be safe to forge pointers and expect to get the original value back.
    pub(crate) unsafe fn new_unchecked(class_id: ClassId, field_index: FieldIndex) -> JFieldId {
        // FIXME: This means that we don't support 32-bit (or, in some world, 16bit) devices at all
        // It would be good to have some fieldid that won't have these issues
        const_assert!(std::mem::size_of::<*const ()>() == 8);
        let class_id_v = class_id.get();
        // These are incremented by 1 so that null is a value that can be represented as a field id
        let class_id_v: u64 = (class_id_v + 1).into();
        let field_index_v: u64 = (field_index.get() + 1).into();
        tracing::info!("Class Id: {:X?}", class_id_v);
        tracing::info!("Field Index {:X?}", field_index_v);

        // [class_id + 1][field_index + 1][0000]
        let field_id = (class_id_v << 32) | (field_index_v << 16);
        let field_id = field_id.into_usize();
        tracing::info!("Field ID: 0x{:X?}", field_id);

        let field_id = field_id as *const ();

        let field_id = JFieldId(field_id);

        debug_assert_eq!(field_id.decompose(), Some((class_id, field_index)));

        field_id
    }

    /// # Safety
    /// This should be a valid [`JFieldId`] handed out by `new_unchecked`
    /// It must be safe to forge pointers and expect to get the original integer back
    pub(crate) unsafe fn into_field_id(self) -> Option<FieldId> {
        if let Some((class_id, field_index)) = self.decompose() {
            Some(FieldId::unchecked_compose(class_id, field_index))
        } else {
            None
        }
    }

    /// # Safety
    /// This should be a valid [`JFieldId`] handed out by `new_unchecked`
    /// It must be safe to forge pointers and expect to get the original integer back
    pub(crate) unsafe fn decompose(self) -> Option<(ClassId, FieldIndex)> {
        let field_id = self.0;
        if field_id.is_null() {
            return None;
        }

        let field_id = field_id as usize;
        #[allow(clippy::cast_possible_truncation)]
        let class_id = ((field_id & 0xFFFF_FFFF_0000_0000) >> 32) as u32;
        let class_id = class_id - 1;
        #[allow(clippy::cast_possible_truncation)]
        let field_index = ((field_id & 0x0000_0000_FFFF_0000) >> 16) as u16;
        let field_index = field_index - 1;

        let class_id = ClassId::new_unchecked(class_id);
        let field_index = FieldIndex::new_unchecked(field_index);

        Some((class_id, field_index))
    }
}

// TODO: This only supports ExactMethodIds
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JMethodId(*const ());
impl JMethodId {
    #[must_use]
    pub fn null() -> JMethodId {
        JMethodId(std::ptr::null())
    }

    pub(crate) fn unchecked_from_i64(id: i64) -> JMethodId {
        let id = id as u64;
        let id = id.into_usize();
        let id = id as *const ();
        JMethodId(id)
    }

    /// This is primarily intended for java apis where we have to return some unique id for a method
    /// as a long.
    pub(crate) fn as_i64(self) -> i64 {
        // This relies on system ptrs being castable to usize and then those being castable to u64
        // -> i64
        u64::from_usize(self.0 as usize) as i64
    }

    /// # Safety
    /// It must be safe to forge pointers and expect to get the original value back.
    pub(crate) unsafe fn new_unchecked(class_id: ClassId, method_index: MethodIndex) -> JMethodId {
        // FIXME: This means that we don't support 32-bit (or, in some world, 16bit) devices at all
        // It would be good to have some methodid that won't have these issues
        const_assert!(std::mem::size_of::<*const ()>() == 8);
        let class_id_v = class_id.get();
        // These are incremented by 1 so that null is a value that can be represented as a field id
        let class_id_v: u64 = (class_id_v + 1).into();
        let field_index_v: u64 = (method_index + 1).into();

        // [class_id + 1][field_index + 1][0000]
        let method_id = (class_id_v << 32) | (field_index_v << 16);
        let method_id = method_id.into_usize();

        let method_id = method_id as *const ();

        let method_id = JMethodId(method_id);

        debug_assert_eq!(method_id.decompose(), Some((class_id, method_index)));

        method_id
    }

    /// # Safety
    /// This should be a valid [`JMethodId`] handed out by `new_unchecked`
    /// It must be safe to forge pointers and expect to get the original integer back
    pub(crate) unsafe fn into_method_id(self) -> Option<ExactMethodId> {
        if let Some((class_id, field_index)) = self.decompose() {
            Some(ExactMethodId::unchecked_compose(class_id, field_index))
        } else {
            None
        }
    }

    /// # Safety
    /// This should be a valid [`JMethodId`] handed out by `new_unchecked`
    /// It must be safe to forge pointers and expect to get the original integer back
    pub(crate) unsafe fn decompose(self) -> Option<(ClassId, MethodIndex)> {
        let method_id = self.0;
        if method_id.is_null() {
            return None;
        }

        let method_id = method_id as usize;
        #[allow(clippy::cast_possible_truncation)]
        let class_id = ((method_id & 0xFFFF_FFFF_0000_0000) >> 32) as u32;
        let class_id = class_id - 1;
        #[allow(clippy::cast_possible_truncation)]
        let method_index = ((method_id & 0x0000_0000_FFFF_0000) >> 16) as u16;
        let method_index = method_index - 1;

        let class_id = ClassId::new_unchecked(class_id);

        Some((class_id, method_index))
    }
}

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
        env: &mut Env<'_>,
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
                    if let Some(gc_ref) = env.get_jobject_as_gcref(self.l) {
                        RuntimeValue::Reference(gc_ref)
                    } else {
                        RuntimeValue::NullReference
                    }
                }
            },
            DescriptorType::Array { .. } => {
                if let Some(gc_ref) = env.get_jobject_as_gcref(self.l) {
                    RuntimeValue::Reference(gc_ref)
                } else {
                    RuntimeValue::NullReference
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

// TODO: We could use phantomdata to make making GcRef's nicer?
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JObject(pub *const ());
impl JObject {
    pub(crate) fn new_unchecked(ptr: *const ()) -> JObject {
        JObject(ptr)
    }

    #[must_use]
    pub fn null() -> JObject {
        JObject(std::ptr::null())
    }
}
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

// TODO: Implement this JVM api.
// We should also make a safe/safer version for Rust consumption
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
