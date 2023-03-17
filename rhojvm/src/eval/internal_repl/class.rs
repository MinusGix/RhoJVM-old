use std::{borrow::Cow, num::NonZeroUsize};

use classfile_parser::{
    field_info::{FieldAccessFlags, FieldInfoOpt},
    ClassAccessFlags,
};
use rhojvm_base::{
    class::{ArrayComponentType, ClassVariant},
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        types::JavaChar,
    },
    data::class_files::load_super_class_files_iter,
    id::ClassId,
    util::convert_classfile_text,
    StepError,
};
use smallvec::{smallvec, SmallVec};

use crate::{
    class_instance::{
        ClassInstance, FieldId, FieldIndex, Instance, ReferenceArrayInstance, ReferenceInstance,
        StaticFormInstance,
    },
    eval::{
        eval_method,
        instances::{make_instance_fields, try_casting, CastResult},
        EvalMethodValue, Frame, Locals, ValueException,
    },
    gc::GcRef,
    initialize_class,
    jni::{JBoolean, JClass, JObject, JString},
    rv::{RuntimeTypePrimitive, RuntimeTypeVoid, RuntimeValue, RuntimeValuePrimitive},
    util::{
        self, construct_string, find_field_with_name, find_field_with_name_up_tree,
        get_string_contents_as_rust_string, make_class_form_of, make_empty_ref_array,
        make_err_into_class_not_found_exception, make_primitive_class_form_of, ref_info,
        to_utf16_arr, Env,
    },
    GeneralError,
};

pub(crate) const BOOLEAN_NAME: &[u8] = b"java/lang/Boolean";
pub(crate) const BYTE_NAME: &[u8] = b"java/lang/Byte";
pub(crate) const CHARACTER_NAME: &[u8] = b"java/lang/Character";
pub(crate) const DOUBLE_NAME: &[u8] = b"java/lang/Double";
pub(crate) const FLOAT_NAME: &[u8] = b"java/lang/Float";
pub(crate) const INTEGER_NAME: &[u8] = b"java/lang/Integer";
pub(crate) const LONG_NAME: &[u8] = b"java/lang/Long";
pub(crate) const SHORT_NAME: &[u8] = b"java/lang/Short";

pub(crate) extern "C" fn class_get_primitive(
    env: *mut Env<'_>,
    _this: JObject,
    name: JString,
) -> JObject {
    assert!(!env.is_null(), "Env was null when passed into java/lang/Class getPrimitive, which is indicative of an internal bug.");

    let env = unsafe { &mut *env };

    let name = unsafe { env.get_jobject_as_gcref(name) }.unwrap();
    let name = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        name,
    )
    .unwrap();

    // Note: This assumes that the jchar encoding can be directly compared to the ascii bytes for
    // these basic characters
    let class_typ = if name == "B" || name == "byte" {
        make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::I8))
    } else if name == "C" || name == "char" {
        make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::Char))
    } else if name == "D" || name == "double" {
        make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::F64))
    } else if name == "F" || name == "float" {
        make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::F32))
    } else if name == "I" || name == "int" || name == "integer" {
        make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::I32))
    } else if name == "J" || name == "long" {
        make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::I64))
    } else if name == "S" || name == "short" {
        make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::I16))
    } else if name == "Z" || name == "bool" || name == "boolean" {
        make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::Bool))
    } else if name == "V" || name == "void" {
        make_primitive_class_form_of(env, None)
    } else {
        panic!("Unknown name ({}) passed into Class#getPrimitive", name);
    };

    let class_typ = class_typ.expect("Failed to construct primitive class.");
    if let Some(class_typ) = env.state.extract_value(class_typ) {
        unsafe { env.get_local_jobject_for(class_typ.into_generic()) }
    } else {
        // Exception
        JObject::null()
    }
}

/// Gets the class name id for a slice of java characters in the format of Class's name
/// This is basically the same as typical, except instead of / it uses .
pub(crate) fn get_class_name_id_for(
    env: &mut Env,
    name: GcRef<Instance>,
) -> Result<ClassId, GeneralError> {
    // TODO: We could do cheaper insertion, especially if it already exists in class names
    let contents =
        util::get_string_contents(&env.class_files, &mut env.class_names, &mut env.state, name)?;
    // Converting back to cesu8 is expensive, but this kind of operation isn't common enough to do
    // anything like storing cesu8 versions alongside them, probably.
    let contents = contents
        .iter()
        .map(|x| x.into_char().unwrap().0)
        .map(|x| {
            if x == u16::from(b'.') {
                u16::from(b'/')
            } else {
                x
            }
        })
        .collect::<Vec<u16>>();
    let contents = String::from_utf16(&contents).map_err(GeneralError::StringConversionFailure)?;
    // TODO: We should actually convert it to cesu8!
    let contents = contents.as_bytes();
    let id = env.class_names.gcid_from_bytes(contents);
    Ok(id)
}

