use std::num::NonZeroUsize;

use classfile_parser::{
    attribute_info::{bootstrap_methods_attribute_parser, InstructionIndex},
    constant_info::ConstantInfo,
    method_info::MethodAccessFlags,
};

use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        op::{InvokeDynamic, InvokeInterface, InvokeSpecial, InvokeStatic, InvokeVirtual},
    },
    data::{
        class_files::ClassFiles,
        class_names::ClassNames,
        classes::Classes,
        methods::{LoadMethodError, Methods},
    },
    id::{ClassId, ExactMethodId, MethodId},
    package::Packages,
    util::Cesu8String,
    StepError,
};
use smallvec::SmallVec;

use crate::{
    class_instance::{
        ClassInstance, MethodHandleInstance, MethodHandleType, ReferenceArrayInstance,
        ReferenceInstance, StaticFormInstance,
    },
    eval::{
        bootstrap::bootstrap_method_arg_to_rv, eval_method, EvalError, EvalMethodValue, Frame,
        Locals, ValueException,
    },
    exc_eval_value, exc_value,
    gc::GcRef,
    initialize_class, map_interface_index_small_vec_to_ids, resolve_derive,
    rv::{RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::{
        construct_string_r, make_class_form_of, make_method_handle, make_primitive_class_form_of,
        ref_info, CallStackEntry, Env,
    },
    GeneralError, State,
};

use super::{RunInstArgsC, RunInstContinue, RunInstContinueValue};

fn grab_runtime_value_from_stack_for_function(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    state: &mut State,
    frame: &mut Frame,
    target: &DescriptorType,
) -> Result<RuntimeValue, GeneralError> {
    let v = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
    Ok(match target {
        DescriptorType::Basic(b) => match b {
            DescriptorTypeBasic::Byte | DescriptorTypeBasic::Boolean => RuntimeValuePrimitive::I8(
                v.into_byte().ok_or(EvalError::ExpectedStackValueIntRepr)?,
            )
            .into(),
            DescriptorTypeBasic::Char => RuntimeValuePrimitive::Char(
                v.into_char().ok_or(EvalError::ExpectedStackValueIntRepr)?,
            )
            .into(),
            DescriptorTypeBasic::Short => RuntimeValuePrimitive::I16(
                v.into_short().ok_or(EvalError::ExpectedStackValueIntRepr)?,
            )
            .into(),
            DescriptorTypeBasic::Int => RuntimeValuePrimitive::I32(
                v.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?,
            )
            .into(),
            DescriptorTypeBasic::Long => {
                RuntimeValuePrimitive::I64(v.into_i64().ok_or(EvalError::ExpectedStackValueLong)?)
                    .into()
            }
            DescriptorTypeBasic::Float => RuntimeValuePrimitive::F32(
                v.into_f32().ok_or(EvalError::ExpectedStackValueIntRepr)?,
            )
            .into(),
            DescriptorTypeBasic::Double => RuntimeValuePrimitive::F64(
                v.into_f64().ok_or(EvalError::ExpectedStackValueIntRepr)?,
            )
            .into(),
            DescriptorTypeBasic::Class(target_id) => match v {
                RuntimeValue::Reference(p_ref) => {
                    let p = state
                        .gc
                        .deref(p_ref)
                        .ok_or(EvalError::InvalidGcRef(p_ref.into_generic()))?;
                    let instance_id = p.instanceof();

                    let is_castable = instance_id == *target_id
                        || classes.is_super_class(
                            class_names,
                            class_files,
                            packages,
                            instance_id,
                            *target_id,
                        )?
                        || classes.implements_interface(
                            class_names,
                            class_files,
                            instance_id,
                            *target_id,
                        )?
                        || classes.is_castable_array(
                            class_names,
                            class_files,
                            packages,
                            instance_id,
                            *target_id,
                        )?;
                    if is_castable {
                        RuntimeValue::Reference(p_ref)
                    } else {
                        todo!(
                            "Type was not castable: {} -> {}",
                            ref_info(class_names, &state.gc, Some(p_ref.into_generic())),
                            class_names.tpath(*target_id)
                        );
                    }
                }

                RuntimeValue::NullReference => RuntimeValue::NullReference,
                RuntimeValue::Primitive(_) => {
                    return Err(EvalError::ExpectedStackValueReference.into())
                }
            },
        },
        DescriptorType::Array { level, component } => {
            let target_id = class_names
                .gcid_from_level_array_of_desc_type_basic(*level, *component)
                .map_err(StepError::BadId)?;
            match v {
                RuntimeValue::Reference(p_ref) => {
                    let p = state
                        .gc
                        .deref(p_ref)
                        .ok_or(EvalError::InvalidGcRef(p_ref.into_generic()))?;
                    let instance_id = p.instanceof();

                    let is_castable = instance_id == target_id
                        || classes.is_super_class(
                            class_names,
                            class_files,
                            packages,
                            instance_id,
                            target_id,
                        )?
                        || classes.implements_interface(
                            class_names,
                            class_files,
                            instance_id,
                            target_id,
                        )?
                        || classes.is_castable_array(
                            class_names,
                            class_files,
                            packages,
                            instance_id,
                            target_id,
                        )?;

                    if is_castable {
                        RuntimeValue::Reference(p_ref)
                    } else {
                        todo!("Type was not castable");
                    }
                }
                RuntimeValue::NullReference => RuntimeValue::NullReference,
                RuntimeValue::Primitive(_) => {
                    return Err(EvalError::ExpectedStackValueReference.into())
                }
            }
        }
    })
}

// TODO: We need a general function for resolving a method
fn find_static_method(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    methods: &mut Methods,
    target_id: ClassId,
    name: &[u8],
    descriptor: &MethodDescriptor,
) -> Result<ExactMethodId, GeneralError> {
    let (_, target_info) = class_names
        .name_from_gcid(target_id)
        .map_err(StepError::BadId)?;
    let target_has_class_file = target_info.has_class_file();

    if target_has_class_file {
        // TODO: This might be too lenient
        let mut current_check_id = target_id;
        loop {
            let method_id = methods.load_method_from_desc(
                class_names,
                class_files,
                current_check_id,
                name,
                descriptor,
            );
            match method_id {
                Ok(method_id) => return Ok(method_id),
                Err(StepError::LoadMethod(LoadMethodError::NonexistentMethodName { .. })) => {
                    // Continue to the super class instance
                    // We assume the class is already loaded
                    let super_id = classes
                        .get(&current_check_id)
                        .ok_or(GeneralError::MissingLoadedClass(current_check_id))?
                        .super_id();
                    if let Some(super_id) = super_id {
                        current_check_id = super_id;
                        continue;
                    }
                    // Break out of the loop since we've checked all the way up the chain
                    break;
                }
                // TODO: Or should we just log the error and skip past it?
                Err(err) => return Err(err.into()),
            }
        }
    } else {
        panic!("InvokeStatic when entry did not have class file");
    }

    Err(
        StepError::LoadMethod(LoadMethodError::NonexistentMethodName {
            class_id: target_id,
            name: Cesu8String(name.to_owned()),
        })
        .into(),
    )
}

fn invoke_static_method(
    env: &mut Env<'_>,
    frame: &mut Frame,
    method_id: ExactMethodId,
    called_from_method_id: MethodId,
    called_at: Option<InstructionIndex>,
) -> Result<RunInstContinueValue, GeneralError> {
    // TODO: We might need to load the method here just in case!
    let method = env
        .methods
        .get(&method_id)
        .ok_or(EvalError::MissingMethod(method_id))?;
    let method_descriptor = method.descriptor().clone();

    let mut locals = Locals::default();
    for parameter in method_descriptor.parameters().iter().rev() {
        let value = grab_runtime_value_from_stack_for_function(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.state,
            frame,
            parameter,
        )?;

        locals.prepush_transform(value);
    }

    let call_frame = Frame::new_locals(locals);

    let cstrack_entry = CallStackEntry {
        called_method: method_id.into(),
        called_from: called_from_method_id,
        called_at,
    };

    // Note: we don't pop the stack entry if there is an error because that lets us know where we
    // were at
    env.call_stack.push(cstrack_entry);
    let res = eval_method(env, method_id.into(), call_frame)?;
    env.call_stack.pop();

    // TODO: Should we have a version that doesn't modify the frame and just returns the
    // EvalMethodValue?
    match res {
        // TODO: Check that these are valid return types
        // We can use the casting code we wrote above for the check, probably?
        EvalMethodValue::ReturnVoid => (),
        EvalMethodValue::Return(v) => frame.stack.push(v)?,
        EvalMethodValue::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
    }

    Ok(RunInstContinueValue::Continue)
}

impl RunInstContinue for InvokeStatic {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            inst_index,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let index = self.index;

        let (class_id, _) = method_id.decompose();
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let (target_class_index, method_nat_index) = {
            let info = class_file
                .get_t(index)
                .ok_or(EvalError::InvalidConstantPoolIndex(index))?;

            match info {
                ConstantInfo::MethodRef(method) => (method.class_index, method.name_and_type_index),
                ConstantInfo::InterfaceMethodRef(method) => {
                    (method.class_index, method.name_and_type_index)
                }
                _ => return Err(EvalError::InvalidInvokeStaticConstantInfo.into()),
            }
        };

        let target_class =
            class_file
                .get_t(target_class_index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    target_class_index.into_generic(),
                ))?;
        let target_class_name = class_file.get_text_b(target_class.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(target_class.name_index.into_generic()),
        )?;
        let target_class_id = env.class_names.gcid_from_bytes(target_class_name);

        let method_nat =
            class_file
                .get_t(method_nat_index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    method_nat_index.into_generic(),
                ))?;
        // TODO: Sadly we have to allocate because load_method_from_desc requires class files and
        // so the slice won't work
        let method_name = class_file
            .get_text_b(method_nat.name_index)
            .ok_or(EvalError::InvalidConstantPoolIndex(
                method_nat.name_index.into_generic(),
            ))?
            .to_owned();
        let method_descriptor = class_file.get_text_b(method_nat.descriptor_index).ok_or(
            EvalError::InvalidConstantPoolIndex(method_nat.descriptor_index.into_generic()),
        )?;
        let method_descriptor =
            MethodDescriptor::from_text(method_descriptor, &mut env.class_names)
                .map_err(EvalError::InvalidMethodDescriptor)?;

        // TODO: Some of these errors should be exceptions
        resolve_derive(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            target_class_id,
            class_id,
        )?;

        // TODO: Some of these errors should be exceptions
        initialize_class(env, target_class_id)?;

        let target_method_id = find_static_method(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.methods,
            target_class_id,
            &method_name,
            &method_descriptor,
        )?;

        invoke_static_method(env, frame, target_method_id, method_id.into(), inst_index)
    }
}

