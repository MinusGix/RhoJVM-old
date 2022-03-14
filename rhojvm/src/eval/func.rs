use classfile_parser::{constant_info::ConstantInfo, method_info::MethodAccessFlags};
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
    eval::{eval_method, EvalError, EvalMethodValue, Frame, Locals},
    initialize_class, map_interface_index_small_vec_to_ids, resolve_derive,
    rv::{RuntimeValue, RuntimeValuePrimitive},
    GeneralError, State,
};

use super::{RunInst, RunInstArgs, RunInstValue};

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
                        todo!("Type was not castable");
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
        let res = eval_method(env, target_method_id.into(), call_frame)?;

        match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => (),
            EvalMethodValue::Return(v) => frame.stack.push(v)?,
            EvalMethodValue::Exception(exc) => return Ok(RunInstValue::Exception(exc)),
        }

        Ok(RunInstValue::Continue)
    }
}

impl RunInst for InvokeInterface {
    fn run(
        self,
        RunInstArgs {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
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
        let res = eval_method(env, target_method_id, call_frame)?;

        match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => (),
            EvalMethodValue::Return(v) => frame.stack.push(v)?,
            EvalMethodValue::Exception(exc) => return Ok(RunInstValue::Exception(exc)),
        }

        Ok(RunInstValue::Continue)
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
        let res = eval_method(env, target_method_id.into(), call_frame)?;

        match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => (),
            EvalMethodValue::Return(v) => frame.stack.push(v)?,
            EvalMethodValue::Exception(exc) => return Ok(RunInstValue::Exception(exc)),
        }

        Ok(RunInstValue::Continue)
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
    }

    // TODO: Does this check the superinterfaces of the superinterfaces?
    // TODO: Does this check the superinterfaces of the superclasses?
    // Check the superinterfaces of instance_id
    if instance_has_class_file {
        let instance_class_file = class_files
            .get(&instance_id)
            .ok_or(GeneralError::MissingLoadedClassFile(instance_id))?;
        let interfaces: SmallVec<[_; 8]> = instance_class_file.interfaces_indices_iter().collect();
        let interfaces: SmallVec<[_; 8]> =
            map_interface_index_small_vec_to_ids(class_names, instance_class_file, interfaces)?;

        for interface_id in interfaces {
            let method_id = methods.load_method_from_desc(
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
                        .ok_or(GeneralError::MissingLoadedMethod(method_id.into()))?;
                    if method.access_flags().contains(MethodAccessFlags::ABSTRACT) {
                        continue;
                    }

                    return Ok(method_id.into());
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

impl RunInst for InvokeVirtual {
    fn run(
        self,
        RunInstArgs {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
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
            .expect("TODO: NullReferenceException");
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
        let res = eval_method(env, target_method_id, call_frame)?;

        match res {
            // TODO: Check that these are valid return types!
            // We can use the casting code we wrote above for the check, probably?
            EvalMethodValue::ReturnVoid => (),
            EvalMethodValue::Return(v) => frame.stack.push(v)?,
            EvalMethodValue::Exception(exc) => return Ok(RunInstValue::Exception(exc)),
        }

        Ok(RunInstValue::Continue)
    }
}

impl RunInst for InvokeDynamic {
    fn run(self, _: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        todo!()
    }
}