pub(crate) extern "C" fn class_get_class_for_name_with_class_loader(
    env: *mut Env<'_>,
    _this: JObject,
    name: JString,
    _initialize: JBoolean,
    _loader: JObject,
) -> JObject {
    // FIXME: We're currently ignoring the loader

    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let name = unsafe { env.get_jobject_as_gcref(name) };
    let name = name.expect("null ref exception");

    let class_id = get_class_name_id_for(env, name).unwrap();

    let origin_id = env.get2_calling_class_id().unwrap_or(class_id);

    // FIXME: I believe this is wrong, however our current implementation requires the class to be
    // initialized before a Class<?> can be made for it, since it requires a StaticClassInstance.
    // We likely have to loosen that to only having the class file be loaded.
    // The make class form of will always initialize it
    // FIXME: I think we should use the caller here? Or modify it so it can take the loader?
    let class_form = util::make_class_form_of(env, origin_id, class_id);

    let class_form = make_err_into_class_not_found_exception(env, class_form, class_id).unwrap();
    let Some(class_form) = env.state.extract_value(class_form) else {
        return JObject::null();
    };
    let Some(class_form) = env.state.extract_value(class_form) else {
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(class_form.into_generic()) }
}

pub(crate) extern "C" fn class_get_class_for_name(
    env: *mut Env<'_>,
    _this: JObject,
    name: JString,
) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let name = unsafe { env.get_jobject_as_gcref(name) };
    // TODO: It doesn't actually specify what it does on null-name
    let name = name.expect("null ref exception");

    let class_id = get_class_name_id_for(env, name).unwrap();

    let class_class_id = env.class_names.gcid_from_bytes(b"java/lang/Class");

    let origin_id = env.get2_calling_class_id().unwrap_or(class_class_id);

    let class_form = make_class_form_of(env, origin_id, class_id);

    let class_form = make_err_into_class_not_found_exception(env, class_form, class_id).unwrap();
    let Some(class_form) = env.state.extract_value(class_form) else {
        return JObject::null();
    };
    let Some(class_form) = env.state.extract_value(class_form) else {
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(class_form.into_generic()) }
}

pub(crate) extern "C" fn class_get_name(env: *mut Env<'_>, this: JObject) -> JString {
    assert!(!env.is_null(), "Env was null when passed to java/lang/Class getDeclaredField, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    // Class<T>
    // SAFETY: We assume that it is a valid ref and that it has not been
    // forged.
    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("get name's class was null");
    let this_of = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    let name = match this_of {
        RuntimeTypeVoid::Primitive(prim) => Cow::Borrowed(match prim {
            RuntimeTypePrimitive::I64 => "long",
            RuntimeTypePrimitive::I32 => "int",
            RuntimeTypePrimitive::I16 => "short",
            RuntimeTypePrimitive::Bool => "boolean",
            RuntimeTypePrimitive::I8 => "byte",
            RuntimeTypePrimitive::F32 => "float",
            RuntimeTypePrimitive::F64 => "double",
            RuntimeTypePrimitive::Char => "char",
        }),
        RuntimeTypeVoid::Void => Cow::Borrowed("void"),
        RuntimeTypeVoid::Reference(this_id) => {
            let (name, _) = env.class_names.name_from_gcid(this_id).unwrap();
            let name = name.get();
            // TODO: Don't use this
            let name = convert_classfile_text(name);

            // Replace it with . since those names are meant to be separated by a .
            Cow::Owned(name.replace('/', "."))
        }
    };

    let name = name
        .encode_utf16()
        .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
        .collect();
    let name = construct_string(env, name, true).unwrap();
    let name = match name {
        ValueException::Value(name) => name,
        ValueException::Exception(_) => todo!(),
    };

    unsafe { env.get_local_jobject_for(name.into_generic()) }
}

pub(crate) extern "C" fn class_get_simple_name(env: *mut Env<'_>, this: JObject) -> JString {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    // Class<T>
    // SAFETY: We assume that it is a valid ref and that it has not been forged
    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("get simple name's class was null");
    let this_of = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
    } else {
        // Should be caught by method calling
        panic!();
    };

    let name = match this_of {
        RuntimeTypeVoid::Primitive(prim) => Cow::Borrowed(match prim {
            RuntimeTypePrimitive::I64 => "long",
            RuntimeTypePrimitive::I32 => "int",
            RuntimeTypePrimitive::I16 => "short",
            RuntimeTypePrimitive::Bool => "boolean",
            RuntimeTypePrimitive::I8 => "byte",
            RuntimeTypePrimitive::F32 => "float",
            RuntimeTypePrimitive::F64 => "double",
            RuntimeTypePrimitive::Char => "char",
        }),
        RuntimeTypeVoid::Void => Cow::Borrowed("void"),
        RuntimeTypeVoid::Reference(this_id) => {
            // TODO: anonymous classes shouldn't have a simple name
            let (name, _) = env.class_names.name_from_gcid(this_id).unwrap();
            let name = name.get();
            // TODO: Don't use this
            let name = convert_classfile_text(name);

            // Replace it with . since those names are meant to be separated by a .
            Cow::Owned(name.split("/").last().unwrap_or("").to_owned())
        }
    };

    let name = name
        .encode_utf16()
        .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
        .collect();
    let name = construct_string(env, name, true).unwrap();
    let name = match name {
        ValueException::Value(name) => name,
        ValueException::Exception(_) => todo!(),
    };

    unsafe { env.get_local_jobject_for(name.into_generic()) }
}