impl RunInstContinue for InvokeInterface {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            inst_index,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (class_id, _) = method_id.decompose();
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let info = class_file
            .get_t(self.index)
            .ok_or(EvalError::InvalidConstantPoolIndex(
                self.index.into_generic(),
            ))?;
        let target_interface_index = info.class_index;
        let method_nat_index = info.name_and_type_index;

        let target_interface =
            class_file
                .get_t(target_interface_index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    target_interface_index.into_generic(),
                ))?;
        let target_interface_name = class_file.get_text_b(target_interface.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(target_interface.name_index.into_generic()),
        )?;
        let target_interface_id = env.class_names.gcid_from_bytes(target_interface_name);

        let method_nat =
            class_file
                .get_t(method_nat_index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    method_nat_index.into_generic(),
                ))?;
        // TODO: Sadly we have to allocate
        let method_name = class_file
            .get_text_b(method_nat.name_index)
            .ok_or(EvalError::InvalidConstantPoolIndex(
                method_nat.name_index.into_generic(),
            ))?
            .to_owned();
        let method_descriptor = class_file.get_text_b(method_nat.descriptor_index).ok_or(
            EvalError::InvalidConstantPoolIndex(method_nat.descriptor_index.into_generic()),
        )?;
        let method_descriptor =
            MethodDescriptor::from_text(method_descriptor, &mut env.class_names)
                .map_err(EvalError::InvalidMethodDescriptor)?;

        // TODO: Some errors should be excpetions
        resolve_derive(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            target_interface_id,
            class_id,
        )?;

        initialize_class(env, target_interface_id)?;

        let mut locals = Locals::default();
        for parameter in method_descriptor.parameters().iter().rev() {
            let value = grab_runtime_value_from_stack_for_function(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.classes,
                &mut env.packages,
                &mut env.state,
                frame,
                parameter,
            )?;

            locals.prepush_transform(value);
        }

        // TODO: Check that this is valid
        // Get the this parameter
        let instance_class = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let instance_ref = instance_class
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?
            .expect("TODO: NullReferenceException");
        let instance = env
            .state
            .gc
            .deref(instance_ref)
            .ok_or(EvalError::InvalidGcRef(instance_ref.into_generic()))?;
        let instance_id = instance.instanceof();
        locals.prepush_transform(RuntimeValue::Reference(instance_ref));

        // Find the actual method to execute
        let target_method_id = find_virtual_method(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.methods,
            target_interface_id,
            instance_id,
            &method_name,
            &method_descriptor,
        )?;

        // TODO: Check if the method is accessible?

        let call_frame = Frame::new_locals(locals);

        let cstack_entry = CallStackEntry {
            called_method: target_method_id,
            called_from: method_id.into(),
            called_at: inst_index,
        };

        // Note: We don't pop the stack entry if there is an error because that lets us know where
        // we were at
        env.call_stack.push(cstack_entry);
        let res = eval_method(env, target_method_id, call_frame)?;
        env.call_stack.pop();

        match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => (),
            EvalMethodValue::Return(v) => frame.stack.push(v)?,
            EvalMethodValue::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
        }

        Ok(RunInstContinueValue::Continue)
    }
}

