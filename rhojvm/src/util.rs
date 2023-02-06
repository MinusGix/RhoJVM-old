use std::{borrow::Cow, num::NonZeroUsize};

use classfile_parser::{
    attribute_info::InstructionIndex,
    constant_info::{ConstantInfo, MethodHandleConstant},
    constant_pool::ConstantPoolIndexRaw,
    field_info::FieldInfoOpt,
};
use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        types::{JavaChar, PrimitiveType},
    },
    data::{
        class_file_loader::LoadClassFileError,
        class_files::ClassFiles,
        class_names::ClassNames,
        classes::{does_extend_class, Classes},
        methods::Methods,
    },
    id::{ClassId, MethodId},
    package::Packages,
    util::MemorySize,
    StepError,
};
use smallvec::{smallvec, SmallVec, ToSmallVec};
use sysinfo::{CpuRefreshKind, RefreshKind, SystemExt};

use crate::{
    class_instance::{
        ClassInstance, FieldId, FieldIndex, Fields, Instance, MethodHandleInstance,
        MethodHandleType, PrimitiveArrayInstance, ReferenceInstance, StaticClassInstance,
        StaticFormInstance,
    },
    eval::{
        eval_method, func::find_virtual_method, instances::make_instance_fields,
        internal_repl::method_type::method_type_to_desc_string, EvalError, EvalMethodValue, Frame,
        Locals, ValueException,
    },
    gc::GcRef,
    initialize_class,
    jni::{native_interface::NativeInterface, JObject},
    resolve_derive,
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeTypeVoid, RuntimeValue, RuntimeValuePrimitive},
    string_intern::StringInterner,
    BegunStatus, GeneralError, ResolveError, State, ThreadData,
};

/// Note: This is internal to rhojvm
#[macro_export]
macro_rules! const_assert {
    ($x:expr $(,)?) => {
        const _: () = assert!($x);
    };
}

/// A macro intended to make extracting a value from a `ValueException` easier.  
#[macro_export]
macro_rules! exc_value {
    (ret inst: $v:expr) => {
        match $v {
            ValueException::Value(x) => x,
            ValueException::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
        }
    };
    (ret: $v:expr) => {
        match $v {
            ValueException::Value(x) => x,
            ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
        }
    };
}

/// A macro intended to make extracting a value from a `EvalMethodValue` easier.
#[macro_export]
macro_rules! exc_eval_value {
    (ret (expect_void: ) ($name:expr): $v:expr) => {
        match $v {
            EvalMethodValue::ReturnVoid => (),
            EvalMethodValue::Exception(exc) => return Ok(ValueException::Exception(exc)),
            EvalMethodValue::Return(_) => {
                unreachable!("Return value from function ({:?}), expected void", $name)
            }
        }
    };
    (ret inst (expect_return: ) ($name:expr): $v:expr) => {
        match $v {
            EvalMethodValue::Return(x) => x,
            EvalMethodValue::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
            EvalMethodValue::ReturnVoid => {
                unreachable!("No (void) return value from function ({:?})", $name)
            }
        }
    };
    (ret inst (expect_return: reference) ($name:expr): $v:expr) => {
        match $v {
            EvalMethodValue::Return(x) => x.into_reference().unwrap_or_else(|| {
                panic!(
                    "Bad return value from function ({:?}), expected reference",
                    $name
                )
            }),
            EvalMethodValue::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
            EvalMethodValue::ReturnVoid => unreachable!(
                "No (void) return value from function ({:?}), expected reference",
                $name
            ),
        }
    };
}

