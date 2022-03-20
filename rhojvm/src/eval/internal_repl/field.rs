use classfile_parser::descriptor::DescriptorType as DescriptorTypeCF;

use rhojvm_base::{code::method::DescriptorType, id::ClassId, StepError};
use smallvec::SmallVec;

use crate::{
    class_instance::{FieldIndex, Instance, ReferenceInstance},
    eval::{
        internal_repl::class::{
            BOOLEAN_NAME, BYTE_NAME, CHARACTER_NAME, DOUBLE_NAME, FLOAT_NAME, INTEGER_NAME,
            LONG_NAME, SHORT_NAME,
        },
        EvalError,
    },
    jni::{JClass, JObject},
    rv::{RuntimeType, RuntimeTypePrimitive},
    util::{make_class_form_of, Env},
    GeneralError,
};

pub(crate) extern "C" fn field_get_type(env: *mut Env<'_>, field: JObject) -> JClass {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let field_ref = unsafe { env.get_jobject_as_gcref(field) };
    // TODO: null pointer exception
    let field_ref = field_ref.expect("objectFieldOffset's field ref was null");

    let field_class_id = env.class_names.gcid_from_bytes(b"java/lang/reflect/Field");
    let field_internal_class_id = env.class_names.gcid_from_bytes(b"rho/InternalField");
    let internal_field_id = env
        .state
        .get_field_internal_field_id(&env.class_files, field_class_id)
        .unwrap();
    let (class_id_field, field_index_field, _) = env
        .state
        .get_internal_field_ids(&env.class_files, field_internal_class_id)
        .unwrap();

    let field = env.state.gc.deref(field_ref).unwrap();
    let field = if let Instance::Reference(ReferenceInstance::Class(field)) = field {
        field
    } else {
        panic!("Bad field reference");
    };
    let internal_field_ref = field
        .fields
        .get(internal_field_id)
        .unwrap()
        .value()
        .into_reference()
        .expect("internal field should be a reference")
        .expect("Got null ptr for internal field");

    let internal_field = env.state.gc.deref(internal_field_ref).unwrap();
    let internal_field = if let ReferenceInstance::Class(field) = internal_field {
        field
    } else {
        panic!("Bad internal field reference");
    };

    // TODO: Various parts of this are used in other places
    // and should be extracted to helper functions

    let class_id_val = internal_field
        .fields
        .get(class_id_field)
        .expect("class id field should exist")
        .value()
        .into_i32()
        .expect("classid field should be i32");
    let class_id_val = ClassId::new_unchecked(class_id_val as u32);

    let field_index_val = internal_field
        .fields
        .get(field_index_field)
        .expect("field index field should exist")
        .value()
        .into_i16()
        .expect("field index field should be i16");
    let field_index_val = FieldIndex::new_unchecked(field_index_val as u16);

    let class_file = env
        .class_files
        .get(&class_id_val)
        .ok_or(EvalError::MissingMethodClassFile(class_id_val))
        .unwrap();

    let field_iter = class_file
        .load_field_values_iter()
        .collect::<SmallVec<[_; 8]>>();
    let (field_info, _) = field_iter
        .into_iter()
        .enumerate()
        .find(|x| x.0 == usize::from(field_index_val.get()))
        .map(|x| x.1)
        .transpose()
        .ok()
        .flatten()
        .unwrap();

    let field_descriptor = class_file
        .get_text_b(field_info.descriptor_index)
        .ok_or(GeneralError::BadClassFileIndex(
            field_info.descriptor_index.into_generic(),
        ))
        .unwrap();
    // Parse the type of the field
    let (field_type, rem) = DescriptorTypeCF::parse(field_descriptor)
        .map_err(GeneralError::InvalidDescriptorType)
        .unwrap();
    // There shouldn't be any remaining data.
    if !rem.is_empty() {
        panic!();
    }
    // Convert to alternative descriptor type
    let field_type = DescriptorType::from_class_file_desc(&mut env.class_names, field_type);
    let field_type: RuntimeType<ClassId> =
        RuntimeType::from_descriptor_type(&mut env.class_names, field_type)
            .map_err(StepError::BadId)
            .unwrap();
    let field_class_id = match field_type {
        RuntimeType::Primitive(prim) => {
            let name = match prim {
                RuntimeTypePrimitive::I64 => LONG_NAME,
                RuntimeTypePrimitive::I32 => INTEGER_NAME,
                RuntimeTypePrimitive::I16 => SHORT_NAME,
                RuntimeTypePrimitive::I8 => BYTE_NAME,
                RuntimeTypePrimitive::Bool => BOOLEAN_NAME,
                RuntimeTypePrimitive::F32 => FLOAT_NAME,
                RuntimeTypePrimitive::F64 => DOUBLE_NAME,
                RuntimeTypePrimitive::Char => CHARACTER_NAME,
            };
            env.class_names.gcid_from_bytes(name)
        }
        RuntimeType::Reference(re) => re,
    };

    // TODO: Bad usage of make class form of
    let form = make_class_form_of(env, field_class_id, field_class_id).unwrap();
    if let Some(form) = env.state.extract_value(form) {
        unsafe { env.get_local_jobject_for(form.into_generic()) }
    } else {
        // Exception
        JClass::null()
    }
}