// FIXME: This code ignores specific actions that it should do
impl RunInstContinue for InvokeSpecial {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            inst_index,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let index = self.index;

        let (class_id, _) = method_id.decompose();
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let info = class_file
            .get_t(index)
            .ok_or(EvalError::InvalidConstantPoolIndex(index))?;

        let (target_class_index, method_nat_index) = match info {
            ConstantInfo::MethodRef(method) => (method.class_index, method.name_and_type_index),
            ConstantInfo::InterfaceMethodRef(method) => {
                (method.class_index, method.name_and_type_index)
            }
            _ => return Err(EvalError::InvalidInvokeStaticConstantInfo.into()),
        };

        let target_class =
            class_file
                .get_t(target_class_index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    target_class_index.into_generic(),
                ))?;
        let target_class_name = class_file.get_text_b(target_class.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(target_class.name_index.into_generic()),
        )?;
        let target_class_id = env.class_names.gcid_from_bytes(target_class_name);

        let method_nat =
            class_file
                .get_t(method_nat_index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    method_nat_index.into_generic(),
                ))?;
        // TODO: Sadly we have to allocate because load_method_from_desc requires class files and
        // so the slice won't work
        let method_name = class_file
            .get_text_b(method_nat.name_index)
            .ok_or(EvalError::InvalidConstantPoolIndex(
                method_nat.name_index.into_generic(),
            ))?
            .to_owned();
        let method_descriptor = class_file.get_text_b(method_nat.descriptor_index).ok_or(
            EvalError::InvalidConstantPoolIndex(method_nat.descriptor_index.into_generic()),
        )?;
        let method_descriptor =
            MethodDescriptor::from_text(method_descriptor, &mut env.class_names)
                .map_err(EvalError::InvalidMethodDescriptor)?;

        // TODO: Some of these errors should be exceptions
        resolve_derive(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            target_class_id,
            class_id,
        )?;

        // TODO: Some of these errors should be exceptions
        initialize_class(env, target_class_id)?;

        let target_method_id = env.methods.load_method_from_desc(
            &mut env.class_names,
            &mut env.class_files,
            target_class_id,
            &method_name,
            &method_descriptor,
        )?;

        let mut locals = Locals::default();

        for parameter in method_descriptor.parameters().iter().rev() {
            let value = grab_runtime_value_from_stack_for_function(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.classes,
                &mut env.packages,
                &mut env.state,
                frame,
                parameter,
            )?;

            locals.prepush_transform(value);
        }

        let instance_class = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let instance_ref = instance_class
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?
            .expect("TODO: NullReferenceException");
        // Push this
        locals.prepush_transform(RuntimeValue::Reference(instance_ref));

        // Construct a frame for the function we're calling and invoke it
        let call_frame = Frame::new_locals(locals);

        let cstack_entry = CallStackEntry {
            called_method: target_method_id.into(),
            called_from: method_id.into(),
            called_at: inst_index,
        };

        // Note: We don't pop the stack entry if there is an error because that lets us know where
        // we were at
        env.call_stack.push(cstack_entry);
        let res = eval_method(env, target_method_id.into(), call_frame)?;
        env.call_stack.pop();

        match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => (),
            EvalMethodValue::Return(v) => frame.stack.push(v)?,
            EvalMethodValue::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
        }

        Ok(RunInstContinueValue::Continue)
    }
}

