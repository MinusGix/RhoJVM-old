use rhojvm_base::{
    data::{class_files::ClassFiles, class_names::ClassNames},
    util::Cesu8Str,
};

use crate::{
    class_instance::{ClassInstance, ReferenceArrayInstance, StaticFormInstance},
    gc::{Gc, GcRef},
    jni::JObject,
    rv::{RuntimeTypePrimitive, RuntimeTypeVoid},
    util::{construct_string_r, find_field_with_name, Env},
};

/// Convert a `java/lang/String` instance into a method descriptor string  
/// Panic heavy.
pub(crate) fn method_type_to_desc_string(
    class_names: &mut ClassNames,
    class_files: &ClassFiles,
    gc: &Gc,
    target: GcRef<ClassInstance>,
) -> String {
    let mt_class_id = class_names.gcid_from_bytes(b"java/lang/invoke/MethodType");

    // TODO: Cache this?
    let (return_ty_field_id, _) = find_field_with_name(class_files, mt_class_id, b"returnTy")
        .unwrap()
        .expect("Failed to find returnTy field in MethodType class");
    let (param_tys_field_id, _) = find_field_with_name(class_files, mt_class_id, b"paramTys")
        .unwrap()
        .expect("Failed to find paramTys field in MethodType class");

    let target = gc.deref(target).unwrap();

    let return_ty = target
        .fields
        .get(return_ty_field_id)
        .expect("Failed to get returnTy field from MethodType instance");
    let param_tys = target
        .fields
        .get(param_tys_field_id)
        .expect("Failed to get paramTys field from MethodType instance");

    // TODO: These panics could be replaced with some function/macro that automatically does them
    // and just does formats, so if we do this a lot then it won't fill the binary with strings as
    // much
    let return_ty = return_ty
        .value()
        .into_reference()
        .expect("MethodType#returnTy is not a reference")
        .expect("MethodType#returnTy was null")
        .checked_as::<StaticFormInstance>(gc)
        .expect("MethodType#returnTy is not a StaticFormInstance");
    let param_tys = param_tys
        .value()
        .into_reference()
        .expect("MethodType#paramTys is not a reference")
        .expect("MethodType#paramTys was null")
        .checked_as::<ReferenceArrayInstance>(gc)
        .expect("MethodType#paramTys is not a ReferenceArrayInstance");

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
            let (name, info) = class_names.name_from_gcid(*class_id).unwrap();
            if info.is_anonymous() {
                panic!("Don't know how to use anonymous class in method descriptor string");
            }

            out.push('L');
            out.push_str(&format!("{}", Cesu8Str(name.get())));
            out.push(';');
        }
    }
}
