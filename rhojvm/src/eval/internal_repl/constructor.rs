use rhojvm_base::{
    data::class_files::ClassFiles,
    id::{ClassId, ExactMethodId},
};

use crate::{
    class_instance::{ClassInstance, FieldId, ReferenceArrayInstance, StaticFormInstance},
    eval::{eval_method, instances::make_instance_fields, EvalMethodValue, Frame, Locals},
    gc::GcRef,
    initialize_class,
    jni::JObject,
    rv::{RuntimeValue, RuntimeValuePrimitive},
    util::{find_field_with_name, ref_info, Env},
};

const CLASS_FIELD_NAME: &[u8] = b"clazz";
const METHOD_IDX_FIELD_NAME: &[u8] = b"methodIndex";

/// Returns (class_field_id, method_idx_field_id)
fn get_field_ids(class_files: &ClassFiles, constructor_id: ClassId) -> (FieldId, FieldId) {
    let Some((class_field_id, _)) =
        find_field_with_name(class_files, constructor_id, CLASS_FIELD_NAME).unwrap() else {
        panic!("class field not found");
    };

    let Some((method_idx_field_id, _)) =
        find_field_with_name(class_files, constructor_id, METHOD_IDX_FIELD_NAME).unwrap() else {
        panic!("method index field not found");
    };

    (class_field_id, method_idx_field_id)
}

pub(crate) extern "C" fn constructor_new_instance(
    env: *mut Env<'_>,
    this: JObject,
    args: JObject,
) -> JObject {
    assert!(!env.is_null());
    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) };
    let this = this.expect("this is null");
    let this = this.unchecked_as::<ClassInstance>();

    let args = unsafe { env.get_jobject_as_gcref(args) };

    let constructor_id = env
        .class_names
        .gcid_from_bytes(b"java/lang/reflect/Constructor");

    let (class_field_id, method_idx_field_id) = get_field_ids(&env.class_files, constructor_id);

    // TODO: We're maybe supposed to check whether the caller has permission for calling this constructor?

    let this = env.state.gc.deref(this).unwrap();

    let class_id = {
        let class = this.fields.get(class_field_id).unwrap();
        let Some(class) = class.value().into_reference().unwrap() else {
        panic!("Constructor's class ref is null");
    };
        let class = class.unchecked_as::<StaticFormInstance>();
        let class = env.state.gc.deref(class).unwrap();
        class.of.into_reference().unwrap()
    };

    let method_idx = {
        let method_idx = this.fields.get(method_idx_field_id).unwrap();
        let RuntimeValue::Primitive(RuntimeValuePrimitive::I16(method_idx)) = method_idx.value() else {
            unreachable!()
        };
        u16::from_be_bytes(i16::to_be_bytes(method_idx))
    };

    let method_id = ExactMethodId::unchecked_compose(class_id, method_idx);

    tracing::info!("Args: {}", ref_info(env, args));

    let args = args
        .map(GcRef::unchecked_as::<ReferenceArrayInstance>)
        .map(|args| env.state.gc.deref(args).unwrap())
        .map(|args| &args.elements);

    // TODO: newInstance is supposed to convert primitive object wrappers, like `Integer`, to primitive values if that is what is needed for the constructor.

    if let Some(args) = args {
        if !args.is_empty() {
            todo!("Constructor#newInstance had args, which isn't currently implemented");
        }
    } else {
        // whatever
    }

    let class_static_ref = initialize_class(env, constructor_id).unwrap().into_value();
    let Some(class_static_ref) = env.state.extract_value(class_static_ref) else {
        todo!();
    };

    // TODO: Do we need to do something special if they're constructing classes which we manage internally? We could have a general 'create class' function which does it?
    let fields = make_instance_fields(env, class_id).unwrap();
    let Some(fields) = env.state.extract_value(fields) else {
        todo!();
    };

    let instance = ClassInstance::new(class_id, class_static_ref, fields);
    let instance = env.state.gc.alloc(instance);

    let locals = Locals::new_with_array([RuntimeValue::Reference(instance.into_generic())]);
    let frame = Frame::new_locals(locals);

    match eval_method(env, method_id.into(), frame).unwrap() {
        EvalMethodValue::ReturnVoid => {}
        EvalMethodValue::Return(_) => tracing::warn!("Init method returned a value"),
        EvalMethodValue::Exception(_) => todo!(),
    }

    unsafe { env.get_local_jobject_for(instance.into_generic()) }
}