/// java/lang/Class
/// `public Field getDeclaredField(String name);`
pub(crate) extern "C" fn class_get_declared_field(
    env: *mut Env<'_>,
    this: JObject,
    name: JString,
) -> JObject {
    assert!(!env.is_null(), "Env was null when passed to java/lang/Class getDeclaredField, which is indicative of an internal bug.");

    // SAFETY: We already checked that it is not null, and we rely on native method calling's
    // safety for this to be fine to turn into a reference
    let env = unsafe { &mut *env };

    // Class<T>
    // SAFETY: We assume that it is a valid ref and that it has not been
    // forged.
    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("RegisterNative's class was null");
    let this_id = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
            .into_reference()
            .expect("Expected Class<T> to be of a Class")
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    // TODO: null pointer exception
    // SAFETY: We assume that it is a valid ref and that it has not been forged.
    let name = unsafe { env.get_jobject_as_gcref(name) };
    let name = name.expect("getDeclaredField's name was null");
    // TODO: This is doing far more work than needs to be done.
    let name_text = util::get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        name,
    )
    .unwrap();

    let (field_id, field_info) =
        find_field_with_name(&env.class_files, this_id, name_text.as_bytes())
            .unwrap()
            .unwrap();

    let field_ref = make_field(env, field_id, field_info);

    unsafe { env.get_local_jobject_for(field_ref.into_generic()) }
}

/// `Field[] getDeclaredFields()`
pub(crate) extern "C" fn class_get_declared_fields(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Class get declared fields's this ref was null");
    let this = this.unchecked_as::<StaticFormInstance>();

    let this_id = env
        .state
        .gc
        .deref(this)
        .unwrap()
        .of
        .into_reference()
        .unwrap();

    let field_class_id = env.class_names.gcid_from_bytes(b"java/lang/reflect/Field");
    let field_arr_id = env
        .class_names
        .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), field_class_id)
        .unwrap();

    let class_file = env.class_files.get(&this_id).unwrap();
    let fields: SmallVec<[_; 10]> = class_file.load_field_values_iter().collect();

    let mut decl_fields = Vec::new();
    for (i, field_data) in fields.into_iter().enumerate() {
        let field_idx = FieldIndex::new_unchecked(i as u16);
        let field_id = FieldId::unchecked_compose(this_id, field_idx);
        let (field_info, _) = field_data.map_err(GeneralError::ClassFileLoad).unwrap();

        let field = make_field(env, field_id, field_info);
        decl_fields.push(Some(field.into_generic()));
    }

    let decl_fields = ReferenceArrayInstance::new(field_arr_id, field_class_id, decl_fields);
    let decl_fields = env.state.gc.alloc(decl_fields);

    unsafe { env.get_local_jobject_for(decl_fields.into_generic()) }
}

pub(crate) unsafe extern "C" fn class_get_fields(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };
    let field_class_id = env.class_names.gcid_from_bytes(b"java/lang/reflect/Field");

    let this = unsafe { env.get_jobject_as_gcref(this) }.unwrap();
    let this = this.unchecked_as::<StaticFormInstance>();
    let of_id = match env.state.gc.deref(this).unwrap().of {
        RuntimeTypeVoid::Void | RuntimeTypeVoid::Primitive(_) => {
            let arr = make_empty_ref_array(env, field_class_id).unwrap();
            return unsafe { env.get_local_jobject_for(arr.into_generic()) };
        }
        RuntimeTypeVoid::Reference(of_id) => of_id,
    };

    if env.class_names.is_array(of_id).unwrap() {
        // getFields does not recognize `length` as a field
        let arr = make_empty_ref_array(env, field_class_id).unwrap();
        return unsafe { env.get_local_jobject_for(arr.into_generic()) };
    }

    let mut fields = Vec::new();

    let mut tree_iter = load_super_class_files_iter(of_id);
    while let Some(target_id) = tree_iter.next_item(&mut env.class_names, &mut env.class_files) {
        let target_id = target_id.unwrap();
        let (_, target_info) = env
            .class_names
            .name_from_gcid(target_id)
            .map_err(StepError::BadId)
            .unwrap();

        if !target_info.has_class_file() {
            continue;
        }

        let class_file = env.class_files.get(&target_id).unwrap().clone();

        for (i, field_data) in class_file.load_field_values_iter().enumerate() {
            let i = FieldIndex::new_unchecked(i as u16);
            let (field_info, _) = field_data.unwrap();
            if !field_info.access_flags.contains(FieldAccessFlags::PUBLIC) {
                continue;
            }

            let field_id = FieldId::unchecked_compose(target_id, i);

            let field = make_field(env, field_id, field_info);
            fields.push(Some(field.into_generic()));
        }
    }

    let array_id = env
        .class_names
        .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), field_class_id)
        .unwrap();
    let array_inst = ReferenceArrayInstance::new(array_id, field_class_id, fields);

    let array_ref = env.state.gc.alloc(array_inst);

    unsafe { env.get_local_jobject_for(array_ref.into_generic()) }
}

