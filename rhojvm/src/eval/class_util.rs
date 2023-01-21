use std::num::NonZeroUsize;

use rhojvm_base::{
    code::method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
    StepError,
};
use smallvec::smallvec;

use crate::{
    class_instance::{
        ClassInstance, MethodHandleInstance, MethodHandleType, ReferenceArrayInstance,
    },
    exc_value,
    gc::GcRef,
    resolve_derive,
    rv::RuntimeValue,
    util::Env,
    GeneralError,
};

use super::{
    eval_method,
    func::{descriptor_type_to_static_form, opt_descriptor_type_to_static_form},
    EvalError, EvalMethodValue, Frame, Locals, ValueException,
};

pub fn get_init_method_type_from_mh(
    env: &mut Env<'_>,
    mh: GcRef<MethodHandleInstance>,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    // Check if we've already initialized the method type
    let method_type = {
        let mh_inst = env
            .state
            .gc
            .deref(mh)
            .ok_or(EvalError::InvalidGcRef(mh.into_generic()))?;

        if let Some(method_type) = mh_inst.method_type_ref {
            return Ok(ValueException::Value(method_type));
        }

        mh_inst.typ.clone()
    };

    let method_desc = match method_type {
        // TODO: Do we need to initialize based on the id?
        MethodHandleType::InvokeStatic(method_id) => {
            env.methods.get(&method_id).unwrap().descriptor()
        }
    };
    let method_desc = method_desc.clone();

    // We have to initialize the method type

    let method_type_class_id = env
        .class_names
        .gcid_from_bytes(b"java/lang/invoke/MethodType");

    // TODO: deriving from itself is bad
    resolve_derive(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
        &mut env.methods,
        &mut env.state,
        method_type_class_id,
        method_type_class_id,
    )?;

    // methodType(Class<?> returnTy, Class<?>[] paramTys)
    let class_class_id = env.class_names.gcid_from_bytes(b"java/lang/Class");
    let class_array_id = env
        .class_names
        .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), class_class_id)
        .map_err(StepError::BadId)?;
    let desc = MethodDescriptor::new(
        smallvec![
            // returnTy
            DescriptorType::Basic(DescriptorTypeBasic::Class(class_class_id)),
            // paramTys
            DescriptorType::Array {
                level: NonZeroUsize::new(1).unwrap(),
                component: DescriptorTypeBasic::Class(class_class_id),
            }
        ],
        Some(DescriptorType::Basic(DescriptorTypeBasic::Class(
            method_type_class_id,
        ))),
    );
    let method_id = env.methods.load_method_from_desc(
        &mut env.class_names,
        &mut env.class_files,
        method_type_class_id,
        b"methodType",
        &desc,
    )?;

    // Create the actual needed values
    let return_ty = method_desc.return_type();
    let return_ty = opt_descriptor_type_to_static_form(env, return_ty.cloned())?;
    let return_ty = exc_value!(ret: return_ty);

    let mut parameters = Vec::new();
    for param in method_desc.parameters() {
        let param = descriptor_type_to_static_form(env, param.clone())?;
        let param = exc_value!(ret: param);
        parameters.push(Some(param.into_generic()));
    }
    let parameters = ReferenceArrayInstance::new(class_array_id, class_class_id, parameters);

    let frame = Frame::new_locals(Locals::new_with_array([
        RuntimeValue::Reference(return_ty.into_generic()),
        RuntimeValue::Reference(env.state.gc.alloc(parameters).into_generic()),
    ]));

    let inst = match eval_method(env, method_id.into(), frame)? {
        EvalMethodValue::ReturnVoid => panic!("MethodType.methodType() returned void"),
        EvalMethodValue::Return(inst) => inst,
        EvalMethodValue::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    let inst = inst
        .into_reference()
        .expect("MethodType.methodType() returned non-reference")
        .expect("MethodType.methodType() returned null")
        .unchecked_as();

    assert!(
        env.state.gc.deref(inst).is_some(),
        "MethodType.methodType() returned invalid reference"
    );

    let mh_inst = env
        .state
        .gc
        .deref_mut(mh)
        .ok_or(EvalError::InvalidGcRef(mh.into_generic()))?;
    mh_inst.method_type_ref = Some(inst);

    Ok(ValueException::Value(inst))
}
