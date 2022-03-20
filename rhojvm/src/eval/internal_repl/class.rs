use std::borrow::Cow;

use classfile_parser::{field_info::FieldAccessFlags, ClassAccessFlags};
use either::Either;
use rhojvm_base::{
    class::{ArrayComponentType, ClassVariant},
    code::{
        method::{DescriptorTypeBasic, MethodDescriptor},
        types::JavaChar,
    },
    id::ClassId,
    util::convert_classfile_text,
};

use crate::{
    class_instance::{ClassInstance, Instance, ReferenceInstance, StaticFormInstance},
    eval::{
        eval_method,
        instances::{make_fields, try_casting, CastResult},
        EvalMethodValue, Frame, Locals, ValueException,
    },
    gc::GcRef,
    initialize_class,
    jni::{JBoolean, JClass, JObject, JString},
    rv::{RuntimeTypePrimitive, RuntimeTypeVoid, RuntimeValue, RuntimeValuePrimitive},
    util::{
        self, construct_string, find_field_with_name, get_string_contents_as_rust_string,
        make_class_form_of, make_primitive_class_form_of, to_utf16_arr, Env,
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
fn get_class_name_id_for(env: &mut Env, name: GcRef<Instance>) -> Result<ClassId, GeneralError> {
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
    tracing::info!("Get Class Name Id for: {}", contents);
    // TODO: We should actually convert it to cesu8!
    let contents = contents.as_bytes();
    let id = env.class_names.gcid_from_bytes(contents);
    tracing::info!("\tId: {:?}", id);
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

    // FIXME: I believe this is wrong, however our current implementation requires the class to be
    // initialized before a Class<?> can be made for it, since it requires a StaticClassInstance.
    // We likely have to loosen that to only having the class file be loaded.
    // The make class form of will always initialize it
    // FIXME: I think we should use the caller here? Or modify it so it can take the loader?
    let class_form = util::make_class_form_of(env, class_id, class_id).expect("Handle errors");
    let class_form = match class_form {
        ValueException::Value(form) => form,
        ValueException::Exception(_) => todo!("Exception in the creating class form"),
    };

    unsafe { env.get_local_jobject_for(class_form.into_generic()) }
}

pub(crate) extern "C" fn class_get_class_for_name(
    env: *mut Env<'_>,
    _this: JObject,
    name: JString,
) -> JObject {
    todo!()
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
    let this = this.expect("RegisterNative's class was null");
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
            let (name, info) = env.class_names.name_from_gcid(this_id).unwrap();
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
    let name = construct_string(env, name).unwrap();
    let name = match name {
        ValueException::Value(name) => name,
        ValueException::Exception(_) => todo!(),
    };

    unsafe { env.get_local_jobject_for(name.into_generic()) }
}

// TODO: Could we use &mut Env instead, since we know it will call native methods with a non-null
// ptr?
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

        let mut field_internal_fields =
            match make_fields(env, field_internal_class_id, |field_info| {
                !field_info.access_flags.contains(FieldAccessFlags::STATIC)
            })
            .unwrap()
            {
                Either::Left(fields) => fields,
                Either::Right(_exc) => todo!(),
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

        let mut field_fields = match make_fields(env, field_class_id, |field_info| {
            !field_info.access_flags.contains(FieldAccessFlags::STATIC)
        })
        .unwrap()
        {
            Either::Left(fields) => fields,
            Either::Right(_exc) => todo!(),
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

    unsafe { env.get_local_jobject_for(field_ref.into_generic()) }
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
        ValueException::Exception(_) => todo!("Handle exception in initializing class"),
    };

    let target_class = env.classes.get(&this_id).unwrap();
    if target_class.is_interface()
        || target_class
            .access_flags()
            .contains(ClassAccessFlags::ABSTRACT)
    {
        todo!("InstantiationError exception");
    }

    let fields = match make_fields(env, this_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })
    .unwrap()
    {
        Either::Left(fields) => fields,
        Either::Right(_) => {
            todo!("Exception in making fields for class")
        }
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
        EvalMethodValue::Exception(_) => {
            todo!("There was an exception calling the default constructor")
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

    let package_name_ref = match construct_string(env, package_name).unwrap() {
        ValueException::Value(name) => name,
        ValueException::Exception(_) => todo!("Exception initializing package name"),
    };
    let spec_title_ref = if let Some(val) = spec_title
        .map(|x| construct_string(env, x))
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
        .map(|x| construct_string(env, x))
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
        .map(|x| construct_string(env, x))
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
        .map(|x| construct_string(env, x))
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
        .map(|x| construct_string(env, x))
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
        .map(|x| construct_string(env, x))
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

    let fields = match make_fields(env, package_class_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })
    .unwrap()
    {
        Either::Left(fields) => fields,
        Either::Right(_exc) => todo!(),
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

    let other = unsafe { env.get_jobject_as_gcref(other) };
    let other = other.expect("IsInstance's other class was null");
    let other_id = match env.state.gc.deref(other).unwrap() {
        Instance::StaticClass(_) => todo!(),
        Instance::Reference(re) => re.instanceof(),
    };

    match try_casting(env, other_id, this_id, |_env, _, _, _| {
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