/// Find  the most specific virtual method
pub fn find_virtual_method(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    methods: &mut Methods,
    base_id: ClassId,
    instance_id: ClassId,
    name: &[u8],
    descriptor: &MethodDescriptor,
) -> Result<MethodId, GeneralError> {
    // We don't bother checking that the instance class has been initialized, since we assume that
    // the caller got a good `GcRef` to it, which would thus keep the static class instance alive.
    // We also don't bother checking that the base_id exists properly since it would have had to be
    // loaded already.

    let (_, instance_info) = class_names
        .name_from_gcid(instance_id)
        .map_err(StepError::BadId)?;
    let instance_has_class_file = instance_info.has_class_file();
    let instance_is_array = instance_info.is_array();

    // This only makes sense to check for things which have a defined class file
    if instance_has_class_file {
        // TODO: This is probably too lenient.
        // TODO: Error if it is an instance initialization method?
        let mut current_check_id = instance_id;
        loop {
            let method_id = methods.load_method_from_desc(
                class_names,
                class_files,
                current_check_id,
                name,
                descriptor,
            );
            match method_id {
                Ok(method_id) => return Ok(method_id.into()),
                Err(StepError::LoadMethod(LoadMethodError::NonexistentMethodName { .. })) => {
                    // Continue to the super class instance
                    // We assume the class is already loaded
                    let super_id = classes
                        .get(&current_check_id)
                        .ok_or(GeneralError::MissingLoadedClass(current_check_id))?
                        .super_id();
                    if let Some(super_id) = super_id {
                        current_check_id = super_id;
                        continue;
                    }
                    // Break out of the loop since we've checked all the way up the chain
                    break;
                }
                // TODO: Or should we just log the error and skip past it?
                Err(err) => return Err(err.into()),
            }
        }
    } else if instance_is_array {
        // Arrays also extend Object, and so have to check its methods
        let object_id = class_names.object_id();
        let method_id =
            methods.load_method_from_desc(class_names, class_files, object_id, name, descriptor);
        match method_id {
            Ok(method_id) => return Ok(method_id.into()),
            Err(StepError::LoadMethod(LoadMethodError::NonexistentMethodName { .. })) => {
                // Silently continue on to checking the interfaces
            }
            // TODO: Or should we just log the error and skip past it?
            Err(err) => return Err(err.into()),
        }

        // otherwise, check interfaces
    }

    // TODO: Does this check the superinterfaces of the superclasses?
    // Check the superinterfaces of instance_id
    if instance_has_class_file {
        let instance_class_file = class_files
            .get(&instance_id)
            .ok_or(GeneralError::MissingLoadedClassFile(instance_id))?;
        let interfaces: SmallVec<[_; 8]> = instance_class_file.interfaces_indices_iter().collect();
        let mut interfaces: SmallVec<[_; 8]> =
            map_interface_index_small_vec_to_ids(class_names, instance_class_file, interfaces)?;

        for interface_id in interfaces {
            if let Some(method_id) = find_interface_method_virtual(
                class_names,
                class_files,
                classes,
                methods,
                name,
                descriptor,
                interface_id,
            )? {
                return Ok(method_id);
            }
        }

        // TODO: Is this the right ordering? The docs don't explicitly mention when it should call the
        // base class version..
        // If we simply look up the chain, then we'd always find the base class version before we bother
        // checking the interfaces?
        Ok(methods
            .load_method_from_desc(class_names, class_files, base_id, name, descriptor)?
            .into())
    } else if instance_is_array {
        if name == b"clone" {
            Ok(MethodId::ArrayClone)
        } else {
            Err(
                StepError::LoadMethod(LoadMethodError::NonexistentMethodName {
                    class_id: instance_id,
                    name: Cesu8String(name.to_owned()),
                })
                .into(),
            )
        }
    } else {
        panic!("When trying to invoke a virtual function, the instance ({:?}) inherently did not have a class file while also was not an array.", class_names.name_from_gcid(instance_id));
    }
}
/// Tries finding a method on an interface
/// Implementation detail of [`find_virtual_method`]
fn find_interface_method_virtual(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    methods: &mut Methods,
    name: &[u8],
    descriptor: &MethodDescriptor,
    interface_id: ClassId,
) -> Result<Option<MethodId>, GeneralError> {
    let method_id =
        methods.load_method_from_desc(class_names, class_files, interface_id, name, descriptor);
    match method_id {
        Ok(method_id) => {
            // While we got a method that matches, we first need to check that it is not
            // abstract.
            let method = methods
                .get(&method_id)
                .ok_or(GeneralError::MissingLoadedMethod(method_id.into()))?;
            if !method.access_flags().contains(MethodAccessFlags::ABSTRACT) {
                return Ok(Some(method_id.into()));
            }

            // TODO: If it is abstract should we really be checking the superinterfaces of this interface?
        }
        Err(StepError::LoadMethod(LoadMethodError::NonexistentMethodName { .. })) => {
            // Ignore
        }
        // TODO: Or should we just log the error and skip past it?
        Err(err) => return Err(err.into()),
    }

    // TODO: Should we be assuming that the interface is loaded?
    // Currently, this is fine, but if we want to dispose of class files temp then it wouldn't be

    // TODO: Is there any cases where they could be cyclic interface implementations?

    // TODO: We could probably do better, such as not constructing a smallvec at all
    // TODO: We could do better by not checking them if they've already been checked?

    // Check the interfaces this interface extends
    let interface_class_file = class_files
        .get(&interface_id)
        .ok_or(GeneralError::MissingLoadedClassFile(interface_id))?;
    let interface_interfaces: SmallVec<[_; 8]> =
        interface_class_file.interfaces_indices_iter().collect();
    let interface_interfaces: SmallVec<[_; 8]> = map_interface_index_small_vec_to_ids(
        class_names,
        interface_class_file,
        interface_interfaces,
    )?;
    // Check each of the interfaces this interface extends for the method
    for interface_interface_id in interface_interfaces {
        if let Some(method_id) = find_interface_method_virtual(
            class_names,
            class_files,
            classes,
            methods,
            name,
            descriptor,
            interface_interface_id,
        )? {
            return Ok(Some(method_id));
        }
    }

    // Failed to find the method
    Ok(None)
}

