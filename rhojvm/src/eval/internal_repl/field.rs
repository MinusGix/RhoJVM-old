use classfile_parser::{
    descriptor::DescriptorType as DescriptorTypeCF,
    field_info::{FieldAccessFlags, FieldInfoOpt},
};

use rhojvm_base::{
    code::method::DescriptorType, data::class_files::ClassFiles, id::ClassId, StepError,
};
use smallvec::SmallVec;

use crate::{
    class_instance::{ClassInstance, FieldId, FieldIndex, Instance, ReferenceInstance},
    eval::{
        internal_repl::class::{
            BOOLEAN_NAME, BYTE_NAME, CHARACTER_NAME, DOUBLE_NAME, FLOAT_NAME, INTEGER_NAME,
            LONG_NAME, SHORT_NAME,
        },
        EvalError,
    },
    gc::{Gc, GcRef},
    initialize_class,
    jni::{JClass, JObject},
    rv::{RuntimeType, RuntimeTypePrimitive},
    util::{construct_string_r, make_class_form_of, rv_into_object, Env},
    GeneralError,
};

struct InternalFieldWrapper {
    internal_field_ref: GcRef<ClassInstance>,
    class_id_field: FieldId,
    field_index_field: FieldId,
}
impl InternalFieldWrapper {
    fn from_internal_field_ref(
        env: &mut Env,
        internal_field_ref: GcRef<ReferenceInstance>,
    ) -> InternalFieldWrapper {
        let field_internal_class_id = env.class_names.gcid_from_bytes(b"rho/InternalField");

        let (class_id_field, field_index_field, _) = env
            .state
            .get_internal_field_ids(&env.class_files, field_internal_class_id)
            .unwrap();
        let internal_field_ref = env.state.gc.checked_as(internal_field_ref).unwrap();

        InternalFieldWrapper {
            internal_field_ref,
            class_id_field,
            field_index_field,
        }
    }

    fn get_off_field(env: &mut Env, field_ref: GcRef<Instance>) -> InternalFieldWrapper {
        let field_class_id = env.class_names.gcid_from_bytes(b"java/lang/reflect/Field");
        let internal_field_id = env
            .state
            .get_field_internal_field_id(&env.class_files, field_class_id)
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

        InternalFieldWrapper::from_internal_field_ref(env, internal_field_ref)
    }

    fn class_id(&self, gc: &Gc) -> ClassId {
        let internal_field = gc.deref(self.internal_field_ref).unwrap();
        let class_id_val = internal_field
            .fields
            .get(self.class_id_field)
            .expect("class id field should exist")
            .value()
            .into_i32()
            .expect("classid field should be i32");
        ClassId::new_unchecked(class_id_val as u32)
    }

    fn field_index(&self, gc: &Gc) -> FieldIndex {
        let internal_field = gc.deref(self.internal_field_ref).unwrap();
        let field_index_val = internal_field
            .fields
            .get(self.field_index_field)
            .expect("field index field should exist")
            .value()
            .into_i16()
            .expect("field index field should be i16");
        FieldIndex::new_unchecked(field_index_val as u16)
    }

    fn field_id(&self, gc: &Gc) -> FieldId {
        let class_id = self.class_id(gc);
        let field_index = self.field_index(gc);
        FieldId::unchecked_compose(class_id, field_index)
    }

    fn find_field_info(&self, class_files: &ClassFiles, gc: &Gc) -> FieldInfoOpt {
        let class_id = self.class_id(gc);
        let field_index = self.field_index(gc);
        let class_file = class_files
            .get(&class_id)
            .expect("class file should exist for class id");
        let (field_info, _) = class_file
            .load_field_values_iter()
            .enumerate()
            .find(|x| x.0 == usize::from(field_index.get()))
            .map(|x| x.1)
            .transpose()
            .ok()
            .flatten()
            .unwrap();
        field_info
    }
}

pub(crate) extern "C" fn field_get_type(env: *mut Env<'_>, field: JObject) -> JClass {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let field_ref = unsafe { env.get_jobject_as_gcref(field) };
    // TODO: null pointer exception
    let field_ref = field_ref.expect("objectFieldOffset's field ref was null");

    let internal_field = InternalFieldWrapper::get_off_field(env, field_ref);

    let class_id = internal_field.class_id(&env.state.gc);
    let field_index = internal_field.field_index(&env.state.gc);

    let class_file = env
        .class_files
        .get(&class_id)
        .ok_or(EvalError::MissingMethodClassFile(class_id))
        .unwrap();

    let field_iter = class_file
        .load_field_values_iter()
        .collect::<SmallVec<[_; 8]>>();
    let (field_info, _) = field_iter
        .into_iter()
        .enumerate()
        .find(|x| x.0 == usize::from(field_index.get()))
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

pub(crate) extern "C" fn field_get(env: *mut Env<'_>, field: JObject, obj: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let field_ref = unsafe { env.get_jobject_as_gcref(field) };
    let field_ref = field_ref.unwrap();

    let obj = unsafe { env.get_jobject_as_gcref(obj) };

    let internal_field = InternalFieldWrapper::get_off_field(env, field_ref);

    let class_id = internal_field.class_id(&env.state.gc);
    let field_id = internal_field.field_id(&env.state.gc);

    // TODO: access control

    let info = internal_field.find_field_info(&env.class_files, &env.state.gc);
    let inst_ref: GcRef<Instance> = if info.access_flags.contains(FieldAccessFlags::STATIC) {
        // We ignore the obj

        // Initialize the class the field is for
        let res = initialize_class(env, class_id).unwrap().into_value();
        let Some(res) = env.state.extract_value(res) else {
            return JObject::null();
        };

        res.into_generic()
    } else {
        let obj = obj.expect("TODO: NPE");
        let obj = obj.unchecked_as::<ReferenceInstance>();

        let inst = env.state.gc.deref(obj).unwrap();
        if inst.instanceof() != class_id {
            todo!("IllegalArgumentException")
        }

        obj.into_generic()
    };

    let inst = env.state.gc.deref(inst_ref).unwrap();
    let (_, field) = inst.fields().find(|(id, _)| *id == field_id).unwrap();

    let value = field.value();
    let value = rv_into_object(env, value).unwrap();
    let Some(value) = env.state.extract_value(value) else {
        return JObject::null();
    };

    if let Some(value) = value {
        unsafe { env.get_local_jobject_for(value.into_generic()) }
    } else {
        JObject::null()
    }
}

pub(crate) extern "C" fn internal_field_get_name(env: *mut Env<'_>, field: JObject) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    let field_ref = unsafe { env.get_jobject_as_gcref(field) };
    let field_ref = field_ref.unwrap().unchecked_as();

    let internal_field = InternalFieldWrapper::from_internal_field_ref(env, field_ref);

    let class_id = internal_field.class_id(&env.state.gc);

    let field_info = internal_field.find_field_info(&env.class_files, &env.state.gc);

    let class_file = env.class_files.get(&class_id).unwrap();

    let name = class_file
        .getr_text(field_info.name_index)
        .unwrap()
        .into_owned();
    // TODO: this conversion could go directly from cesu8 to utf16
    let string_ref = construct_string_r(env, &name, true).unwrap();

    let Some(string_ref) = env.state.extract_value(string_ref) else {
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(string_ref.into_generic()) }
}
