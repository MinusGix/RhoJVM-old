use usize_cast::IntoUsize;

use crate::{
    class_instance::{Instance, PrimitiveArrayInstance, ReferenceArrayInstance, ReferenceInstance},
    gc::GcRef,
    initialize_class,
    jni::{JClass, JInt, JObject},
    rv::RuntimeTypeVoid,
    util::Env,
};

pub(crate) extern "C" fn array_new_instance(
    env: *mut Env,
    _: JClass,
    component: JClass,
    length: JInt,
) -> JObject {
    assert!(!env.is_null(), "Env was null. Internal bug?");
    let env = unsafe { &mut *env };

    assert!(length >= 0, "Negative length");
    let length = length.unsigned_abs();
    let length = length.into_usize();

    let component = unsafe { env.get_jobject_as_gcref(component) };
    let component = component.expect("NPE");

    let comp_of = match env.state.gc.deref(component).unwrap() {
        Instance::StaticClass(_) => unreachable!(),
        Instance::Reference(re) => match re {
            ReferenceInstance::StaticForm(form) => form.of,
            _ => panic!("Expected Class<T>"),
        },
    };
    let array_ref = match comp_of {
        RuntimeTypeVoid::Primitive(rv_prim) => {
            let prim = rv_prim.into_primitive_type();
            let array_id = env
                .classes
                .load_array_of_primitives(&mut env.class_names, prim)
                .unwrap();
            let mut elements = Vec::new();
            elements.resize(length, rv_prim.default_value());
            let array_inst = PrimitiveArrayInstance::new(array_id, rv_prim, elements);
            env.state.gc.alloc(array_inst).into_generic()
        }
        RuntimeTypeVoid::Reference(comp_id) => {
            let _status = initialize_class(env, comp_id).unwrap();
            let array_id = env
                .classes
                .load_array_of_instances(
                    &mut env.class_names,
                    &mut env.class_files,
                    &mut env.packages,
                    comp_id,
                )
                .unwrap();
            let mut elements: Vec<Option<GcRef<ReferenceInstance>>> = Vec::new();
            elements.resize(length, None);
            let array_inst = ReferenceArrayInstance::new(array_id, comp_id, elements);
            env.state.gc.alloc(array_inst).into_generic()
        }
        RuntimeTypeVoid::Void => panic!("Trying to create a void[]"),
    };

    unsafe { env.get_local_jobject_for(array_ref) }
}
