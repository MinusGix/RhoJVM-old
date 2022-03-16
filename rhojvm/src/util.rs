use classfile_parser::field_info::{FieldAccessFlags, FieldInfoOpt};
use either::Either;
use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        types::JavaChar,
    },
    data::{class_files::ClassFiles, class_names::ClassNames, classes::Classes, methods::Methods},
    id::ClassId,
    package::Packages,
    util::MemorySize,
};
use sysinfo::{RefreshKind, SystemExt};

use crate::{
    class_instance::{
        ClassInstance, FieldId, FieldIndex, Instance, PrimitiveArrayInstance, ReferenceInstance,
        StaticClassInstance, StaticFormInstance,
    },
    eval::{
        eval_method, instances::make_fields, EvalError, EvalMethodValue, Frame, Locals,
        ValueException,
    },
    gc::GcRef,
    initialize_class,
    jni::{native_interface::NativeInterface, JObject},
    resolve_derive,
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    string_intern::StringInterner,
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
    pub class_names: ClassNames,
    pub class_files: ClassFiles,
    pub classes: Classes,
    pub packages: Packages,
    pub methods: Methods,
    pub state: State,
    pub tdata: ThreadData,
    pub string_interner: StringInterner,
    pub(crate) system_info: sysinfo::System,
}
impl<'i> Env<'i> {
    pub fn new(
        interface: &'i NativeInterface,
        class_names: ClassNames,
        class_files: ClassFiles,
        classes: Classes,
        packages: Packages,
        methods: Methods,
        state: State,
        tdata: ThreadData,
        string_interner: StringInterner,
    ) -> Env<'i> {
        Env {
            interface,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            tdata,
            string_interner,
            system_info: sysinfo::System::new_with_specifics(
                RefreshKind::new().with_cpu().with_memory(),
            ),
        }
    }

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
        let ptr: usize = re.get_index_unchecked();
        debug_assert_ne!(ptr, std::usize::MAX);
        // We _have_ to add 1 so that nullptr has a different value!
        let ptr: usize = ptr + 1;
        // TODO: is this valid? We know it is non-null and it is a zst so presumably valid
        // everywhere?
        let ptr: *const () = ptr as *const ();

        JObject(ptr)
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
        let ptr: usize = ptr - 1;

        // Sanity/Safety: We can only really assume that what we've been passed in is correct.
        let gc_ref: GcRef<Instance> = GcRef::new_unchecked(ptr);

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

pub(crate) fn signed_offset_32_16(lhs: u16, rhs: i32) -> Option<u16> {
    let lhs = u32::from(lhs);
    if rhs.is_negative() {
        if rhs == i32::MIN {
            None
        } else {
            lhs.checked_sub(rhs.abs() as u32)
        }
    } else {
        // It was not negative so it fits inside a u32
        #[allow(clippy::cast_sign_loss)]
        lhs.checked_add(rhs as u32)
    }
    .map(u16::try_from)
    .and_then(Result::ok)
}

pub(crate) fn get_disjoint2_mut<T>(
    data: &mut [T],
    index1: usize,
    index2: usize,
) -> Option<(&mut T, &mut T)> {
    use std::cmp::Ordering;
    if index1 >= data.len() || index2 >= data.len() {
        return None;
    }

    // It would be nice if rust had this in the standard library..
    let (left, right) = data.split_at_mut(index2);
    let (val1, val2) = match std::cmp::Ord::cmp(&index1, &index2) {
        Ordering::Less => (left.get_mut(index1), right.get_mut(0)),
        // Can't have multiple mutable references to the same data
        Ordering::Equal => return None,
        Ordering::Greater => {
            let (left, right) = right.split_at_mut(index1 - index2);
            (right.get_mut(0), left.get_mut(0))
        }
    };

    let val1 = val1?;
    let val2 = val2?;

    Some((val1, val2))
}

