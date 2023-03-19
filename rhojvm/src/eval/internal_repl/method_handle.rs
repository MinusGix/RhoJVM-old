use rhojvm_base::{
    code::method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
    data::{class_files::ClassFiles, class_names::ClassNames},
};
use smallvec::smallvec;

use crate::{
    class_instance::{
        ClassInstance, Field, FieldId, Fields, MethodHandleInfoInstance, MethodHandleInstance,
        MethodHandleType, ReferenceInstance, StaticFormInstance,
    },
    eval::{
        eval_method,
        instances::make_instance_fields,
        internal_repl::method_type::{
            make_method_type_ref_vec, method_type_to_desc_string, MethodTypeWrapper,
        },
        EvalMethodValue, Frame, Locals,
    },
    gc::{Gc, GcRef},
    initialize_class,
    jni::{JClass, JObject},
    resolve_derive,
    rv::{RuntimeType, RuntimeTypeVoid, RuntimeValue},
    util::{
        construct_method_handle, find_field_with_name, get_string_contents_as_rust_string,
        make_class_form_of, make_type_class_form_of, Env,
    },
};

struct LookupWrapper {
    pub target: GcRef<ClassInstance>,
    pub referent_field: FieldId,
}
impl LookupWrapper {
    pub fn from_ref(
        class_names: &mut ClassNames,
        class_files: &ClassFiles,
        target: GcRef<ClassInstance>,
    ) -> LookupWrapper {
        let lookup_class_id = class_names.gcid_from_bytes(b"java/lang/invoke/MethodHandles$Lookup");

        let (referent_field_id, _) =
            find_field_with_name(class_files, lookup_class_id, b"referent")
                .unwrap()
                .expect("Failed to find referent field in Lookup class");

        LookupWrapper {
            target,
            referent_field: referent_field_id,
        }
    }

    pub fn referent_field<'gc>(&self, gc: &'gc Gc) -> &'gc Field {
        let target = gc.deref(self.target).unwrap();
        target
            .fields
            .get(self.referent_field)
            .expect("Failed to get referent field from Lookup instance")
    }

    pub fn referent_ref(&self, gc: &Gc) -> GcRef<ReferenceInstance> {
        let referent = self.referent_field(gc);
        referent
            .value()
            .into_reference()
            .expect("Lookup#referent should be a reference")
            .expect("Lookup#referent was null")
    }
}

pub(crate) extern "C" fn mh_lookup_reveal_direct(
    env: *mut Env,
    _lookup: JObject,
    target: JObject,
) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let target: GcRef<_> = unsafe { env.get_jobject_as_gcref(target) }.expect("target was null");
    assert!(env.state.gc.deref(target).is_some());
    let target: GcRef<MethodHandleInstance> = target.unchecked_as();
    assert!(env.state.gc.deref(target).is_some());

    // TODO: Throw exception if target is not a direct method handle
    // TODO: exception if access checking fails
    // TODO: exception if security manager refuses

    let mh_info_id = env
        .class_names
        .gcid_from_bytes(b"rho/invoke/MethodHandleInfoInst");

    // TODO: Deriving from itself is bad
    resolve_derive(env, mh_info_id, mh_info_id).unwrap();

    let mh_info_static_ref = initialize_class(env, mh_info_id).unwrap().into_value();
    let mh_info_static_ref = if let Some(re) = env.state.extract_value(mh_info_static_ref) {
        re
    } else {
        // exception
        return JObject::null();
    };

    // We assume that there are no fields to initialize
    let class_instance = ClassInstance::new(mh_info_id, mh_info_static_ref, Fields::default());
    let mh_info_instance = MethodHandleInfoInstance::new(class_instance, target);

    let mh_info_ref = env.state.gc.alloc(mh_info_instance);

    unsafe { env.get_local_jobject_for(mh_info_ref.into_generic()) }
}

pub(crate) extern "C" fn mhs_lookup_lookup_class(env: *mut Env, this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) }.expect("this was null");
    let this = this.unchecked_as::<ClassInstance>();

    let lookup = LookupWrapper::from_ref(&mut env.class_names, &env.class_files, this);

    let referent = lookup.referent_ref(&env.state.gc);

    unsafe { env.get_local_jobject_for(referent.into_generic()) }
}

