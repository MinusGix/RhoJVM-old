use classfile_parser::{constant_info::ConstantInfo, method_info::MethodAccessFlags};
use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic, MethodDescriptor},
        op::{InvokeDynamic, InvokeInterface, InvokeSpecial, InvokeStatic, InvokeVirtual},
    },
    id::{ClassId, MethodId},
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, LoadMethodError, Methods, StepError,
};
use smallvec::SmallVec;

use crate::{
    class_instance::ReferenceInstance,
    eval::{eval_method, EvalError, EvalMethodValue, Frame, Locals},
    initialize_class, map_interface_index_small_vec_to_ids, resolve_derive,
    rv::{RuntimeValue, RuntimeValuePrimitive},
    GeneralError, State,
};

use super::{RunInst, RunInstArgs, RunInstValue};

fn grab_runtime_value_from_stack_for_function(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    frame: &mut Frame,
    target: &DescriptorType,
) -> Result<RuntimeValue, GeneralError> {
    Ok(match target {
        DescriptorType::Basic(b) => {
            let v = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
            match b {
                DescriptorTypeBasic::Byte | DescriptorTypeBasic::Boolean => {
                    RuntimeValuePrimitive::I8(
                        v.into_byte().ok_or(EvalError::ExpectedStackValueIntRepr)?,
                    )
                    .into()
                }
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
                DescriptorTypeBasic::Long => RuntimeValuePrimitive::I64(
                    v.into_i64().ok_or(EvalError::ExpectedStackValueLong)?,
                )
                .into(),
                DescriptorTypeBasic::Float => RuntimeValuePrimitive::F32(
                    v.into_f32().ok_or(EvalError::ExpectedStackValueIntRepr)?,
                )
                .into(),
                DescriptorTypeBasic::Double => RuntimeValuePrimitive::F64(
                    v.into_f64().ok_or(EvalError::ExpectedStackValueIntRepr)?,
                )
                .into(),
                DescriptorTypeBasic::Class(id) => match v {
                    RuntimeValue::Reference(p_ref) => {
                        let p = state
                            .gc
                            .deref(p_ref)
                            .ok_or(EvalError::InvalidGcRef(p_ref.into_generic()))?;
                        match p {
                            ReferenceInstance::Class(c) => {
                                let instance_id = c.instanceof;

                                let is_castable = instance_id == *id
                                    || classes.is_super_class(
                                        class_directories,
                                        class_names,
                                        class_files,
                                        packages,
                                        instance_id,
                                        *id,
                                    )?
                                    || classes.implements_interface(
                                        class_directories,
                                        class_names,
                                        class_files,
                                        instance_id,
                                        *id,
                                    )?;

                                if is_castable {
                                    RuntimeValue::Reference(p_ref)
                                } else {
                                    todo!("Type was not castable")
                                }
                            }
                            // TODO: I think we need to check if it is a super class
                            // (though that is just object for arrays) and then check if
                            // the class is some array interface
                            // then use is_castable_array, or does it already do that
                            // earlier stuff?
                            ReferenceInstance::PrimitiveArray(_) => todo!(),
                            ReferenceInstance::ReferenceArray(_) => todo!(),
                        }
                    }
                    RuntimeValue::NullReference => RuntimeValue::NullReference,
                    RuntimeValue::Primitive(_) => {
                        return Err(EvalError::ExpectedStackValueReference.into())
                    }
                },
            }
        }
        DescriptorType::Array { level, component } => todo!(),
    })
}

impl RunInst for InvokeStatic {
    fn run(
        self,
        RunInstArgs {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
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
            &env.class_directories,
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
            &env.class_directories,
            &mut env.class_names,
            &mut env.class_files,
            target_class_id,
            &method_name,
            &method_descriptor,
        )?;

        let mut locals = Locals::default();
        for parameter in method_descriptor.parameters() {
            let value = grab_runtime_value_from_stack_for_function(
                &env.class_directories,
                &mut env.class_names,
                &mut env.class_files,
                &mut env.classes,
                &mut env.packages,
                &mut env.methods,
                &mut env.state,
                frame,
                parameter,
            )?;

            locals.push_transform(value);
        }

        let frame = Frame::new_locals(locals);
        let res = eval_method(env, target_method_id, frame)?;

        Ok(match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => RunInstValue::ReturnVoid,
            EvalMethodValue::Return(v) => RunInstValue::Return(v),
            EvalMethodValue::Exception(exc) => RunInstValue::Exception(exc),
        })
    }
}

impl RunInst for InvokeInterface {
    fn run(self, args: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        todo!()
    }
}