pub(crate) unsafe extern "C" fn class_get_field(
    env: *mut Env<'_>,
    this: JObject,
    name: JString,
) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) }.unwrap();
    let this = this.unchecked_as::<StaticFormInstance>();
    let of_id = match env.state.gc.deref(this).unwrap().of {
        RuntimeTypeVoid::Void | RuntimeTypeVoid::Primitive(_) => {
            // TODO: Is this always a NoSuchFieldException? or does it do something wacky and goto the underlying class, like `Integer`?
            todo!()
        }
        RuntimeTypeVoid::Reference(of_id) => of_id,
    };

    if env.class_names.is_array(of_id).unwrap() {
        // GetField does not work on arrays
        todo!("NoSuchFieldException")
    }

    // TODO: NPE if name is null
    let name = unsafe { env.get_jobject_as_gcref(name) }.unwrap();
    let name = util::get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        name,
    )
    .unwrap();

    // TODO: nosuchfieldexception
    let (field_id, field_info) = find_field_with_name_up_tree(
        &mut env.class_names,
        &mut env.class_files,
        of_id,
        name.as_bytes(),
        |_, info| info.access_flags.contains(FieldAccessFlags::PUBLIC),
    )
    .unwrap()
    .unwrap();

    let field_ref = make_field(env, field_id, field_info);

    unsafe { env.get_local_jobject_for(field_ref.into_generic()) }
}

/// Construct a `java/lang/reflect/Field` instance.
fn make_field(env: &mut Env, field_id: FieldId, field_info: FieldInfoOpt) -> GcRef<ClassInstance> {
    let field_class_id = env.class_names.gcid_from_bytes(b"java/lang/reflect/Field");
    let field_internal_class_id = env.class_names.gcid_from_bytes(b"rho/InternalField");

    // TODO: We could make a InternalField a magic class?
    let field_internal_ref = {
        // TODO: resolve derive??
        let field_internal_class_ref = match initialize_class(env, field_internal_class_id)
            .unwrap()
            .into_value()
        {
            ValueException::Value(v) => v,
            ValueException::Exception(_) => todo!(),
        };

        let field_internal_fields = make_instance_fields(env, field_internal_class_id).unwrap();
        let Some(mut field_internal_fields) = env.state.extract_value(field_internal_fields) else {
            todo!()
        };

        {
            let (f_class_id, f_field_index) = field_id.decompose();
            let (class_id_field, field_index_field, flags_field) = env
                .state
                .get_internal_field_ids(&env.class_files, field_internal_class_id)
                .unwrap();
            *(field_internal_fields
                .get_mut(class_id_field)
                .unwrap()
                .value_mut()) = RuntimeValuePrimitive::I32(f_class_id.get() as i32).into();
            *(field_internal_fields
                .get_mut(field_index_field)
                .unwrap()
                .value_mut()) = RuntimeValuePrimitive::I16(f_field_index.get() as i16).into();
            *(field_internal_fields
                .get_mut(flags_field)
                .unwrap()
                .value_mut()) =
                RuntimeValuePrimitive::I16(field_info.access_flags.bits() as i16).into();
        };

        let field_internal = ClassInstance {
            instanceof: field_internal_class_id,
            static_ref: field_internal_class_ref,
            fields: field_internal_fields,
        };
        env.state.gc.alloc(field_internal)
    };

    let field_ref = {
        // TODO: resolve derive??
        let field_class_ref = match initialize_class(env, field_class_id).unwrap().into_value() {
            ValueException::Value(v) => v,
            ValueException::Exception(_) => todo!(),
        };

        let field_fields = make_instance_fields(env, field_class_id).unwrap();
        let Some(mut field_fields) = env.state.extract_value(field_fields) else {
            todo!()
        };

        {
            let internal_field_id = env
                .state
                .get_field_internal_field_id(&env.class_files, field_class_id)
                .unwrap();
            *(field_fields.get_mut(internal_field_id).unwrap().value_mut()) =
                RuntimeValue::Reference(field_internal_ref.into_generic());
        };

        let field = ClassInstance {
            instanceof: field_class_id,
            static_ref: field_class_ref,
            fields: field_fields,
        };
        env.state.gc.alloc(field)
    };

    field_ref
}