pub(crate) extern "C" fn mhs_lookup(env: *mut Env, _: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let cstack_entry = &env.call_stack[env.call_stack.len() - 1];
    let Some((caller_class_id, _)) = cstack_entry.called_from.decompose() else {
        panic!("MethodHandles.Lookup#lookup called from non-normal method");
    };

    let mhl_class_id = env
        .class_names
        .gcid_from_bytes(b"java/lang/invoke/MethodHandles$Lookup");

    let class_inst = make_class_form_of(env, mhl_class_id, caller_class_id).unwrap();
    let Some(class_inst) = env.state.extract_value(class_inst) else {
        // exception
        return JClass::null();
    };

    let class_class_id = env.class_names.gcid_from_bytes(b"java/lang/Class");

    let lookup_static_ref = initialize_class(env, mhl_class_id).unwrap().into_value();
    let Some(lookup_static_ref) = env.state.extract_value(lookup_static_ref) else {
        // exception
        return JClass::null();
    };

    let lookup_fields = make_instance_fields(env, mhl_class_id).unwrap();
    let Some(lookup_fields) = env.state.extract_value(lookup_fields) else {
        // exception
        return JClass::null();
    };

    let lookup_desc = MethodDescriptor::new(
        smallvec![DescriptorType::Basic(DescriptorTypeBasic::Class(
            class_class_id
        ))],
        None,
    );

    let lookup_init = env
        .methods
        .load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            mhl_class_id,
            b"<init>",
            &lookup_desc,
        )
        .unwrap();

    let lookup_inst = ClassInstance {
        instanceof: mhl_class_id,
        static_ref: lookup_static_ref,
        fields: lookup_fields,
    };
    let lookup_inst = env.state.gc.alloc(lookup_inst);

    let frame = Frame::new_locals(Locals::new_with_array([
        RuntimeValue::Reference(lookup_inst.into_generic()),
        RuntimeValue::Reference(class_inst.into_generic()),
    ]));

    let res = eval_method(env, lookup_init.into(), frame).unwrap();
    match res {
        EvalMethodValue::ReturnVoid => unsafe {
            env.get_local_jobject_for(lookup_inst.into_generic())
        },
        EvalMethodValue::Return(_) => unreachable!(),
        EvalMethodValue::Exception(exc) => {
            env.state.fill_native_exception(exc);
            JObject::null()
        }
    }
}

pub(crate) unsafe extern "C" fn mhs_lookup_find_static(
    env: *mut Env,
    _this: JObject,
    target: JObject,
    name: JObject,
    method_type: JObject,
) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let target: GcRef<_> = unsafe { env.get_jobject_as_gcref(target) }.unwrap();
    let target: GcRef<StaticFormInstance> = target.unchecked_as();
    let target_class_id = env
        .state
        .gc
        .deref(target)
        .unwrap()
        .of
        .into_reference()
        .unwrap();

    let name: GcRef<_> = unsafe { env.get_jobject_as_gcref(name) }.unwrap();
    let name = get_string_contents_as_rust_string(
        &env.class_files,
        &mut env.class_names,
        &mut env.state,
        name,
    )
    .unwrap();

    let method_type = unsafe { env.get_jobject_as_gcref(method_type) }.unwrap();
    let method_type = method_type.unchecked_as();
    // TODO: We could more directly convert it to a MethodDescriptor
    let desc = method_type_to_desc_string(
        &mut env.class_names,
        &env.class_files,
        &env.state.gc,
        method_type,
    );

    let desc = MethodDescriptor::from_text(desc.as_bytes(), &mut env.class_names).unwrap();

    // TODO: check that the class and arg types are accessible to the lookup inst
    let method_id = env
        .methods
        .load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            target_class_id,
            // TODO: CESU8
            name.as_bytes(),
            &desc,
        )
        .unwrap();

    // TODO: variable arity flag?
    let method_handle =
        construct_method_handle(env, MethodHandleType::InvokeStatic(method_id)).unwrap();
    let Some(method_handle) = env.state.extract_value(method_handle) else {
        // exception
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(method_handle.into_generic()) }
}

