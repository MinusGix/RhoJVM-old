use crate::{
    class_instance::{ClassInstance, ReferenceArrayInstance, StaticFormInstance},
    jni::JObject,
    util::{find_field_with_name, Env},
};

pub(crate) extern "C" fn mt_to_method_descriptor_string(env: *mut Env, this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this_ref = unsafe { env.get_jobject_as_gcref(this) };
    let this_ref = this_ref.expect("Null reference");
    let this_ref = this_ref.unchecked_as::<ClassInstance>();

    let mt_class_id = env
        .class_names
        .gcid_from_bytes(b"java/lang/invoke/MethodType");

    // TODO: Cache this?
    let (return_ty_field_id, _) = find_field_with_name(&env.class_files, mt_class_id, b"returnTy")
        .unwrap()
        .expect("Failed to find returnTy field in MethodType class");
    let (param_tys_field_id, _) = find_field_with_name(&env.class_files, mt_class_id, b"paramTys")
        .unwrap()
        .expect("Failed to find paramTys field in MethodType class");

    let this = env.state.gc.deref(this_ref).unwrap();

    let return_ty = this
        .fields
        .get(return_ty_field_id)
        .expect("Failed to get returnTy field from MethodType instance");
    let param_tys = this
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
        .checked_as::<StaticFormInstance>(&env.state.gc)
        .expect("MethodType#returnTy is not a StaticFormInstance");
    let param_tys = param_tys
        .value()
        .into_reference()
        .expect("MethodType#paramTys is not a reference")
        .expect("MethodType#paramTys was null")
        .checked_as::<ReferenceArrayInstance>(&env.state.gc)
        .expect("MethodType#paramTys is not a ReferenceArrayInstance");

    JObject::null()
}
