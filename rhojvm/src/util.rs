use classfile_parser::field_info::FieldAccessFlags;
use either::Either;
use rhojvm_base::{
    package::Packages, util::MemorySize, ClassDirectories, ClassFiles, ClassNames, Classes, Methods,
};

use crate::{
    class_instance::{ClassInstance, Instance, PrimitiveArrayInstance, StaticClassInstance},
    eval::{
        eval_method, instances::make_fields, EvalError, EvalMethodValue, Frame, Locals,
        ValueException,
    },
    gc::GcRef,
    initialize_class,
    jni::{native_interface::NativeInterface, JObject},
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    BegunStatus, GeneralError, State, ThreadData,
};

/// Note: This is internal to rhojvm
#[macro_export]
macro_rules! const_assert {
    ($x:expr $(,)?) => {
        const _: () = assert!($x);
    };
}

/// A struct that holds references to several of the important structures in their typical usage
/// This is repr-C because it needs to be able to be passed to native functions
#[repr(C)]
pub struct Env<'i> {
    // Interface MUST be the first field so that it is the first field in the jni
    pub interface: &'i NativeInterface,
    pub class_directories: ClassDirectories,
    pub class_names: ClassNames,
    pub class_files: ClassFiles,
    pub classes: Classes,
    pub packages: Packages,
    pub methods: Methods,
    pub(crate) state: State,
    pub(crate) tdata: ThreadData,
}
impl<'i> Env<'i> {
    pub(crate) fn get_empty_string(
        &mut self,
    ) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
        if let Some(empty_string_ref) = self.state.empty_string_ref {
            return Ok(ValueException::Value(empty_string_ref));
        }

        // This code feels kinda hacky, but I'm unsure of a great way past it
        // The issue primarily is that String relies on itself for all of its constructors
        // Even its default constructor, which ldcs an empty string and gets its char array
        // then uses that reference as its own char array (because it is a clone of the empty
        // string)
        // TODO: Is there a better way?

        let string_ref = match alloc_string(self)? {
            ValueException::Value(string_ref) => string_ref,
            ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
        };

        // We try to find the correct field to modify based on types, and we have to modify since
        // an array is initialized to null, not an empty array
        // This should harden us a bit against different implementations
        // And we only do this once, so the extra calculation time is insignificant
        let char_array_id = self.state.char_array_id(&mut self.class_names);
        let empty_array =
            PrimitiveArrayInstance::new(char_array_id, RuntimeTypePrimitive::Char, Vec::new());
        let empty_array_ref = self.state.gc.alloc(empty_array);

        let string = self
            .state
            .gc
            .deref_mut(string_ref)
            .ok_or(EvalError::InvalidGcRef(string_ref.into_generic()))?;

        let mut found_storage_field = false;
        for (_field_name, field) in string.fields.iter_mut() {
            if field.typ() == RuntimeType::Reference(char_array_id) {
                *field.value_mut() = RuntimeValue::Reference(empty_array_ref.into_generic());
                found_storage_field = true;
                break;
            }
        }

        if !found_storage_field {
            return Err(GeneralError::StringNoValueField);
        }

        self.state.empty_string_ref = Some(string_ref);

        Ok(ValueException::Value(string_ref))
    }

    #[allow(clippy::unused_self)]
    /// Get a [`JObject`] instance for a specific [`GcRef`].
    /// Note that this is a local [`JObject`] so it can become invalid!
    /// # Safety
    /// The platform must support casting `usize` to a pointer and back losslessly
    pub(crate) unsafe fn get_local_jobject_for(&mut self, re: GcRef<Instance>) -> JObject {
        const_assert!(std::mem::size_of::<usize>() == std::mem::size_of::<*const ()>());
        // TODO: Mark it down as being stored? Well, I think they're shortlived by default?
        let ptr = re.get_index_unchecked();
        debug_assert_ne!(ptr, std::usize::MAX);
        // We _have_ to add 1 so that nullptr has a different value!
        let ptr = ptr + 1;
        // TODO: is this valid? We know it is non-null and it is a zst so presumably valid
        // everywhere?
        let ptr: *const () = ptr as *const ();
        let ptr = JObject(ptr);

        ptr
    }

    #[allow(clippy::unused_self)]
    /// Get a [`GcRef`] from a [`JObject`].
    /// Note that it may not be valid anymore, since they can keep them around however long they
    /// want, or just straight up forge them.
    /// Returns `None` if it was null
    /// # Safety
    /// The [`JObject`] should be valid and not refer to an object that has changed.
    /// The platform must support casting `usize` to a pointer and back losslessly
    pub(crate) unsafe fn get_jobject_as_gcref(&mut self, re: JObject) -> Option<GcRef<Instance>> {
        let ptr: *const () = re.0;
        if ptr.is_null() {
            return None;
        }

        let ptr: usize = ptr as usize;
        // It can't be null now, but we check that it is nonzero anyway
        // TODO: Do we have a guarantee that being null means that converting it to a usize
        // will produce 0?
        if ptr == 0 {
            return None;
        }

        // Shift down by 1 since `get_local_jobject_for` shifted it up by 1
        let ptr = ptr - 1;

        // Sanity/Safety: We can only really assume that what we've been passed in is correct.
        let gc_ref = GcRef::new_unchecked(ptr);

        Some(gc_ref)
    }
}