impl RunInstContinue for InvokeVirtual {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            inst_index,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        // TODO: Validate that the instance ref can be considered to extend the class that the
        // virtual method is on.
        //
        let index = self.index;

        let (class_id, _) = method_id.decompose();

        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let info = class_file
            .get_t(index)
            .ok_or(EvalError::InvalidConstantPoolIndex(index))?;

        let (target_class_index, method_nat_index) = match info {
            ConstantInfo::MethodRef(method) => (method.class_index, method.name_and_type_index),
            ConstantInfo::InterfaceMethodRef(method) => {
                (method.class_index, method.name_and_type_index)
            }
            _ => return Err(EvalError::InvalidInvokeStaticConstantInfo.into()),
        };

        let target_class =
            class_file
                .get_t(target_class_index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    target_class_index.into_generic(),
                ))?;
        let target_class_name = class_file.get_text_b(target_class.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(target_class.name_index.into_generic()),
        )?;
        let target_class_id = env.class_names.gcid_from_bytes(target_class_name);

        let method_nat =
            class_file
                .get_t(method_nat_index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    method_nat_index.into_generic(),
                ))?;
        // TODO: Sadly we have to allocate because load_method_from_desc requires class files and
        // so the slice won't work
        let method_name = class_file
            .get_text_b(method_nat.name_index)
            .ok_or(EvalError::InvalidConstantPoolIndex(
                method_nat.name_index.into_generic(),
            ))?
            .to_owned();
        let method_descriptor = class_file.get_text_b(method_nat.descriptor_index).ok_or(
            EvalError::InvalidConstantPoolIndex(method_nat.descriptor_index.into_generic()),
        )?;
        let method_descriptor =
            MethodDescriptor::from_text(method_descriptor, &mut env.class_names)
                .map_err(EvalError::InvalidMethodDescriptor)?;

        // TODO: Some of these errors should be exceptions
        resolve_derive(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            target_class_id,
            class_id,
        )?;

        // TODO: Some of these errors should be exceptions
        initialize_class(env, target_class_id)?;

        // We have to make the locals before getting the target method id since we need the instance

        let mut locals = Locals::default();

        for parameter in method_descriptor.parameters().iter().rev() {
            let value = grab_runtime_value_from_stack_for_function(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.classes,
                &mut env.packages,
                &mut env.state,
                frame,
                parameter,
            )?;

            locals.prepush_transform(value);
        }

        // TODO: Check that this is valid
        // Get the this parameter
        let instance_class = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let instance_ref = instance_class
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?
            .expect(
                "TODO: NullReferenceException. This happens if the `this` of a function is null.",
            );
        let instance = env
            .state
            .gc
            .deref(instance_ref)
            .ok_or(EvalError::InvalidGcRef(instance_ref.into_generic()))?;
        let instance_id = instance.instanceof();
        locals.prepush_transform(RuntimeValue::Reference(instance_ref));

        // Find the actual target method to execute
        let target_method_id = find_virtual_method(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.methods,
            target_class_id,
            instance_id,
            &method_name,
            &method_descriptor,
        )?;

        // TODO: Check if the method is accessible?

        let call_frame = Frame::new_locals(locals);

        let cstack_entry = CallStackEntry {
            called_method: target_method_id,
            called_from: method_id.into(),
            called_at: inst_index,
        };

        // Note: We don't pop the stack entry if there is an error because that lets us know where
        // we were at
        env.call_stack.push(cstack_entry);
        let res = eval_method(env, target_method_id, call_frame)?;
        env.call_stack.pop();

        match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => (),
            EvalMethodValue::Return(v) => frame.stack.push(v)?,
            EvalMethodValue::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
        }

        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for InvokeDynamic {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            inst_index,
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        // The general idea behind InvokeDynamic is, well, invoking a function dynamically.
        // A Java program typically always knows one of:
        // - InvokeStatic: The literal static function that you're calling (ex a static method)
        // - InvokeVirtual: The virtual function that you're calling (ex a method on an object,
        // since you may have an instance of a class which *extends* the type you 'know')
        // - InvokeInterface: Relatively similar to invoke virtual
        // However, these require you to know the signature of the function you're calling at
        // compile time. You won't run into a 'this function does not exist' error at runtime, like
        // you might in javascript or python.
        //
        // What InvokeDynamic does is have an index into the bootstrap method table.
        // When you first execute the InvokeDynamic operation, you look up the bootstrap method.
        // The bootstrap method gives you back a `CallSite` instance, which is forevermore
        // associated with this specific InvokeDynamic instruction.
        // However, what the `CallSite` does is have a `MethodHandle` within it, which as the name
        // suggests, it can reference some method. This method is the actual method that will be
        // called when you execute the InvokeDynamic instruction.
        //
        // Sometimes that is *it*. You do it once, and then forevermore you refer to the same
        // function. This helps the JVM have special optimizations, like for string building,
        // that can still apply to code compiled for older versions of the JVM.
        // Because the JVM implements the code to choose the `MethodHandle`, and so it doesn't have
        // to rely on the bytecode being a certain specific way. (Or, rather, it has chosen this
        // specific way for writing a way for a large class of problems to be improved in the
        // future)
        //
        // However, the `CallSite`'s `MethodHandle` can be changed out for a new function.
        // A language runtime, like if you were running JRuby, might do this. It could have
        // different functions for different types of inputs, and so it could swap out the
        // `MethodHandle` for those.

        // FIXME: We aren't checking whether this invoke dynamic instruction already has a `CallSite` instance, which is a big problem!

        // Index to the data about the invoke dynamic instruction
        let index = self.index;

        // Get the class file we're operating within
        let (class_id, _) = method_id.decompose();
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        // Get the information about the Invoke dynamic instruction
        let inv_dyn = class_file.getr(index)?.clone();

        // TODO: This should be a function on class file data
        // Load the boot strap methods table and parse it
        let bootstrap_attr = {
            // TODO: Cache the parsed bootstrap methods table
            let data_range = class_file
                .load_attribute_range_with_name("BootstrapMethods")
                .ok_or(EvalError::NoBootstrapTable)?;
            let (data_rem, bootstrap_attr) =
                bootstrap_methods_attribute_parser(class_file.parse_data_for(data_range))
                    .map_err(|_| EvalError::InvalidBootstrapTable)?;
            debug_assert!(data_rem.is_empty());
            bootstrap_attr
        };

        let call_site = {
            // Get the bootstrap method from the table, this is the function we'd invoke to get a
            // `CallSite`
            let b_method = bootstrap_attr
                .bootstrap_methods
                .get(usize::from(inv_dyn.bootstrap_method_attr_index))
                .ok_or(EvalError::InvalidBootstrapTableIndex(
                    inv_dyn.bootstrap_method_attr_index,
                ))?;

            let method_handle = {
                // The method handle instance that will serve as the bootstrap method
                let method_handle_con = class_file.getr(b_method.bootstrap_method_ref)?.clone();
                // construct the bootstrap method handle instance
                exc_value!(ret inst: make_method_handle(env, class_id, &method_handle_con)?)
            };

            // TODO: method type?
            // References to bootstrap method arguments
            let mut bargs_rv = Vec::with_capacity(b_method.bootstrap_arguments.len());
            // TODO: Don't clone
            for barg_idx in b_method.bootstrap_arguments.clone() {
                bargs_rv.push(
                    exc_value!(ret inst: bootstrap_method_arg_to_rv(env, class_id, barg_idx)?),
                );
            }

            // Reget the class file
            let class_file = env
                .class_files
                .get(&class_id)
                .ok_or(EvalError::MissingMethodClassFile(class_id))?;

            // I'm relatively sure this is the method type?
            let inv_nat = class_file.getr(inv_dyn.name_and_type_index)?;
            let inv_name = class_file.getr_text(inv_nat.name_index)?.into_owned();

            let method_type = {
                let inv_desc = class_file.getr_text_b(inv_nat.descriptor_index)?;
                let inv_desc = MethodDescriptor::from_text(inv_desc, &mut env.class_names)
                    .map_err(EvalError::InvalidMethodDescriptor)?;

                exc_value!(ret inst: make_method_type(env, &inv_desc)?)
            };

            // Get the instance of the bootstrap method
            let mh_inst = env.state.gc.deref(method_handle).unwrap();
            tracing::info!("MH Inst: {:?}", mh_inst);

            match mh_inst.typ {
                MethodHandleType::Constant { value, .. } => {
                    if let Some(value) = value {
                        value
                    } else {
                        todo!("NPE?")
                    }
                }
                MethodHandleType::InvokeStatic(method_id) => {
                    let method = env.methods.get(&method_id).unwrap();

                    let desc = method.descriptor();

                    // Ensure that the bootstrap method returns a `CallSite` instance.
                    // TODO: What if it returns something which extends a `CallSite`?
                    {
                        let callsite_class_id = env
                            .class_names
                            .gcid_from_bytes(b"java/lang/invoke/CallSite");

                        if let Some(DescriptorType::Basic(DescriptorTypeBasic::Class(
                            ret_class_id,
                        ))) = desc.return_type()
                        {
                            if *ret_class_id != callsite_class_id {
                                tracing::error!("Bootstrap method handle returned a class that wasn't a callsite: {:?}", env.class_names.tpath(*ret_class_id));
                                panic!(
                                "Bootstrap method handle returned a class that wasn't a callsite"
                            );
                            }
                        } else {
                            tracing::error!(
                                "Bootstrap method handle returned a non-class type {:?}",
                                desc.return_type()
                            );
                            panic!("Bootstrap method handle returned a non-class type");
                        }
                    }

                    // The MethodHandles$Lookup reference
                    // This is expected to be the first argument to the bootstrap method
                    let lookup = {
                        // Load the method handles class
                        let mh_class_id = env
                            .class_names
                            .gcid_from_bytes(b"java/lang/invoke/MethodHandles");
                        env.class_files
                            .load_by_class_path_id(&mut env.class_names, mh_class_id)
                            .map_err(StepError::from)?;
                        // Get the id for the lookup class
                        let mh_lookup_class_id = env
                            .class_names
                            .gcid_from_bytes(b"java/lang/invoke/MethodHandles$Lookup");

                        // TODO: A reference to lookup could be stored in a field to make this more efficient
                        let lookup_method_id = {
                            // Create the descriptor for the lookup method
                            let method_handles_lookup_desc =
                                MethodDescriptor::new_ret(DescriptorType::Basic(
                                    DescriptorTypeBasic::Class(mh_lookup_class_id),
                                ));
                            env.methods.load_method_from_desc(
                                &mut env.class_names,
                                &mut env.class_files,
                                mh_class_id,
                                b"lookup",
                                &method_handles_lookup_desc,
                            )?
                        };

                        let frame = Frame::default();
                        exc_eval_value!(ret inst (expect_return: reference)
                                ("Method Handle lookup"):
                                eval_method(env, lookup_method_id.into(), frame)?
                        )
                        .expect("Null method handle lookup reference")
                    };
                    let call_site_name =
                        exc_value!(ret inst: construct_string_r(env, inv_name.as_ref())?);
                    // we have the method type already

                    // there are additional parameters
                    // ex:
                    // - (Ljava/long/Object;)Z is an erased method signature
                    // - The REF_invokeStatic Main.lambda$main$o:(Ljava/lang/String;)Z is the MethodHandle to the actual lambda logic
                    // - The (Ljava/lang/String;)Z is a non-erased method signature accepting one string and returning a boolean
                    // The jvm will pass all the info to the bootstrap method
                    // The bootstrap method will then use that info to create an instance of Predicate (our callsite?)
                    // then the jvm will pass that to the filter method

                    let frame = Frame::new_locals({
                        let mut locals = Locals::new_with_array([
                            RuntimeValue::Reference(lookup),
                            RuntimeValue::Reference(call_site_name.into_generic()),
                            RuntimeValue::Reference(method_type.into_generic()),
                        ]);

                        for barg in bargs_rv {
                            locals.push_transform(barg);
                        }

                        locals
                    });

                    let call_site = exc_eval_value!(ret inst (expect_return: reference)
                            ("Bootstrap method"):
                            eval_method(env, method_id.into(), frame)?
                    )
                    .unwrap();

                    // TODO: Store the call site on the ?class?, ?method?, ?general cache?

                    call_site
                }
            }
        };

        let call_site_class_id = env
            .class_names
            .gcid_from_bytes(b"java/lang/invoke/CallSite");

        // Get the underlying method handle in the call site
        let target = {
            let get_target_method_id = {
                let method_handle_class_id = env
                    .class_names
                    .gcid_from_bytes(b"java/lang/invoke/MethodHandle");
                let method_handle_desc = MethodDescriptor::new_ret(DescriptorType::Basic(
                    DescriptorTypeBasic::Class(method_handle_class_id),
                ));

                let call_site_instance_id = env.state.gc.deref(call_site).unwrap().instanceof();

                find_virtual_method(
                    &mut env.class_names,
                    &mut env.class_files,
                    &mut env.classes,
                    &mut env.methods,
                    call_site_class_id,
                    call_site_instance_id,
                    b"getTarget",
                    &method_handle_desc,
                )?
            };

            let frame = Frame::new_locals(Locals::new_with_array([RuntimeValue::Reference(
                call_site.into_generic(),
            )]));

            let cstack_entry = CallStackEntry {
                called_method: get_target_method_id.into(),
                called_from: method_id.into(),
                called_at: inst_index,
            };

            env.call_stack.push(cstack_entry);
            let target = eval_method(env, get_target_method_id.into(), frame)?;
            env.call_stack.pop();

            let target = exc_eval_value!(ret inst (expect_return: reference)
                ("CallSite getTarget"):
                target
            )
            .expect("Null call site target");

            target
        };

        let target = target.unchecked_as::<MethodHandleInstance>();
        let Some(target_inst) = env.state.gc.deref(target) else {
            tracing::error!("Call site target was nonexistent/null. Might be due to a bad return value?");
            panic!("Call site target was null");
        };

        match &target_inst.typ {
            MethodHandleType::Constant { value, .. } => {
                tracing::info!(
                    "Invoking constant method handle: {}",
                    ref_info(
                        &env.class_names,
                        &env.state.gc,
                        value.map(GcRef::unchecked_as)
                    )
                );
                if let Some(value) = value {
                    frame.stack.push(RuntimeValue::Reference(*value))?;
                } else {
                    frame.stack.push(RuntimeValue::NullReference)?;
                }
                Ok(RunInstContinueValue::Continue)
            }
            MethodHandleType::InvokeStatic(inv_method_id) => {
                tracing::info!("Invoking static method");
                let res =
                    invoke_static_method(env, frame, *inv_method_id, method_id.into(), inst_index);
                tracing::info!("Finished static method");
                res
            }
        }
    }
}