/// Construct an exception with a string
/// It must have a constructor that takes a string
pub(crate) fn make_exception_with_text(
    env: &mut Env,
    class_name: &[u8],
    why: &str,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    let exception_id = env.class_names.gcid_from_bytes(class_name);

    let why = why
        .encode_utf16()
        .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
        .collect();
    let why = construct_string(env, why)?;
    let why = match why {
        ValueException::Value(why) => why,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let constructor_desc = MethodDescriptor::new(
        smallvec::smallvec![DescriptorType::Basic(DescriptorTypeBasic::Class(
            env.class_names.gcid_from_bytes(b"java/lang/String")
        ),)],
        None,
    );

    let method_id = env
        .methods
        .load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            exception_id,
            b"<init>",
            &constructor_desc,
        )
        .unwrap();

    let static_ref = match initialize_class(env, exception_id)?.into_value() {
        ValueException::Value(re) => re,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };
    let fields = match make_fields(env, exception_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })
    .unwrap()
    {
        Either::Left(fields) => fields,
        Either::Right(exc) => {
            return Ok(ValueException::Exception(exc));
        }
    };
    let exception_this_ref = env.state.gc.alloc(ClassInstance {
        instanceof: exception_id,
        static_ref,
        fields,
    });

    let frame = Frame::new_locals(Locals::new_with_array([
        RuntimeValue::Reference(exception_this_ref.into_generic()),
        RuntimeValue::Reference(why.into_generic()),
    ]));

    match eval_method(env, method_id.into(), frame)? {
        EvalMethodValue::ReturnVoid => {}
        EvalMethodValue::Return(_) => tracing::warn!("Constructor returned value"),
        EvalMethodValue::Exception(exc) => return Ok(ValueException::Exception(exc)),
    }

    Ok(ValueException::Value(exception_this_ref))
}

pub(crate) fn find_field_with_name(
    class_files: &ClassFiles,
    class_id: ClassId,
    target_name: &[u8],
) -> Result<Option<(FieldId, FieldInfoOpt)>, GeneralError> {
    let class_file = class_files
        .get(&class_id)
        .ok_or(GeneralError::MissingLoadedClassFile(class_id))?;
    for (i, field_data) in class_file.load_field_values_iter().enumerate() {
        let i = FieldIndex::new_unchecked(i as u16);
        let (field_info, _) = field_data.map_err(GeneralError::ClassFileLoad)?;
        let field_name = class_file.get_text_b(field_info.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(field_info.name_index.into_generic()),
        )?;
        if field_name == target_name {
            return Ok(Some((FieldId::unchecked_compose(class_id, i), field_info)));
        }
    }

    Ok(None)
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

pub(crate) fn to_utf16_arr(text: &str) -> Vec<RuntimeValuePrimitive> {
    text.encode_utf16()
        .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
        .collect()
}

pub(crate) fn construct_string_r(
    env: &mut Env,
    text: &str,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    construct_string(env, to_utf16_arr(text))
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
        &mut env.class_names,
        &mut env.class_files,
        &mut env.methods,
    )?;
    // Evaluate the string constructor
    let string_inst = eval_method(
        env,
        string_char_array_constructor.into(),
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

pub(crate) fn get_string_contents<'a>(
    class_files: &ClassFiles,
    class_names: &mut ClassNames,
    state: &'a mut State,
    string: GcRef<Instance>,
) -> Result<&'a [RuntimeValuePrimitive], GeneralError> {
    let string_id = class_names.gcid_from_bytes(b"java/lang/String");
    let string_content_field = state
        .get_string_data_field(class_files, string_id)
        .expect("getDeclaredField failed to get data field id for string");

    // TODO: Don't unwrap
    let string = match state
        .gc
        .deref(string)
        .ok_or(EvalError::InvalidGcRef(string))
        .unwrap()
    {
        Instance::StaticClass(_) => panic!("Got static class gcref for String"),
        Instance::Reference(v) => match v {
            ReferenceInstance::Class(v) => v,
            _ => panic!("Did not get normal Class gcref for String"),
        },
    };

    // We don't have to verify that name is of the right class because the function calling
    // code would verify that it is being passed a string.
    // but also, String is final

    let data = string
        .fields
        .get(string_content_field)
        .ok_or(EvalError::MissingField(string_content_field))
        .expect("getDeclaredField failed to get data field from string name");

    let data = data.value();
    let data = data
        .into_reference()
        .expect("string data field to be a reference")
        .expect("string data field to be non-null");

    let data = match state.gc.deref(data).unwrap() {
        ReferenceInstance::PrimitiveArray(arr) => arr,
        _ => panic!("Bad type for name text"),
    };
    assert_eq!(data.element_type, RuntimeTypePrimitive::Char);
    Ok(data.elements.as_slice())
}

/// NOTE: This should not be used unless it can't be avoided, or it is only used as a temporary
/// stop-gap, as there is typically more efficient ways of directly using the utf16 string you have!
pub fn get_string_contents_as_rust_string(
    class_files: &ClassFiles,
    class_names: &mut ClassNames,
    state: &mut State,
    string: GcRef<Instance>,
) -> Result<String, GeneralError> {
    let contents = get_string_contents(class_files, class_names, state, string)?;
    // Converting back to cesu8 is expensive, but this kind of operation isn't common enough to do
    // anything like storing cesu8 versions alongside them, probably.
    let contents = contents
        .iter()
        .map(|x| x.into_char().unwrap().0)
        .collect::<Vec<u16>>();
    String::from_utf16(&contents).map_err(GeneralError::StringConversionFailure)
}