// TODO: A JavaString is obviously not exactly equivalent to a Rust string..
#[derive(Debug, Clone)]
pub struct JavaString(pub Vec<u16>);
impl MemorySize for JavaString {
    fn memory_size(&self) -> usize {
        self.0.memory_size()
    }
}

pub(crate) const fn signed_offset_16(lhs: u16, rhs: i16) -> Option<u16> {
    if rhs.is_negative() {
        if rhs == i16::MIN {
            None
        } else {
            lhs.checked_sub(rhs.abs() as u16)
        }
    } else {
        // It was not negative so it fits inside a u16
        #[allow(clippy::cast_sign_loss)]
        lhs.checked_add(rhs as u16)
    }
}

/// Gets a reference to the string class, initializing it if it doesn't exist
pub(crate) fn get_string_ref(
    env: &mut Env,
) -> Result<ValueException<GcRef<StaticClassInstance>>, GeneralError> {
    // Initialize the string class if it somehow isn't already ready
    let string_id = env.state.string_class_id(&mut env.class_names);
    initialize_class(env, string_id).map(BegunStatus::into_value)
}

pub(crate) fn alloc_string(
    env: &mut Env,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    let string_id = env.state.string_class_id(&mut env.class_names);
    let string_ref = get_string_ref(env)?;
    // Allocate the uninitialized instance
    let string_ref = match string_ref {
        ValueException::Value(string_ref) => string_ref,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let fields = match make_fields(env, string_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })? {
        Either::Left(fields) => fields,
        Either::Right(exc) => {
            return Ok(ValueException::Exception(exc));
        }
    };

    // new does not run a constructor, it only initializes it
    let instance = ClassInstance::new(string_id, string_ref, fields);

    Ok(ValueException::Value(env.state.gc.alloc(instance)))
}

/// Construct a JVM String given some string
/// Note that `utf16_text` should be completely `RuntimeValuePrimitive::Char`
pub(crate) fn construct_string(
    env: &mut Env,
    utf16_text: Vec<RuntimeValuePrimitive>,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    // Create a char[] in utf16
    let char_arr_ref = {
        let char_arr_id = env.state.char_array_id(&mut env.class_names);
        let char_arr =
            PrimitiveArrayInstance::new(char_arr_id, RuntimeTypePrimitive::Char, utf16_text);
        env.state.gc.alloc(char_arr)
    };

    let string_ref = match alloc_string(env)? {
        ValueException::Value(string_ref) => string_ref,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    // Get the string constructor that would take a char[], bool
    let string_char_array_constructor = env.state.get_string_char_array_constructor(
        &env.class_directories,
        &mut env.class_names,
        &mut env.class_files,
        &mut env.methods,
    )?;
    // Evaluate the string constructor
    let string_inst = eval_method(
        env,
        string_char_array_constructor,
        Frame::new_locals(Locals::new_with_array([
            RuntimeValue::Reference(string_ref.into_generic()),
            RuntimeValue::Reference(char_arr_ref.into_generic()),
            RuntimeValue::Primitive(RuntimeValuePrimitive::Bool(true)),
        ])),
    )?;

    match string_inst {
        // We expect it to return nothing
        EvalMethodValue::ReturnVoid => (),
        // Returning something is weird..
        EvalMethodValue::Return(v) => {
            tracing::warn!("String constructor returned {:?}, ignoring..", v);
        }
        // If there was an exception, we simply pass it along
        EvalMethodValue::Exception(exc) => return Ok(ValueException::Exception(exc)),
    }

    Ok(ValueException::Value(string_ref))
}