pub(crate) fn constant_info_to_rv(
    env: &mut Env,
    class_id: ClassId,
    inf: &ConstantInfo,
) -> Result<ValueException<RuntimeValue>, GeneralError> {
    let class_file = env
        .class_files
        .get(&class_id)
        .ok_or(EvalError::MissingMethodClassFile(class_id))?;
    match inf {
        ConstantInfo::Utf8(_) => todo!(),
        ConstantInfo::Integer(_) => todo!(),
        ConstantInfo::Float(_) => todo!(),
        ConstantInfo::Long(_) => todo!(),
        ConstantInfo::Double(_) => todo!(),
        ConstantInfo::Class(_) => todo!(),
        ConstantInfo::String(_) => todo!(),
        ConstantInfo::FieldRef(_) => todo!(),
        ConstantInfo::MethodRef(_) => todo!(),
        ConstantInfo::InterfaceMethodRef(_) => todo!(),
        ConstantInfo::NameAndType(_) => todo!(),
        ConstantInfo::MethodHandle(method_handle_c) => {
            Ok(make_method_handle(env, class_id, method_handle_c)?
                .map(|v| RuntimeValue::Reference(v.into_generic())))
        }
        ConstantInfo::MethodType(method_typ) => {
            let descriptor = class_file.get_text_b(method_typ.descriptor_index).ok_or(
                EvalError::InvalidConstantPoolIndex(method_typ.descriptor_index.into_generic()),
            )?;
            let descriptor = MethodDescriptor::from_text(descriptor, &mut env.class_names)
                .map_err(EvalError::InvalidMethodDescriptor)?;
            Ok(make_method_type(env, &descriptor)?
                .map(|v| RuntimeValue::Reference(v.into_generic())))
        }
        ConstantInfo::InvokeDynamic(_) => todo!(),
        ConstantInfo::Unusable => todo!(),
    }
}

