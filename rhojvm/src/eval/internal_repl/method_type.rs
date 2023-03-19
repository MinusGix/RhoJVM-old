use std::num::NonZeroUsize;

use rhojvm_base::{
    data::{class_files::ClassFiles, class_names::ClassNames},
    util::Cesu8Str,
};

use crate::{
    class_instance::{
        ClassInstance, Field, FieldId, ReferenceArrayInstance, ReferenceInstance,
        StaticFormInstance,
    },
    eval::{instances::make_instance_fields, ValueException},
    exc_value,
    gc::{Gc, GcRef},
    initialize_class,
    jni::JObject,
    rv::{RuntimeTypePrimitive, RuntimeTypeVoid, RuntimeValue},
    util::{construct_string_r, find_field_with_name, Env},
    GeneralError,
};

pub(crate) struct MethodTypeWrapper {
    pub target: GcRef<ClassInstance>,
    pub return_ty_field: FieldId,
    pub param_tys_field: FieldId,
}
impl MethodTypeWrapper {
    pub fn return_ty_field_id(class_names: &mut ClassNames, class_files: &ClassFiles) -> FieldId {
        let mt_class_id = class_names.gcid_from_bytes(b"java/lang/invoke/MethodType");
        let (return_ty_field_id, _) = find_field_with_name(class_files, mt_class_id, b"returnTy")
            .unwrap()
            .expect("Failed to find returnTy field in MethodType class");

        return_ty_field_id
    }

    pub fn param_tys_field_id(class_names: &mut ClassNames, class_files: &ClassFiles) -> FieldId {
        let mt_class_id = class_names.gcid_from_bytes(b"java/lang/invoke/MethodType");
        let (param_tys_field_id, _) = find_field_with_name(class_files, mt_class_id, b"paramTys")
            .unwrap()
            .expect("Failed to find paramTys field in MethodType class");

        param_tys_field_id
    }

    pub fn from_ref(
        class_names: &mut ClassNames,
        class_files: &ClassFiles,
        target: GcRef<ClassInstance>,
    ) -> MethodTypeWrapper {
        let mt_class_id = class_names.gcid_from_bytes(b"java/lang/invoke/MethodType");

        // TODO: Cache this?
        let (return_ty_field_id, _) = find_field_with_name(class_files, mt_class_id, b"returnTy")
            .unwrap()
            .expect("Failed to find returnTy field in MethodType class");
        let (param_tys_field_id, _) = find_field_with_name(class_files, mt_class_id, b"paramTys")
            .unwrap()
            .expect("Failed to find paramTys field in MethodType class");

        MethodTypeWrapper {
            target,
            return_ty_field: return_ty_field_id,
            param_tys_field: param_tys_field_id,
        }
    }

    pub fn return_ty_field<'gc>(&self, gc: &'gc Gc) -> &'gc Field {
        let target = gc.deref(self.target).unwrap();
        target
            .fields
            .get(self.return_ty_field)
            .expect("Failed to get returnTy field from MethodType instance")
    }

    /// `void.class` for void return type  
    /// otherwise a normal `Class` instance
    pub fn return_ty_ref(&self, gc: &Gc) -> GcRef<StaticFormInstance> {
        let return_ty = self.return_ty_field(gc);
        return_ty
            .value()
            .into_reference()
            .expect("MethodType#returnTy should be a reference")
            .expect("MethodType#returnTy was null")
            .checked_as(gc)
            .expect("MethodType#returnTy was not a StaticFormInstance")
    }

    pub fn param_tys_field<'gc>(&self, gc: &'gc Gc) -> &'gc Field {
        let target = gc.deref(self.target).unwrap();
        target
            .fields
            .get(self.param_tys_field)
            .expect("Failed to get paramTys field from MethodType instance")
    }

    pub fn param_tys_ref(&self, gc: &Gc) -> GcRef<ReferenceArrayInstance> {
        let param_tys = self.param_tys_field(gc);
        param_tys
            .value()
            .into_reference()
            .expect("MethodType#paramTys should be a reference")
            .expect("MethodType#paramTys was null")
            .checked_as(gc)
            .expect("MethodType#paramTys was not a ReferenceArrayInstance")
    }
}

/// Convert a `java/lang/String` instance into a method descriptor string  
/// Panic heavy.
pub(crate) fn method_type_to_desc_string(
    class_names: &mut ClassNames,
    class_files: &ClassFiles,
    gc: &Gc,
    target: GcRef<ClassInstance>,
) -> String {
    let target = MethodTypeWrapper::from_ref(class_names, class_files, target);

    let return_ty = target.return_ty_ref(gc);
    let param_tys = target.param_tys_ref(gc);

    // TODO: Could we do this directly in utf16?
    let mut out = "(".to_string();

    let class_class_id = class_names.gcid_from_bytes(b"java/lang/Class");
    let param_tys = gc.deref(param_tys).unwrap();
    assert_eq!(param_tys.element_type, class_class_id);
    for param_ty in &param_tys.elements {
        let param_ty = param_ty.unwrap();
        let param_ty = param_ty.checked_as::<StaticFormInstance>(gc).unwrap();
        static_form_instance_to_desc(class_names, gc, param_ty, &mut out, false);
    }
    out.push(')');

    static_form_instance_to_desc(class_names, &gc, return_ty, &mut out, true);

    out
}