/// `() -> Constructor<?>[]`
pub(crate) extern "C" fn class_get_declared_constructors(
    env: *mut Env<'_>,
    this: JObject,
) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Class get declared constructors's this ref was null");
    let this = this.unchecked_as::<StaticFormInstance>();
    let Some(this) = env.state.gc.deref(this) else {
        unreachable!();
    };

    let constructor_id = env
        .class_names
        .gcid_from_bytes(b"java/lang/reflect/Constructor");
    let constructor_arr_id = env
        .class_names
        .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), constructor_id)
        .unwrap();

    // If this is a primitive, void, array, or interface, then it has no constructors.
    let is_fundamentally_empty = match this.of {
        RuntimeTypeVoid::Primitive(_) => true,
        RuntimeTypeVoid::Void => true,
        RuntimeTypeVoid::Reference(id) => {
            if env.class_names.is_array(id).unwrap() {
                true
            } else {
                let this_class = env.classes.get(&id).unwrap();
                this_class.is_interface()
            }
        }
    };

    if is_fundamentally_empty {
        // TODO: Should we have a special empty array instance that works for all types?
        // (or one for primitives, and one for references?)
        let empty_array = env.state.gc.alloc(ReferenceArrayInstance::new(
            constructor_arr_id,
            constructor_id,
            Vec::new(),
        ));

        return unsafe { env.get_local_jobject_for(empty_array.into_generic()) };
    }

    let RuntimeTypeVoid::Reference(this_of_id) = this.of else {
        unreachable!();
    };

    let class_class_id = env.class_names.gcid_from_bytes(b"java/lang/Class");

    // Get the method id for the Constructor constructor
    let constructor_init_id = {
        let constructor_desc = MethodDescriptor::new(
            smallvec![
                // Class that the constructor is on
                DescriptorType::Basic(DescriptorTypeBasic::Class(class_class_id)),
                // Method index
                DescriptorType::Basic(DescriptorTypeBasic::Short),
            ],
            None,
        );
        env.methods
            .load_method_from_desc(
                &mut env.class_names,
                &mut env.class_files,
                constructor_id,
                b"<init>",
                &constructor_desc,
            )
            .unwrap()
    };

    // Get the static ref for the Constructor ClassInstance
    let constructor_static_ref = initialize_class(env, constructor_id).unwrap().into_value();
    let Some(constructor_static_ref) = env.state.extract_value(constructor_static_ref) else {
        todo!()
    };

    let of_static_form = make_class_form_of(env, class_class_id, this_of_id).unwrap();
    let Some(of_static_form) = env.state.extract_value(of_static_form) else {
        todo!()
    };

    // Now we need to find all the constructors on the T in this Class<T> instance
    // Constructor[]
    let ClassVariant::Class(of_class) = env.classes.get(&this_of_id).unwrap() else {
        unreachable!()
    };

    let mut constructors: Vec<Option<GcRef<ReferenceInstance>>> = Vec::new();
    for method_id in of_class.iter_method_ids() {
        // Ensure the method is loaded
        env.methods
            .load_method_from_id(&mut env.class_names, &mut env.class_files, method_id)
            .unwrap();

        let method = env.methods.get(&method_id).unwrap();
        if !method.is_init() {
            continue;
        }

        // Create the constructor instance
        let constructor_ref = {
            let fields = make_instance_fields(env, constructor_id).unwrap();
            let Some(fields) = env.state.extract_value(fields) else {
                unreachable!()
            };

            let constructor_inst = ClassInstance {
                instanceof: constructor_id,
                static_ref: constructor_static_ref,
                fields,
            };

            env.state.gc.alloc(constructor_inst)
        };

        // Get the method index to store with it
        let (_, method_index) = method_id.decompose();
        let method_index = i16::from_be_bytes(u16::to_be_bytes(method_index));

        let locals = Locals::new_with_array([
            // `this`, aka the Constructor instance
            RuntimeValue::Reference(constructor_ref.into_generic()),
            // The class the Constructor refers to
            RuntimeValue::Reference(of_static_form.into_generic()),
            // The specific index in that class which is the Constructor method
            RuntimeValue::Primitive(RuntimeValuePrimitive::I16(method_index)),
        ]);
        let frame = Frame::new_locals(locals);

        // Initialize the constructor
        match eval_method(env, constructor_init_id.into(), frame).unwrap() {
            EvalMethodValue::ReturnVoid => {}
            EvalMethodValue::Return(_) => tracing::warn!("Constructor init returned a value"),
            EvalMethodValue::Exception(_) => {
                todo!("There was an exception in the Constructor init")
            }
        }

        constructors.push(Some(constructor_ref.into_generic()));
    }

    // Create the constructor array
    let constructor_array = env.state.gc.alloc(ReferenceArrayInstance::new(
        constructor_arr_id,
        constructor_id,
        constructors,
    ));

    unsafe { env.get_local_jobject_for(constructor_array.into_generic()) }
}