// FIXME: This code ignores specific actions that it should do
impl RunInst for InvokeSpecial {
    fn run(
        self,
        RunInstArgs {
            env,
            method_id,
            frame,
            inst_index,
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let index = self.index;

        let instance_class = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let instance_ref = instance_class
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?
            .expect("TODO: NullReferenceException");

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
            &env.class_directories,
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
            &env.class_directories,
            &mut env.class_names,
            &mut env.class_files,
            target_class_id,
            &method_name,
            &method_descriptor,
        )?;

        let mut locals = Locals::default();
        // Push this
        locals.push_transform(RuntimeValue::Reference(instance_ref));

        for parameter in method_descriptor.parameters() {
            let value = grab_runtime_value_from_stack_for_function(
                &env.class_directories,
                &mut env.class_names,
                &mut env.class_files,
                &mut env.classes,
                &mut env.packages,
                &mut env.methods,
                &mut env.state,
                frame,
                parameter,
            )?;

            locals.push_transform(value);
        }

        let frame = Frame::new_locals(locals);
        let res = eval_method(env, target_method_id, frame)?;

        Ok(match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => RunInstValue::ReturnVoid,
            EvalMethodValue::Return(v) => RunInstValue::Return(v),
            EvalMethodValue::Exception(exc) => RunInstValue::Exception(exc),
        })
    }
}

/// Find  the most specific virtual method
fn find_virtual_method(
    class_directories: &ClassDirectories,
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

    // TODO: Error if it is an instance initialization method?
    let mut current_check_id = instance_id;
    loop {
        let method_id = methods.load_method_from_desc(
            class_directories,
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
                    if super_id == base_id {
                        // Break out of the loop since we've reached the base class
                        // and, while it is a potential end result, we still need to check
                        // the interfaces.
                        break;
                    }

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

    // TODO: Does this check the superinterfaces of the superinterfaces?
    // TODO: Does this check the superinterfaces of the superclasses?
    // Check the superinterfaces of instance_id
    let instance_class_file = class_files
        .get(&instance_id)
        .ok_or(GeneralError::MissingLoadedClassFile(instance_id))?;
    let interfaces: SmallVec<[_; 8]> = instance_class_file.interfaces_indices_iter().collect();
    let interfaces: SmallVec<[_; 8]> =
        map_interface_index_small_vec_to_ids(class_names, instance_class_file, interfaces)?;
    for interface_id in interfaces {
        // let class_file = class_files
        //     .get(&instance_id)
        //     .ok_or(GeneralError::MissingLoadedClassFile(instance_id))?;
        let method_id = methods.load_method_from_desc(
            class_directories,
            class_names,
            class_files,
            interface_id,
            name,
            descriptor,
        );
        match method_id {
            Ok(method_id) => {
                // While we got a method that matches, we first need to check that it is not
                // abstract.
                let method = methods
                    .get(&method_id)
                    .ok_or(GeneralError::MissingLoadedMethod(method_id))?;
                if method.access_flags().contains(MethodAccessFlags::ABSTRACT) {
                    continue;
                }

                return Ok(method_id);
            }
            Err(StepError::LoadMethod(LoadMethodError::NonexistentMethodName { .. })) => {
                // Skip past this interface
                continue;
            }
            // TODO: Or should we just log the error and skip past it?
            Err(err) => return Err(err.into()),
        }
    }

    // TODO: Is this the right ordering? The docs don't explicitly mention when it should call the
    // base class version..
    // If we simply look up the chain, then we'd always find the base class version before we bother
    // checking the interfaces?
    Ok(methods.load_method_from_desc(
        class_directories,
        class_names,
        class_files,
        base_id,
        name,
        descriptor,
    )?)

    // todo!("Exception failed to find function");
}

impl RunInst for InvokeVirtual {
    fn run(
        self,
        RunInstArgs {
            env,
            method_id,
            frame,
            inst_index,
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        // TODO: Validate that the instance ref can be considered to extend the class that the
        // virtual method is on.
        //
        let index = self.index;

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
        let instance_id = match instance {
            ReferenceInstance::Class(instance) => instance.instanceof,
            ReferenceInstance::PrimitiveArray(_) => todo!(),
            ReferenceInstance::ReferenceArray(_) => todo!(),
        };

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
            &env.class_directories,
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

        let target_method_id = find_virtual_method(
            &env.class_directories,
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

        let mut locals = Locals::default();
        // Add the this parameter
        locals.push_transform(RuntimeValue::Reference(instance_ref));

        for parameter in method_descriptor.parameters() {
            let value = grab_runtime_value_from_stack_for_function(
                &env.class_directories,
                &mut env.class_names,
                &mut env.class_files,
                &mut env.classes,
                &mut env.packages,
                &mut env.methods,
                &mut env.state,
                frame,
                parameter,
            )?;

            locals.push_transform(value);
        }

        let frame = Frame::new_locals(locals);
        let res = eval_method(env, target_method_id, frame)?;

        Ok(match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => RunInstValue::ReturnVoid,
            EvalMethodValue::Return(v) => RunInstValue::Return(v),
            EvalMethodValue::Exception(exc) => RunInstValue::Exception(exc),
        })
    }
}

impl RunInst for InvokeDynamic {
    fn run(self, args: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        todo!()
    }
}