pub(crate) extern "C" fn mt_to_method_descriptor_string(env: *mut Env, this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this_ref = unsafe { env.get_jobject_as_gcref(this) };
    let this_ref = this_ref.expect("Null reference");
    let this_ref = this_ref.unchecked_as::<ClassInstance>();

    let out = method_type_to_desc_string(
        &mut env.class_names,
        &env.class_files,
        &env.state.gc,
        this_ref,
    );

    let res = construct_string_r(env, &out, true).unwrap();
    let Some(res) = env.state.extract_value(res) else {
        return JObject::null();
    };
    let res = res.into_generic();

    unsafe { env.get_local_jobject_for(res) }
}

fn static_form_instance_to_desc(
    class_names: &ClassNames,
    gc: &Gc,
    sf: GcRef<StaticFormInstance>,
    out: &mut String,
    is_return: bool,
) {
    let sf = gc.deref(sf).unwrap();

    match &sf.of {
        RuntimeTypeVoid::Primitive(prim) => match prim {
            RuntimeTypePrimitive::I64 => out.push('J'),
            RuntimeTypePrimitive::I32 => out.push('I'),
            RuntimeTypePrimitive::I16 => out.push('S'),
            RuntimeTypePrimitive::I8 => out.push('B'),
            RuntimeTypePrimitive::Bool => out.push('Z'),
            RuntimeTypePrimitive::F32 => out.push('F'),
            RuntimeTypePrimitive::F64 => out.push('D'),
            RuntimeTypePrimitive::Char => out.push('C'),
        },
        RuntimeTypeVoid::Void => {
            if is_return {
                out.push('V');
            } else {
                // TODO: This could be an exception? But I think the constructors for MethodType should
                // disallow this, so panicking is mostly fine
                panic!("MethodType#paramTys contains a Class<void> instance");
            }
        }
        RuntimeTypeVoid::Reference(class_id) => {
            let (name, _) = class_names.name_from_gcid(*class_id).unwrap();

            out.push('L');
            out.push_str(&format!("{}", Cesu8Str(name.get())));
            out.push(';');
        }
    }
}

/// Construct a `java/lang/invoke/MethodType` instance.  
/// `return_ty` is the `Class<?>` return type. Should be `void.class` if it returns nothing.  
/// `params` is a list of `Class<?>` parameters.
pub(crate) fn make_method_type_ref_vec(
    env: &mut Env,
    return_ty: GcRef<StaticFormInstance>,
    params: Vec<Option<GcRef<ReferenceInstance>>>,
) -> Result<ValueException<GcRef<ReferenceInstance>>, GeneralError> {
    let method_type_id = env
        .class_names
        .gcid_from_bytes(b"java/lang/invoke/MethodType");

    let static_method_type = exc_value!(ret: initialize_class(env, method_type_id)?.into_value());

    let mut method_type_fields = exc_value!(ret: make_instance_fields(env, method_type_id)?);

    let return_ty_field_id =
        MethodTypeWrapper::return_ty_field_id(&mut env.class_names, &env.class_files);
    let param_tys_field_id =
        MethodTypeWrapper::param_tys_field_id(&mut env.class_names, &env.class_files);

    *method_type_fields
        .get_mut(return_ty_field_id)
        .unwrap()
        .value_mut() = RuntimeValue::Reference(return_ty.into_generic());

    let class_class_id = env.class_names.gcid_from_bytes(b"java/lang/Class");
    let params_id = env
        .class_names
        .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), class_class_id)
        .unwrap();
    let params = ReferenceArrayInstance::new(params_id, class_class_id, params);
    let params_ref = env.state.gc.alloc(params);

    *method_type_fields
        .get_mut(param_tys_field_id)
        .unwrap()
        .value_mut() = RuntimeValue::Reference(params_ref.into_generic());

    // TODO: this is skipping calling the constructor. The constructor does some extra verification that we should be doing. Either we should call the constructor or we should manually do the same verification.
    let method_type_inst = ClassInstance {
        instanceof: method_type_id,
        static_ref: static_method_type,
        fields: method_type_fields,
    };

    let method_type_ref = env.state.gc.alloc(method_type_inst);

    Ok(ValueException::Value(method_type_ref.into_generic()))
}