/// Make a jvm`MethodType` instance from a `MethodDescriptor`
fn make_method_type(
    env: &mut Env,
    descriptor: &MethodDescriptor,
) -> Result<ValueException<GcRef<ClassInstance>>, GeneralError> {
    // Get the class information
    let class_class_id = env.class_names.gcid_from_bytes(b"java/lang/Class");
    let class_array_id = env
        .class_names
        .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), class_class_id)
        .map_err(StepError::BadId)?;

    let method_type_id = env
        .class_names
        .gcid_from_bytes(b"java/lang/invoke/MethodType");

    // TODO: Invalid usage of resolve derive because we are acting like we are resolving it from itself
    resolve_derive(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
        &mut env.methods,
        &mut env.state,
        method_type_id,
        method_type_id,
    )?;

    // Initialize the class properly
    let _static_method_type = match initialize_class(env, method_type_id)?.into_value() {
        ValueException::Value(re) => re,
        ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
    };

    // Calling methodType function (note that it isn't a normal constructor!)
    // with signatures (Class<?> ret, Class<?>[]parameters) -> MethodType
    let locals = {
        let mut locals = Locals::default();

        if let Some(return_type) = descriptor.return_type() {
            let form = match descriptor_type_to_static_form(env, *return_type)? {
                ValueException::Value(form) => form,
                ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
            };

            locals.push_transform(RuntimeValue::Reference(form.into_generic()));
        }

        let mut parameters = Vec::new();
        for parameter in descriptor.parameters() {
            let form = match descriptor_type_to_static_form(env, *parameter)? {
                ValueException::Value(form) => form,
                ValueException::Exception(exc) => return Ok(ValueException::Exception(exc)),
            };
            let form = Some(form.unchecked_as::<ReferenceInstance>());

            parameters.push(form);
        }
        let array = ReferenceArrayInstance::new(class_array_id, class_class_id, parameters);
        let array_ref = env.state.gc.alloc(array);
        locals.push_transform(RuntimeValue::Reference(array_ref.into_generic()));

        locals
    };

    let frame = Frame::new_locals(locals);

    let method_type_descriptor = MethodDescriptor::new(
        smallvec::smallvec![
            DescriptorType::Basic(DescriptorTypeBasic::Class(class_class_id)),
            DescriptorType::Array {
                level: NonZeroUsize::new(1).unwrap(),
                component: DescriptorTypeBasic::Class(class_class_id)
            }
        ],
        Some(DescriptorType::Basic(DescriptorTypeBasic::Class(
            method_type_id,
        ))),
    );

    let method_id = env.methods.load_method_from_desc(
        &mut env.class_names,
        &mut env.class_files,
        method_type_id,
        b"methodType",
        &method_type_descriptor,
    )?;

    match eval_method(env, method_id.into(), frame)? {
        EvalMethodValue::ReturnVoid => panic!("Constructor returned nothing"),
        EvalMethodValue::Return(inst) => {
            // TODO: Don't unwrap
            let inst = inst.into_reference().unwrap();
            let inst = inst.expect("Got a null pointer from methodType constructor");

            // It has to have returned a class instance because that's what the descriptor specified
            Ok(ValueException::Value(inst.unchecked_as()))
        }
        EvalMethodValue::Exception(exc) => Ok(ValueException::Exception(exc)),
    }
}