#[derive(Debug, Clone)]
pub struct CallStackEntry {
    /// The method that was called
    pub called_method: MethodId,
    /// The method id of the method that called it, typically this should be the previous entry in
    /// the callstack
    pub called_from: MethodId,
    /// The instruction index of inside the method that called it, if there was one
    /// If we were to do something like running an instruction by itself, then this would
    /// be `None`.
    pub called_at: Option<InstructionIndex>,
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
    /// Keeps track of methods being executed and what instruction they were executed at
    pub call_stack: Vec<CallStackEntry>,
    pub(crate) system_info: sysinfo::System,
    /// Keep track of the time we started up (approximately)
    /// Primarily for `System#nanoTime`/`System#currentTimeMillis`
    pub(crate) startup_instant: std::time::Instant,
    pub(crate) skip_logging: bool,
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
            call_stack: Vec::with_capacity(128),
            system_info: sysinfo::System::new_with_specifics(
                RefreshKind::new()
                    .with_cpu(CpuRefreshKind::everything())
                    .with_memory(),
            ),
            startup_instant: std::time::Instant::now(),
            skip_logging: false,
        }
    }

    pub fn pretty_call_stack(&self, include_inst_idx: bool) -> String {
        let mut result = String::new();

        for (i, entry) in self.call_stack.iter().rev().enumerate() {
            let CallStackEntry {
                called_method,
                called_at,
                ..
            } = entry;

            let (class_name, method_name) = match *called_method {
                MethodId::Exact(method_id) => {
                    let class_id = method_id.decompose().0;
                    let class_name = self.class_names.tpath(class_id);
                    let called_method_name =
                        if let Some(called_method) = self.methods.get(&method_id) {
                            let name_idx = called_method.name_index();
                            let class_file = self.class_files.get(&class_id).unwrap();

                            class_file.getr_text(name_idx).unwrap()
                        } else {
                            Cow::Borrowed("<unknown method>")
                        };
                    (class_name, called_method_name)
                }
                MethodId::ArrayClone => {
                    let class_name = "<Internal Array>";
                    let method_name = "clone";

                    (class_name, Cow::Borrowed(method_name))
                }
            };

            result.push_str("  at ");
            result.push_str(class_name);
            result.push('.');
            result.push_str(method_name.as_ref());
            // TODO: include source file name and line number info

            if include_inst_idx {
                if let Some(inst_idx) = called_at {
                    result.push_str(" (#");
                    result.push_str(&inst_idx.0.to_string());
                    result.push(')');
                }
            }

            if i != self.call_stack.len() - 1 {
                result.push('\n');
            }
        }

        result
    }

    /// Get the latest (on the call stack) class id.  
    /// (This only really makes sense to use when you're not going to be called manually from Rust)
    pub fn get_calling_class_id(&self) -> Option<ClassId> {
        self.call_stack
            .last()
            .and_then(|x| x.called_from.decompose())
            .map(|(id, _)| id)
    }

    /// Get the second latest (on the call stack) class id. (Used for some methods that are wrapped)
    /// (This only really makes sense to use when you're not going to be called manually from Rust)
    pub fn get2_calling_class_id(&self) -> Option<ClassId> {
        self.call_stack.len().checked_sub(2).and_then(|x| {
            self.call_stack
                .get(x)
                .and_then(|x| x.called_from.decompose())
                .map(|(id, _)| id)
        })
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
pub(crate) fn make_exception_by_name(
    env: &mut Env,
    class_name: &[u8],
    why: &str,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    let exception_id = env.class_names.gcid_from_bytes(class_name);

    make_exception(env, exception_id, why)
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

    let fields = make_instance_fields(env, string_id)?;
    let fields = exc_value!(ret: fields);

    // new does not run a constructor, it only initializes it
    let instance = ClassInstance::new(string_id, string_ref, fields);

    Ok(ValueException::Value(env.state.gc.alloc(instance)))
}

pub(crate) fn to_utf16_arr(text: &str) -> Vec<RuntimeValuePrimitive> {
    text.encode_utf16()
        .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
        .collect()
}

/// Construct a JVM String given some Rust utf8 string
pub(crate) fn construct_string_r(
    env: &mut Env,
    text: &str,
    should_intern: bool,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    construct_string(env, to_utf16_arr(text), should_intern)
}

/// Construct a JVM String given some string
/// Note that `utf16_text` should be completely `RuntimeValuePrimitive::Char`
pub(crate) fn construct_string(
    env: &mut Env,
    utf16_text: Vec<RuntimeValuePrimitive>,
    should_intern: bool,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    if let Some(inst) = env.string_interner.get_by_data(&env.state.gc, &utf16_text) {
        return Ok(ValueException::Value(inst));
    }

    // Create a char[] in utf16
    let char_arr_ref = {
        let char_arr_id = env.state.char_array_id(&mut env.class_names);
        let char_arr =
            PrimitiveArrayInstance::new(char_arr_id, RuntimeTypePrimitive::Char, utf16_text);
        env.state.gc.alloc(char_arr)
    };

    let string_id = env.state.string_class_id(&mut env.class_names);

    let string_ref = match alloc_string(env)? {
        ValueException::Value(string_ref) => string_ref,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let string_data_field_id = env
        .state
        .get_string_data_field(&env.class_files, string_id)?;

    let string = env
        .state
        .gc
        .deref_mut(string_ref)
        .ok_or(EvalError::InvalidGcRef(string_ref.into_generic()))?;

    *(string
        .fields
        .get_mut(string_data_field_id)
        .unwrap()
        .value_mut()) = RuntimeValue::Reference(char_arr_ref.into_generic());

    if should_intern {
        env.string_interner.intern(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.state,
            string_ref,
        )?;
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

pub(crate) fn construct_url_from_string(
    env: &mut Env,
    text: GcRef<ClassInstance>,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    let string_id = env.class_names.gcid_from_bytes(b"java/lang/String");
    let desc = MethodDescriptor::new(
        smallvec![DescriptorType::Basic(DescriptorTypeBasic::Class(string_id))],
        None,
    );

    let url_id = env.class_names.gcid_from_bytes(b"java/net/URL");

    let url_static_ref = initialize_class(env, url_id)?.into_value();
    let url_static_ref = exc_value!(ret: url_static_ref);

    let method_id = env.methods.load_method_from_desc(
        &mut env.class_names,
        &mut env.class_files,
        url_id,
        b"<init>",
        &desc,
    )?;

    let fields = make_instance_fields(env, url_id)?;
    let fields = exc_value!(ret: fields);

    let inst = ClassInstance::new(url_id, url_static_ref, fields);
    let inst_ref = env.state.gc.alloc(inst);

    let frame = Frame::new_locals(Locals::new_with_array([
        RuntimeValue::Reference(inst_ref.into_generic()),
        RuntimeValue::Reference(text.into_generic()),
    ]));

    let res = eval_method(env, method_id.into(), frame)?;
    exc_eval_value!(ret (expect_void: ) ("URL constructor"): res);

    Ok(ValueException::Value(inst_ref))
}

pub(crate) fn state_target_primitive_field(
    state: &mut State,
    typ: Option<RuntimeTypePrimitive>,
) -> &mut Option<GcRef<StaticFormInstance>> {
    match typ {
        Some(prim) => match prim {
            RuntimeTypePrimitive::I64 => &mut state.long_static_form,
            RuntimeTypePrimitive::I32 => &mut state.int_static_form,
            RuntimeTypePrimitive::I16 => &mut state.short_static_form,
            RuntimeTypePrimitive::I8 => &mut state.byte_static_form,
            RuntimeTypePrimitive::Bool => &mut state.bool_static_form,
            RuntimeTypePrimitive::F32 => &mut state.float_static_form,
            RuntimeTypePrimitive::F64 => &mut state.double_static_form,
            RuntimeTypePrimitive::Char => &mut state.char_static_form,
        },
        None => &mut state.void_static_form,
    }
}

/// Make a Class<T> for a primitive type, the value being cached
/// `None` represents void
pub(crate) fn make_primitive_class_form_of(
    env: &mut Env,
    primitive: Option<RuntimeTypePrimitive>,
) -> Result<ValueException<GcRef<StaticFormInstance>>, GeneralError> {
    let rv: RuntimeTypeVoid<ClassId> = primitive
        .map(RuntimeTypeVoid::from)
        .unwrap_or(RuntimeTypeVoid::Void);

    if let Some(re) = *state_target_primitive_field(&mut env.state, primitive) {
        return Ok(ValueException::Value(re));
    }

    let class_form_id = env.class_names.gcid_from_bytes(b"java/lang/Class");

    // Where are we resolving it from?
    // resolve_derive(
    //     &mut env.class_names,
    //     &mut env.class_files,
    //     &mut env.classes,
    //     &mut env.packages,
    //     &mut env.methods,
    //     &mut env.state,
    //     class_form_id,
    //     from_class_id,
    // )?;

    let class_form_ref = initialize_class(env, class_form_id)?.into_value();
    let class_form_ref = match class_form_ref {
        ValueException::Value(v) => v,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let fields = make_instance_fields(env, class_form_id)?;
    let fields = exc_value!(ret: fields);

    // new does not run a constructor, it only initializes it
    let inner_class = ClassInstance {
        instanceof: class_form_id,
        static_ref: class_form_ref,
        fields,
    };

    let static_form = StaticFormInstance::new(inner_class, rv, None);
    let static_form_ref = env.state.gc.alloc(static_form);

    let storage_field = state_target_primitive_field(&mut env.state, primitive);
    if let Some(re) = *storage_field {
        // We somehow loaded it while we were loading it. That's a potential cause
        // for circularity bugs if it occurs, but if we got here then we presumably
        // managed to avoid it
        // However, since we already got it, we just return the stored reference
        // since that is the valid one.
        Ok(ValueException::Value(re))
    } else {
        // Otherwise cache it
        *storage_field = Some(static_form_ref);
        Ok(ValueException::Value(static_form_ref))
    }
}

pub(crate) fn make_class_form_of(
    env: &mut Env,
    from_class_id: ClassId,
    of_class_id: ClassId,
) -> Result<ValueException<GcRef<StaticFormInstance>>, GeneralError> {
    resolve_derive(env, of_class_id, from_class_id)?;

    // // TODO: Some of these errors should be exceptions
    // let static_ref = initialize_class(env, of_class_id)?.into_value();
    // let static_ref = match static_ref {
    //     ValueException::Value(v) => v,
    //     ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    // };

    let class_info = env.state.classes_info.get_mut_init(of_class_id);
    // If it already exists, then return it, so that we don't recreate instances of Class<T>
    // because they should be the same instance.
    // We could have some trickery with equals to make them equivalent, but caching it is also
    // just less work in general.
    if let Some(form_ref) = class_info.class_ref {
        return Ok(ValueException::Value(form_ref));
    }

    let class_form_id = env.class_names.gcid_from_bytes(b"java/lang/Class");

    // TODO: Some of these errors should be exceptions
    resolve_derive(env, class_form_id, from_class_id)?;

    // TODO: Some of these errors should be exceptions
    let class_form_ref = initialize_class(env, class_form_id)?.into_value();
    let class_form_ref = match class_form_ref {
        ValueException::Value(v) => v,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let fields = make_instance_fields(env, class_form_id)?;
    let fields = exc_value!(ret: fields);

    // new does not run a constructor, it only initializes it
    let inner_class = ClassInstance {
        instanceof: class_form_id,
        static_ref: class_form_ref,
        fields,
    };

    let static_form =
        StaticFormInstance::new(inner_class, RuntimeType::Reference(of_class_id), None);
    let static_form_ref = env.state.gc.alloc(static_form);

    // Store the created form on the static inst so that it can be reused and cached
    let mut class_info = env.state.classes_info.get_mut_init(of_class_id);
    debug_assert!(class_info.class_ref.is_none(), "If this is false then we've initialized it in between our check, which could be an issue? Though it also seems completely possible.");
    class_info.class_ref = Some(static_form_ref);
    Ok(ValueException::Value(static_form_ref))
}

/// Convert the runtimevalue into object form.  
/// Primtives are boxed into their respective classes.
pub(crate) fn rv_into_object(
    env: &mut Env,
    rv: RuntimeValue,
) -> Result<ValueException<Option<GcRef<ReferenceInstance>>>, GeneralError> {
    match rv {
        RuntimeValue::Primitive(p) => {
            let class_id = match p {
                RuntimeValuePrimitive::I64(_) => env.class_names.gcid_from_bytes(b"java/lang/Long"),
                RuntimeValuePrimitive::I32(_) => {
                    env.class_names.gcid_from_bytes(b"java/lang/Integer")
                }
                RuntimeValuePrimitive::I16(_) => {
                    env.class_names.gcid_from_bytes(b"java/lang/Short")
                }
                RuntimeValuePrimitive::I8(_) => env.class_names.gcid_from_bytes(b"java/lang/Byte"),
                RuntimeValuePrimitive::F64(_) => {
                    env.class_names.gcid_from_bytes(b"java/lang/Double")
                }
                RuntimeValuePrimitive::F32(_) => {
                    env.class_names.gcid_from_bytes(b"java/lang/Float")
                }
                RuntimeValuePrimitive::Bool(_) => {
                    env.class_names.gcid_from_bytes(b"java/lang/Boolean")
                }
                RuntimeValuePrimitive::Char(_) => {
                    env.class_names.gcid_from_bytes(b"java/lang/Character")
                }
            };

            let static_ref = initialize_class(env, class_id)?.into_value();
            let static_ref = exc_value!(ret: static_ref);

            // Get the method
            let descriptor = MethodDescriptor::new(
                smallvec![
                    DescriptorType::Basic(DescriptorTypeBasic::Class(class_id)),
                    DescriptorType::Basic(p.runtime_type().to_desc_type_basic()),
                ],
                None,
            );
            let constructor_id = env.methods.load_method_from_desc(
                &mut env.class_names,
                &mut env.class_files,
                class_id,
                b"<init>",
                &descriptor,
            )?;

            // Create the instance
            let fields = exc_value!(ret: make_instance_fields(env, class_id)?);
            let instance = ClassInstance::new(class_id, static_ref, fields);

            let instance_ref = env.state.gc.alloc(instance);

            // Call the method
            let locals =
                Locals::new_with_array([RuntimeValue::Reference(instance_ref.into_generic()), rv]);
            let frame = Frame::new_locals(locals);

            let res = eval_method(env, constructor_id.into(), frame)?;
            exc_eval_value!(ret (expect_void: )("primitive constructor"): res);

            Ok(ValueException::Value(Some(instance_ref.into_generic())))
        }
        RuntimeValue::NullReference => Ok(ValueException::Value(None)),
        RuntimeValue::Reference(r) => Ok(ValueException::Value(Some(r))),
    }
}

pub(crate) fn make_method_handle(
    env: &mut Env,
    class_id: ClassId,
    MethodHandleConstant {
        reference_kind,
        reference_index,
    }: &MethodHandleConstant,
) -> Result<ValueException<GcRef<MethodHandleInstance>>, GeneralError> {
    #[allow(clippy::match_same_arms)]
    match reference_kind {
        // getField
        1 => todo!(),
        // getStatic
        2 => todo!(),
        // putField
        3 => todo!(),
        // putStatic
        4 => todo!(),
        // invokeVirtual
        5 => todo!(),
        // invokeStatic
        6 => make_invoke_static_method_handle(env, class_id, *reference_index),
        // invokeSpecial
        7 => todo!(),
        // newInvokeSpecial
        8 => todo!(),
        // invokeInterface
        9 => todo!(),
        _ => panic!("Unknown MethodHandle reference kind: {}", reference_kind),
    }
}

pub(crate) fn make_invoke_static_method_handle(
    env: &mut Env,
    class_id: ClassId,
    reference_index: ConstantPoolIndexRaw<ConstantInfo>,
) -> Result<ValueException<GcRef<MethodHandleInstance>>, GeneralError> {
    let class_file = env
        .class_files
        .get(&class_id)
        .ok_or(EvalError::MissingMethodClassFile(class_id))?;

    let reference_value = class_file
        .get_t(reference_index)
        .ok_or(EvalError::InvalidConstantPoolIndex(reference_index))?;

    let (nat_index, class_index) = match reference_value {
        ConstantInfo::MethodRef(method) => (method.name_and_type_index, method.class_index),
        ConstantInfo::InterfaceMethodRef(method) => {
            if let Some(version) = class_file.version() {
                if version.major < 52 {
                    panic!("InterfaceMethodRef found for pre version 52 class file")
                }
            }

            (method.name_and_type_index, method.class_index)
        }
        _ => panic!("MethodHandle constant info reference index was not a method ref"),
    };

    let class = class_file.getr(class_index)?;
    // Get the name of the class/interface the method is on
    let target_class_id = {
        let target_class_name = class_file.getr_text_b(class.name_index)?;
        env.class_names.gcid_from_bytes(target_class_name)
    };
    // Get the name of the method and the descriptor of it
    let (target_method_name, target_desc) = {
        let nat = class_file.getr(nat_index)?;
        let target_method_name = class_file.getr_text_b(nat.name_index)?;

        let target_desc = {
            let target_desc = class_file.getr_text_b(nat.descriptor_index)?;
            MethodDescriptor::from_text(target_desc, &mut env.class_names)
                .map_err(EvalError::InvalidMethodDescriptor)?
        };

        (target_method_name, target_desc)
    };

    // Unfortunately we have to collect since we need mutable access to class_files
    let target_method_name: SmallVec<[_; 16]> = target_method_name.to_smallvec();
    // Find the method
    let target_method_id = env.methods.load_method_from_desc(
        &mut env.class_names,
        &mut env.class_files,
        target_class_id,
        &target_method_name,
        &target_desc,
    )?;

    let method_handle =
        construct_method_handle(env, MethodHandleType::InvokeStatic(target_method_id))?;

    Ok(method_handle)
}

pub(crate) fn construct_method_handle(
    env: &mut Env<'_>,
    typ: MethodHandleType,
) -> Result<ValueException<GcRef<MethodHandleInstance>>, GeneralError> {
    // rho's method handle extends the abstract method handle class and is what method handle
    // instance is of
    let mh_id = env
        .class_names
        .gcid_from_bytes(b"rho/invoke/MethodHandleInst");
    // TODO: Deriving from itself is bad
    resolve_derive(env, mh_id, mh_id)?;

    let mh_static_ref = initialize_class(env, mh_id)?.into_value();
    let mh_static_ref = match mh_static_ref {
        ValueException::Value(re) => re,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    // We assume that there are no fields to initialize
    let class_instance = ClassInstance::new(mh_id, mh_static_ref, Fields::default());
    let mh_instance = MethodHandleInstance::new(class_instance, typ);

    Ok(ValueException::Value(env.state.gc.alloc(mh_instance)))
}

/// Create an instance of `java/io/ByteArrayInputStream` holding the given data
pub(crate) fn construct_byte_array_input_stream(
    env: &mut Env,
    data: &[u8],
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    let bai_id = env
        .class_names
        .gcid_from_bytes(b"java/io/ByteArrayInputStream");
    resolve_derive(env, bai_id, bai_id)?;

    let byte_array_id = env
        .class_names
        .gcid_from_array_of_primitives(PrimitiveType::Byte);

    let bai_static_ref = initialize_class(env, bai_id)?.into_value();
    let bai_static_ref = match bai_static_ref {
        ValueException::Value(re) => re,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let data = data
        .iter()
        .map(|x| RuntimeValuePrimitive::I8(*x as i8))
        .collect::<Vec<_>>();

    let array = PrimitiveArrayInstance::new(byte_array_id, RuntimeTypePrimitive::I8, data);
    let array_ref = env.state.gc.alloc(array);

    let fields = make_instance_fields(env, bai_id)?;
    let fields = exc_value!(ret: fields);

    let bai = ClassInstance::new(bai_id, bai_static_ref, fields);
    let bai_ref = env.state.gc.alloc(bai);

    // TODO: Cache this
    let descriptor = MethodDescriptor::new(
        smallvec::smallvec![DescriptorType::Array {
            level: NonZeroUsize::new(1).unwrap(),
            component: DescriptorTypeBasic::Byte
        }],
        None,
    );

    let constructor_id = env.methods.load_method_from_desc(
        &mut env.class_names,
        &mut env.class_files,
        bai_id,
        b"<init>",
        &descriptor,
    )?;

    let locals = Locals::new_with_array([
        RuntimeValue::Reference(bai_ref.into_generic()),
        RuntimeValue::Reference(array_ref.into_generic()),
    ]);
    let frame = Frame::new_locals(locals);

    match eval_method(env, constructor_id.into(), frame)? {
        EvalMethodValue::ReturnVoid | EvalMethodValue::Return(_) => {}
        EvalMethodValue::Exception(exc) => return Ok(ValueException::Exception(exc)),
    }

    Ok(ValueException::Value(bai_ref))
}

/// Construct an instance of an exception.  
/// Note that it will throw an error or exception itself if the class does not have a cons
/// from a `java/lang/String`.  
/// ```rust,ignore
/// let exception_id: ClassId = env.class_names.gcid_from_bytes(b"java/lang/ClassNotFoundException");
/// let exc: ValueException<GcRef<ClassInstance>> = make_exception(env, exception_id, "Failed to find the Toast class")?;
/// // Get the intended exception if its a value, otherwise get the exception that was thrown
/// // in creating the exception.
/// let exc: GcRef<ClassInstance> = exc.flatten();
/// ```
pub(crate) fn make_exception(
    env: &mut Env,
    exception_id: ClassId,
    message: &str,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    // TODO: If there was an exception in construction the exception, we should note that in the exception. Probably via a wrapper? Then if it errors again, we panic.
    let exception_static_ref = initialize_class(env, exception_id)?.into_value();
    let exception_static_ref = match exception_static_ref {
        ValueException::Value(re) => re,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let fields = make_instance_fields(env, exception_id)?;
    let fields = exc_value!(ret: fields);

    let exception = ClassInstance::new(exception_id, exception_static_ref, fields);
    let exception_ref = env.state.gc.alloc(exception);

    let string_id = env.class_names.gcid_from_bytes(b"java/lang/String");

    let descriptor = MethodDescriptor::new(
        smallvec::smallvec![DescriptorType::Basic(DescriptorTypeBasic::Class(string_id))],
        None,
    );

    let constructor_id = env.methods.load_method_from_desc(
        &mut env.class_names,
        &mut env.class_files,
        exception_id,
        b"<init>",
        &descriptor,
    )?;

    let message = construct_string_r(env, message, false)?;
    let message = exc_value!(ret: message);

    let locals = Locals::new_with_array([
        RuntimeValue::Reference(exception_ref.into_generic()),
        RuntimeValue::Reference(message.into_generic()),
    ]);
    let frame = Frame::new_locals(locals);

    match eval_method(env, constructor_id.into(), frame)? {
        EvalMethodValue::ReturnVoid | EvalMethodValue::Return(_) => {}
        EvalMethodValue::Exception(exc) => return Ok(ValueException::Exception(exc)),
    }

    Ok(ValueException::Value(exception_ref))
}

pub(crate) fn make_err_into_class_not_found_exception<T, E: Into<GeneralError>>(
    env: &mut Env,
    res: Result<T, E>,
    class_id: ClassId,
) -> Result<ValueException<T>, GeneralError> {
    let res = res.map_err(E::into);
    if matches!(
        res,
        Err(GeneralError::Step(StepError::LoadClassFile(
            LoadClassFileError::Nonexistent | LoadClassFileError::NonexistentFile(_)
        )) | GeneralError::Resolve(ResolveError::InaccessibleClass { .. }))
    ) {
        let class_not_found_id = env
            .class_names
            .gcid_from_bytes(b"java/lang/ClassNotFoundException");
        let exc = make_exception(
            env,
            class_not_found_id,
            &format!("Failed to find {}", env.class_names.tpath(class_id)),
        )?;
        let exc = exc.flatten();
        return Ok(ValueException::Exception(exc));
    }

    res.map(ValueException::Value)
}

pub(crate) fn mh_info(env: &mut Env, re: GcRef<MethodHandleInstance>) -> String {
    let method = env.state.gc.deref(re).unwrap();
    match method.typ {
        MethodHandleType::Constant { value, return_ty } => {
            format!(
                "MethodHandle(Constant(Value: {}, Return As: {:?}))",
                ref_info(env, value.map(GcRef::unchecked_as)),
                return_ty
            )
        }
        MethodHandleType::InvokeStatic(id) => {
            let Some(method) = env.methods.get(&id) else {
                return "MethodHandle(InvokeStatic(<Bad MethodId>))".to_string();
            };

            let class_id = method.id().decompose().0;
            let desc = method.descriptor();
            let name = env
                .class_files
                .get(&class_id)
                .unwrap()
                .get_text_t(method.name_index())
                .unwrap();

            format!(
                "MethodHandle(InvokeStatic({}::{}, {}))",
                env.class_names.tpath(class_id),
                name,
                desc.as_pretty_string(&env.class_names)
            )
        }
    }
}

pub(crate) fn ref_info(env: &mut Env, re: Option<GcRef<Instance>>) -> String {
    let Some(re) = re else {
        return "<Null>".to_string()
    };

    let Some(inst) = env.state.gc.deref(re) else {
        return "<Bad GCRef>".to_string()
    };

    let log = env.skip_logging;

    match inst {
        Instance::StaticClass(stat) => {
            let id = stat.id;
            let name = env.class_names.tpath(id);
            format!("StaticClass({})", name)
        }
        Instance::Reference(inst) => match inst {
            ReferenceInstance::Class(class) => {
                let exc_id = env.class_names.gcid_from_bytes(b"java/lang/Exception");
                let string_id = env.class_names.gcid_from_bytes(b"java/lang/String");
                let object_id = env.class_names.object_id();

                let id = class.instanceof;
                let name = env.class_names.tpath(id).to_string();
                if name.is_empty() {
                    format!("Class(ANON:{})", id.get())
                } else if id == string_id {
                    env.skip_logging = true;
                    let text = get_string_contents_as_rust_string(
                        &env.class_files,
                        &mut env.class_names,
                        &mut env.state,
                        re,
                    );
                    env.skip_logging = log;
                    format!("Class(java/lang/String = {:?})", text)
                } else if id
                    == env
                        .class_names
                        .gcid_from_bytes(b"java/lang/invoke/MethodType")
                {
                    env.skip_logging = true;
                    let res = format!(
                        "Class(java/lang/invoke/MethodType = {})",
                        method_type_to_desc_string(
                            &mut env.class_names,
                            &env.class_files,
                            &env.state.gc,
                            re.unchecked_as()
                        )
                    );
                    env.skip_logging = log;
                    res
                } else {
                    env.skip_logging = true;
                    if let Ok(true) = does_extend_class(
                        &mut env.class_names,
                        &mut env.class_files,
                        &mut env.classes,
                        id,
                        exc_id,
                    ) {
                        let throwable_id = env.class_names.gcid_from_bytes(b"java/lang/Throwable");
                        let desc = MethodDescriptor::new_ret(DescriptorType::Basic(
                            DescriptorTypeBasic::Class(string_id),
                        ));

                        let target_method_id = find_virtual_method(
                            &mut env.class_names,
                            &mut env.class_files,
                            &mut env.classes,
                            &mut env.methods,
                            throwable_id,
                            id,
                            b"getMessage",
                            &desc,
                        )
                        .unwrap();

                        let locals =
                            Locals::new_with_array([RuntimeValue::Reference(re.unchecked_as())]);
                        let frame = Frame::new_locals(locals);

                        let res = eval_method(env, target_method_id, frame).unwrap();
                        let EvalMethodValue::Return(RuntimeValue::Reference(msg)) = res else {
                            env.skip_logging = log;
                            return format!("Class({}; Message: <Bad Return>)", name);
                        };

                        let msg = get_string_contents_as_rust_string(
                            &env.class_files,
                            &mut env.class_names,
                            &mut env.state,
                            msg.unchecked_as(),
                        )
                        .unwrap();

                        env.skip_logging = log;

                        format!("Class({}; Message: {:?})", name, msg)
                    } else {
                        let desc = MethodDescriptor::new_ret(DescriptorType::Basic(
                            DescriptorTypeBasic::Class(string_id),
                        ));
                        let target_method_id = find_virtual_method(
                            &mut env.class_names,
                            &mut env.class_files,
                            &mut env.classes,
                            &mut env.methods,
                            object_id,
                            id,
                            b"toString",
                            &desc,
                        )
                        .unwrap();

                        let locals =
                            Locals::new_with_array([RuntimeValue::Reference(re.unchecked_as())]);
                        let frame = Frame::new_locals(locals);

                        let res = eval_method(env, target_method_id, frame);
                        let res = match res {
                            Ok(v) => v,
                            Err(_err) => {
                                env.skip_logging = log;
                                return format!("Class({}; toString = <internal error>)", name);
                            }
                        };
                        let EvalMethodValue::Return(RuntimeValue::Reference(msg)) = res else {
                            env.skip_logging = log;
                            return format!("Class({}; toString = <error>)", name)
                        };

                        let msg = get_string_contents_as_rust_string(
                            &env.class_files,
                            &mut env.class_names,
                            &mut env.state,
                            msg.unchecked_as(),
                        )
                        .unwrap();

                        // Some text can be really big, and passed around a lot!
                        let msg = &msg[0..msg.len().min(200)];

                        env.skip_logging = log;

                        format!("Class({}; toString = {:?})", name, msg)
                    }
                }
            }
            ReferenceInstance::StaticForm(form) => {
                let t = match form.of {
                    RuntimeTypeVoid::Primitive(p) => format!("Primitive({:?})", p),
                    RuntimeTypeVoid::Void => "Void".to_string(),
                    RuntimeTypeVoid::Reference(id) => {
                        let name = env.class_names.tpath(id);
                        format!("{}", name)
                    }
                };

                format!("Class<{}>", t)
            }
            ReferenceInstance::Thread(_t) => "Thread".to_string(),
            ReferenceInstance::MethodHandle(_handle) => mh_info(env, re.unchecked_as()),
            ReferenceInstance::MethodHandleInfo(_info) => "MethodHandleInfo".to_string(),
            ReferenceInstance::PrimitiveArray(arr) => {
                let t = match arr.element_type {
                    RuntimeTypePrimitive::I8 => "I8",
                    RuntimeTypePrimitive::I16 => "I16",
                    RuntimeTypePrimitive::I32 => "I32",
                    RuntimeTypePrimitive::I64 => "I64",
                    RuntimeTypePrimitive::F32 => "F32",
                    RuntimeTypePrimitive::F64 => "F64",
                    RuntimeTypePrimitive::Bool => "Bool",
                    RuntimeTypePrimitive::Char => "Char",
                };

                let mut res = format!("PrimitiveArray<{}>[", t);
                for (i, x) in arr.elements.iter().enumerate() {
                    res.push_str(&format!("{:?}", x));

                    if i != arr.elements.len() - 1 {
                        res.push_str(", ");
                    }
                }

                res.push(']');
                res
            }
            ReferenceInstance::ReferenceArray(arr) => {
                let id = arr.element_type;
                let t = env.class_names.tpath(id);
                let elements = arr.elements.clone();

                let mut res = format!("ReferenceArray<{}>[", t);
                for (i, x) in elements.iter().enumerate() {
                    res.push_str(&ref_info(env, x.map(GcRef::into_generic)));

                    if i != elements.len() - 1 {
                        res.push_str(", ");
                    }
                }

                res.push(']');
                res
            }
        },
    }
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