pub(crate) extern "C" fn class_new_instance(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Class new instance's this ref was null");
    let this_of = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    let this_id = if let Some(id) = this_of.into_reference() {
        id
    } else {
        todo!("InstantiationException because of primitive")
    };

    let static_ref = initialize_class(env, this_id).unwrap().into_value();
    let static_ref = match static_ref {
        ValueException::Value(static_ref) => static_ref,
        ValueException::Exception(exc) => todo!(
            "Handle exception in initializing class: {} ({:?}) {}",
            env.class_names.tpath(this_id).to_string(),
            this_id,
            ref_info(env, exc)
        ),
    };

    let target_class = env.classes.get(&this_id).unwrap();
    if target_class.is_interface()
        || target_class
            .access_flags()
            .contains(ClassAccessFlags::ABSTRACT)
    {
        todo!("InstantiationError exception");
    }

    let fields = make_instance_fields(env, this_id).unwrap();
    let Some(fields) = env.state.extract_value(fields) else {
        todo!()
    };

    let class = ClassInstance {
        instanceof: this_id,
        static_ref,
        fields,
    };

    // Allocate the class instance
    let class_ref = env.state.gc.alloc(class);

    // However, now we have to run the empty constructor if one exists for the class.
    // Note: We don't include the `this` pointer because that is implicit in the descriptor
    let descriptor = MethodDescriptor::new_empty();

    // TODO: We need to check that they can access the constructor!
    let method_id = env
        .methods
        .load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            this_id,
            b"<init>",
            &descriptor,
        )
        .unwrap();

    let locals = Locals::new_with_array([RuntimeValue::Reference(class_ref.into_generic())]);
    let frame = Frame::new_locals(locals);
    match eval_method(env, method_id.into(), frame).unwrap() {
        EvalMethodValue::ReturnVoid => {}
        EvalMethodValue::Return(_) => tracing::warn!("Constructor returned value?"),
        EvalMethodValue::Exception(exc) => {
            todo!(
                "There was an exception calling the default constructor: {}",
                ref_info(env, exc)
            )
        }
    }

    // Now we just return the initialized class ref
    unsafe { env.get_local_jobject_for(class_ref.into_generic()) }
}