/// `MethodHandle constant(Class<?> type, Object value)`
pub(crate) extern "C" fn mhs_constant(
    env: *mut Env<'_>,
    _this: JClass,
    typ: JObject,
    value: JObject,
) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let typ_ref = unsafe { env.get_jobject_as_gcref(typ) }.expect("type was null");
    let typ_ref: GcRef<StaticFormInstance> = env.state.gc.checked_as(typ_ref).unwrap();

    let value = unsafe { env.get_jobject_as_gcref(value) };
    let value = value.map(GcRef::unchecked_as);

    let typ_of = env.state.gc.deref(typ_ref).unwrap().of;
    let typ_of = match typ_of {
        RuntimeTypeVoid::Primitive(_) => todo!("We need to actually verify that the class is a primitive wrapper"),
        RuntimeTypeVoid::Void => todo!("IllegalArgumentException. Cannot create a constant-returning function with return type void"),
        RuntimeTypeVoid::Reference(class_id) => {
            // TODO: Check that the value we have actually extends the class_id
            RuntimeType::Reference(class_id)
        },
    };

    let mh = construct_method_handle(
        env,
        MethodHandleType::Constant {
            value,
            return_ty: typ_of,
        },
    )
    .unwrap();
    let Some(mh) = env.state.extract_value(mh) else {
        // exception
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(mh.into_generic()) }
}

pub(crate) extern "C" fn mh_inst_type(env: *mut Env<'_>, this: JObject) -> JObject {
    assert!(!env.is_null());

    let env = unsafe { &mut *env };

    let this = unsafe { env.get_jobject_as_gcref(this) }.unwrap();
    let this: GcRef<MethodHandleInstance> = this.unchecked_as();

    let this = env.state.gc.deref(this).unwrap();
    let this_typ = &this.typ;

    let method_type = match this_typ {
        MethodHandleType::Constant { return_ty, .. } => {
            // Convert the RuntimeType instance into a Class<?>
            let return_ty_class = make_type_class_form_of(env, Some(*return_ty)).unwrap();
            let Some(return_ty_class) = env.state.extract_value(return_ty_class) else {
                return JObject::null();
            };
            make_method_type_ref_vec(env, return_ty_class, Vec::new())
        }
        MethodHandleType::InvokeStatic(method_id)
        | MethodHandleType::InvokeInterface(method_id) => {
            let method = env.methods.get(method_id).unwrap();
            let (class_id, _) = method_id.decompose();
            // TODO: don't clone
            let desc = method.descriptor().clone();
            // Get the `this` reference type if it is an instance method
            let self_ref: Option<GcRef<ReferenceInstance>> = match this_typ {
                MethodHandleType::InvokeStatic(_) => None,
                MethodHandleType::InvokeInterface(_) => {
                    let re = make_type_class_form_of(env, Some(RuntimeType::Reference(class_id)))
                        .unwrap();
                    let Some(re) = env.state.extract_value(re) else {
                        return JObject::null();
                    };

                    Some(re.into_generic())
                }
                MethodHandleType::Constant { .. } => unreachable!(),
            };

            let return_ty = desc
                .return_type()
                .map(|ty| RuntimeType::from_descriptor_type(&mut env.class_names, *ty).unwrap());
            let return_ty_class = make_type_class_form_of(env, return_ty).unwrap();
            let Some(return_ty_class) = env.state.extract_value(return_ty_class) else {
                return JObject::null();
            };

            let params_ty = desc.parameters().iter().map(|ty| {
                let ty = RuntimeType::from_descriptor_type(&mut env.class_names, *ty).unwrap();
                let ty = make_type_class_form_of(env, Some(ty)).unwrap();
                let Some(ty) = env.state.extract_value(ty) else {
                        todo!("handle this appropriately")
                    };
                ty.into_generic()
            });
            let params_ty = self_ref.into_iter().chain(params_ty).map(Some).collect();

            make_method_type_ref_vec(env, return_ty_class, params_ty)
        }
    }
    .unwrap();

    let Some(method_type) = env.state.extract_value(method_type) else {
        return JObject::null();
    };

    unsafe { env.get_local_jobject_for(method_type.into_generic()) }
}