pub(crate) fn opt_descriptor_type_to_static_form(
    env: &mut Env,
    typ: Option<DescriptorType>,
) -> Result<ValueException<GcRef<StaticFormInstance>>, GeneralError> {
    match typ {
        Some(typ) => descriptor_type_to_static_form(env, typ),
        None => make_primitive_class_form_of(env, None),
    }
}

pub(crate) fn descriptor_type_to_static_form(
    env: &mut Env,
    typ: DescriptorType,
) -> Result<ValueException<GcRef<StaticFormInstance>>, GeneralError> {
    match typ {
        DescriptorType::Basic(basic) => match basic {
            DescriptorTypeBasic::Byte => todo!(),
            DescriptorTypeBasic::Char => todo!(),
            DescriptorTypeBasic::Double => todo!(),
            DescriptorTypeBasic::Float => todo!(),
            DescriptorTypeBasic::Int => todo!(),
            DescriptorTypeBasic::Long => todo!(),
            DescriptorTypeBasic::Class(class_id) => {
                // TODO: Incorrect usage of make_class_form_of
                make_class_form_of(env, class_id, class_id)
            }
            DescriptorTypeBasic::Short => todo!(),
            DescriptorTypeBasic::Boolean => {
                make_primitive_class_form_of(env, Some(RuntimeTypePrimitive::Bool))
            }
        },
        DescriptorType::Array { level, component } => {
            let id = env
                .class_names
                .gcid_from_level_array_of_desc_type_basic(level, component)
                .map_err(StepError::BadId)?;
            // TODO: Incorrect usage of make_class_form_of
            make_class_form_of(env, id, id)
        }
    }
}