// TODO: Cache created packages
pub(crate) extern "C" fn class_get_package(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Class new instance's this ref was null");
    // The id held inside
    let this_of = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    let this_id = if let Some(id) = this_of.into_reference() {
        id
    } else {
        // Otherwise, return null for primitive types
        // TODO: is this correct?
        return JObject::null();
    };

    // TODO: Should we assume its loaded?
    let class = env.classes.get(&this_id).unwrap();
    let (package_name, package_info) = if let Some(package_id) = class.package() {
        let package = env.packages.get(package_id).unwrap();
        (package.name(), &package.info)
    } else {
        (b"" as &[u8], &env.packages.null_package_info)
    };

    // TODO: reduce the amount of code repetition
    let package_name = to_utf16_arr(convert_classfile_text(package_name).as_ref());
    let spec_title = package_info
        .specification_title
        .as_ref()
        .map(AsRef::as_ref)
        .map(to_utf16_arr);
    let spec_vendor = package_info
        .specification_vendor
        .as_ref()
        .map(AsRef::as_ref)
        .map(to_utf16_arr);
    let spec_version = package_info
        .specification_version
        .as_ref()
        .map(AsRef::as_ref)
        .map(to_utf16_arr);
    let impl_title = package_info
        .implementation_title
        .as_ref()
        .map(AsRef::as_ref)
        .map(to_utf16_arr);
    let impl_vendor = package_info
        .implementation_vendor
        .as_ref()
        .map(AsRef::as_ref)
        .map(to_utf16_arr);
    let impl_version = package_info
        .implementation_version
        .as_ref()
        .map(AsRef::as_ref)
        .map(to_utf16_arr);

    let sealed = package_info.sealed;

    let package_name_ref = match construct_string(env, package_name, true).unwrap() {
        ValueException::Value(name) => name,
        ValueException::Exception(_) => todo!("Exception initializing package name"),
    };
    let spec_title_ref = if let Some(val) = spec_title
        .map(|x| construct_string(env, x, true))
        .transpose()
        .unwrap()
    {
        match val {
            ValueException::Value(val) => RuntimeValue::Reference(val.into_generic()),
            ValueException::Exception(_) => todo!(),
        }
    } else {
        RuntimeValue::NullReference
    };
    let spec_vendor_ref = if let Some(val) = spec_vendor
        .map(|x| construct_string(env, x, true))
        .transpose()
        .unwrap()
    {
        match val {
            ValueException::Value(val) => RuntimeValue::Reference(val.into_generic()),
            ValueException::Exception(_) => todo!(),
        }
    } else {
        RuntimeValue::NullReference
    };
    let spec_version_ref = if let Some(val) = spec_version
        .map(|x| construct_string(env, x, true))
        .transpose()
        .unwrap()
    {
        match val {
            ValueException::Value(val) => RuntimeValue::Reference(val.into_generic()),
            ValueException::Exception(_) => todo!(),
        }
    } else {
        RuntimeValue::NullReference
    };
    let impl_title_ref = if let Some(val) = impl_title
        .map(|x| construct_string(env, x, true))
        .transpose()
        .unwrap()
    {
        match val {
            ValueException::Value(val) => RuntimeValue::Reference(val.into_generic()),
            ValueException::Exception(_) => todo!(),
        }
    } else {
        RuntimeValue::NullReference
    };
    let impl_vendor_ref = if let Some(val) = impl_vendor
        .map(|x| construct_string(env, x, true))
        .transpose()
        .unwrap()
    {
        match val {
            ValueException::Value(val) => RuntimeValue::Reference(val.into_generic()),
            ValueException::Exception(_) => todo!(),
        }
    } else {
        RuntimeValue::NullReference
    };
    let impl_version_ref = if let Some(val) = impl_version
        .map(|x| construct_string(env, x, true))
        .transpose()
        .unwrap()
    {
        match val {
            ValueException::Value(val) => RuntimeValue::Reference(val.into_generic()),
            ValueException::Exception(_) => todo!(),
        }
    } else {
        RuntimeValue::NullReference
    };

    let package_class_id = env.class_names.gcid_from_bytes(b"java/lang/Package");
    let package_class_ref = match initialize_class(env, package_class_id)
        .unwrap()
        .into_value()
    {
        ValueException::Value(re) => re,
        ValueException::Exception(_) => todo!("Exception initializing Package class"),
    };

    let fields = make_instance_fields(env, package_class_id).unwrap();
    let Some(fields) = env.state.extract_value(fields) else {
        todo!()
    };

    let package_instance = ClassInstance {
        instanceof: package_class_id,
        static_ref: package_class_ref,
        fields,
    };

    let package_ref = env.state.gc.alloc(package_instance);

    let string_id = env.class_names.gcid_from_bytes(b"java/lang/String");
    let constructor_desc = MethodDescriptor::new(
        smallvec::smallvec![
            // name
            DescriptorTypeBasic::Class(string_id).into(),
            // spec title
            DescriptorTypeBasic::Class(string_id).into(),
            // spec vendor
            DescriptorTypeBasic::Class(string_id).into(),
            // spec version
            DescriptorTypeBasic::Class(string_id).into(),
            // impl title
            DescriptorTypeBasic::Class(string_id).into(),
            // impl vendor
            DescriptorTypeBasic::Class(string_id).into(),
            // impl version
            DescriptorTypeBasic::Class(string_id).into(),
            // is sealed
            DescriptorTypeBasic::Boolean.into()
        ],
        None,
    );

    let constructor_id = env
        .methods
        .load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            package_class_id,
            b"<init>",
            &constructor_desc,
        )
        .unwrap();

    let locals = Locals::new_with_array([
        RuntimeValue::Reference(package_ref.into_generic()),
        RuntimeValue::Reference(package_name_ref.into_generic()),
        spec_title_ref,
        spec_vendor_ref,
        spec_version_ref,
        impl_title_ref,
        impl_vendor_ref,
        impl_version_ref,
        RuntimeValuePrimitive::Bool(sealed.unwrap_or(false).into()).into(),
    ]);
    let frame = Frame::new_locals(locals);

    match eval_method(env, constructor_id.into(), frame) {
        Ok(res) => match res {
            EvalMethodValue::ReturnVoid => {}
            EvalMethodValue::Return(_) => tracing::warn!("Constructor returned value"),
            EvalMethodValue::Exception(_) => todo!(),
        },
        Err(err) => panic!("{:?}", err),
    };

    unsafe { env.get_local_jobject_for(package_ref.into_generic()) }
}

pub(crate) extern "C" fn class_is_primitive(env: *mut Env<'_>, this: JObject) -> JBoolean {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Class new instance's this ref was null");
    // The id held inside
    let this_of = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    JBoolean::from(match this_of {
        RuntimeTypeVoid::Primitive(_) | RuntimeTypeVoid::Void => true,
        RuntimeTypeVoid::Reference(_) => false,
    })
}

pub(crate) extern "C" fn class_is_array(env: *mut Env<'_>, this: JObject) -> JBoolean {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Class new instance's this ref was null");
    // The id held inside
    let this_of = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    JBoolean::from(match this_of {
        RuntimeTypeVoid::Primitive(_) | RuntimeTypeVoid::Void => false,
        RuntimeTypeVoid::Reference(this_id) => {
            let (_, info) = env.class_names.name_from_gcid(this_id).unwrap();

            info.is_array()
        }
    })
}