pub(crate) fn make_class_form_of(
    env: &mut Env,
    from_class_id: ClassId,
    of_class_id: ClassId,
) -> Result<ValueException<GcRef<StaticFormInstance>>, GeneralError> {
    resolve_derive(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
        &mut env.methods,
        &mut env.state,
        of_class_id,
        from_class_id,
    )?;

    // // TODO: Some of these errors should be exceptions
    // let static_ref = initialize_class(env, of_class_id)?.into_value();
    // let static_ref = match static_ref {
    //     ValueException::Value(v) => v,
    //     ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    // };

    let mut class_info = env.state.classes_info.get_mut_init(of_class_id);
    // If it already exists, then return it, so that we don't recreate instances of Class<T>
    // because they should be the same instance.
    // We could have some trickery with equals to make them equivalent, but caching it is also
    // just less work in general.
    if let Some(form_ref) = class_info.class_ref {
        return Ok(ValueException::Value(form_ref));
    }

    let class_form_id = env.class_names.gcid_from_bytes(b"java/lang/Class");

    // TODO: Some of these errors should be exceptions
    resolve_derive(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
        &mut env.methods,
        &mut env.state,
        class_form_id,
        from_class_id,
    )?;

    // TODO: Some of these errors should be exceptions
    let class_form_ref = initialize_class(env, class_form_id)?.into_value();
    let class_form_ref = match class_form_ref {
        ValueException::Value(v) => v,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let mut fields = match make_fields(env, class_form_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })? {
        Either::Left(fields) => fields,
        Either::Right(exc) => {
            return Ok(ValueException::Exception(exc));
        }
    };

    // This is the classId field, which is the only field in Rho's java/lang/Class
    let class_id_field_id = env
        .state
        .get_class_class_id_field(&env.class_files, class_form_id)?;

    let class_id_field = fields
        .get_mut(class_id_field_id)
        .ok_or(EvalError::MissingField(class_id_field_id))?;
    *class_id_field.value_mut() = RuntimeValuePrimitive::I32(of_class_id.get() as i32).into();

    // new does not run a constructor, it only initializes it
    let inner_class = ClassInstance {
        instanceof: class_form_id,
        static_ref: class_form_ref,
        fields,
    };

    let static_form = StaticFormInstance::new(inner_class, of_class_id, None);
    let static_form_ref = env.state.gc.alloc(static_form);

    // Store the created form on the static inst so that it can be reused and cached
    let mut class_info = env.state.classes_info.get_mut_init(of_class_id);
    debug_assert!(class_info.class_ref.is_none(), "If this is false then we've initialized it in between our check, which could be an issue? Though it also seems completely possible.");
    class_info.class_ref = Some(static_form_ref);
    Ok(ValueException::Value(static_form_ref))
}

#[cfg(test)]
mod tests {
    use crate::util::get_disjoint2_mut;

    #[test]
    fn test_get_disjoint() {
        let mut data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        assert_eq!(get_disjoint2_mut(&mut data, 0, 0), None);
        assert_eq!(get_disjoint2_mut(&mut data, 1, 1), None);
        assert_eq!(get_disjoint2_mut(&mut data, 50, 50), None);

        assert_eq!(get_disjoint2_mut(&mut data, 0, 1), Some((&mut 0, &mut 1)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 2), Some((&mut 0, &mut 2)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 3), Some((&mut 0, &mut 3)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 4), Some((&mut 0, &mut 4)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 5), Some((&mut 0, &mut 5)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 6), Some((&mut 0, &mut 6)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 7), Some((&mut 0, &mut 7)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 8), Some((&mut 0, &mut 8)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 9), Some((&mut 0, &mut 9)));
        assert_eq!(get_disjoint2_mut(&mut data, 0, 10), Some((&mut 0, &mut 10)));

        assert_eq!(get_disjoint2_mut(&mut data, 5, 10), Some((&mut 5, &mut 10)));

        assert_eq!(get_disjoint2_mut(&mut data, 8, 0), Some((&mut 8, &mut 0)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 1), Some((&mut 8, &mut 1)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 2), Some((&mut 8, &mut 2)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 3), Some((&mut 8, &mut 3)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 4), Some((&mut 8, &mut 4)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 5), Some((&mut 8, &mut 5)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 6), Some((&mut 8, &mut 6)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 7), Some((&mut 8, &mut 7)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 8), None);
        assert_eq!(get_disjoint2_mut(&mut data, 8, 9), Some((&mut 8, &mut 9)));
        assert_eq!(get_disjoint2_mut(&mut data, 8, 10), Some((&mut 8, &mut 10)));

        assert_eq!(get_disjoint2_mut(&mut data, 12, 0), None);
    }
}