pub(crate) extern "C" fn class_get_component_type(env: *mut Env<'_>, this: JClass) -> JClass {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("Class new instance's this ref was null");
    // The id held inside
    if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        let this_id = this
            .of
            .into_reference()
            .expect("Expected Class<T> of a Class");

        let prim_class_form = match env.classes.get(&this_id).unwrap() {
            ClassVariant::Array(array) => match array.component_type() {
                ArrayComponentType::Boolean => {
                    make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::Bool))
                }
                ArrayComponentType::Char => {
                    make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::Char))
                }
                ArrayComponentType::Byte => {
                    make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::I8))
                }
                ArrayComponentType::Short => {
                    make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::I16))
                }
                ArrayComponentType::Int => {
                    make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::I32))
                }
                ArrayComponentType::Long => {
                    make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::I64))
                }
                ArrayComponentType::Float => {
                    make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::F32))
                }
                ArrayComponentType::Double => {
                    make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::F64))
                }
                ArrayComponentType::Class(id) => {
                    // TODO: this usage is incorrect
                    let form = make_class_form_of(env, id, id).unwrap();
                    if let Some(form) = env.state.extract_value(form) {
                        return unsafe { env.get_local_jobject_for(form.into_generic()) };
                    } else {
                        // There was an exception
                        return JClass::null();
                    }
                }
            },
            // It wasn't an array
            ClassVariant::Class(_) => return JClass::null(),
        };

        let prim_class_form = prim_class_form.unwrap();
        if let Some(prim_class_form) = env.state.extract_value(prim_class_form) {
            unsafe { env.get_local_jobject_for(prim_class_form.into_generic()) }
        } else {
            // Exception
            JClass::null()
        }
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    }
}

pub(crate) extern "C" fn class_is_assignable_from(
    env: *mut Env<'_>,
    this: JClass,
    other: JClass,
) -> JBoolean {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("IsAssignableFrom's class was null");
    let this_of = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    let other = unsafe { env.get_jobject_as_gcref(other) };
    let other = other.expect("IsAssignableFrom's other class was null");
    let other_of = if let Instance::Reference(ReferenceInstance::StaticForm(other)) =
        env.state.gc.deref(other).unwrap()
    {
        other.of
    } else {
        panic!();
    };

    // If they're the same primitive typeclass or the same class id then they're the same
    if this_of == other_of {
        return JBoolean::from(true);
    }

    // If they aren't references then they can't be equal

    let this_id = if let Some(id) = this_of.into_reference() {
        id
    } else {
        return JBoolean::from(false);
    };

    let other_id = if let Some(id) = other_of.into_reference() {
        id
    } else {
        return JBoolean::from(false);
    };

    let is_castable = other_id == this_id
        || env
            .classes
            .is_super_class(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.packages,
                other_id,
                this_id,
            )
            .unwrap()
        || env
            .classes
            .implements_interface(
                &mut env.class_names,
                &mut env.class_files,
                other_id,
                this_id,
            )
            .unwrap()
        || env
            .classes
            .is_castable_array(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.packages,
                other_id,
                this_id,
            )
            .unwrap();

    // TODO: We need to special handle primitive classes

    JBoolean::from(is_castable)
}

pub(crate) extern "C" fn class_is_instance(
    env: *mut Env<'_>,
    this: JClass,
    other: JObject,
) -> JBoolean {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("IsAssignableFrom's class was null");
    let this_id = if let Instance::Reference(ReferenceInstance::StaticForm(this)) =
        env.state.gc.deref(this).unwrap()
    {
        this.of
            .into_reference()
            .expect("Expected Class<T> to be of a Class")
    } else {
        // This should be caught by method calling
        // Though it would be good to not panic
        panic!();
    };

    let class_class_id = env.class_names.gcid_from_bytes(b"java/lang/Class");

    let other = unsafe { env.get_jobject_as_gcref(other) };
    let other = other.expect("IsInstance's other class was null");
    let other_id = match env.state.gc.deref(other).unwrap() {
        Instance::StaticClass(_) => todo!(),
        Instance::Reference(re) => re.instanceof(),
    };

    match try_casting(env, class_class_id, other_id, this_id, |_env, _, _, _| {
        Ok(CastResult::Failure)
    })
    .unwrap()
    {
        CastResult::Success => JBoolean::from(true),
        CastResult::Failure => JBoolean::from(false),
        CastResult::Exception(exc) => {
            env.state.fill_native_exception(exc);
            JBoolean::from(false)
        }
    }
}

pub(crate) extern "C" fn class_is_interface(env: *mut Env<'_>, this: JClass) -> JBoolean {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("IsAssignableFrom's class was null");
    let Some(Instance::Reference(ReferenceInstance::StaticForm(this))) =
        env.state.gc.deref(this) else {
            panic!();
        };
    let this_id = this
        .of
        .into_reference()
        .expect("Expected Class<T> to be of a Class");

    // TODO: initialize the class if needed
    let this_class = env.classes.get(&this_id).unwrap();
    JBoolean::from(this_class.is_interface())
}
