use classfile_parser::{
    constant_info::ConstantInfo,
    descriptor::DescriptorType as DescriptorTypeCF,
    field_info::{FieldAccessFlags, FieldInfoOpt},
    ClassAccessFlags,
};
use either::Either;
use rhojvm_base::{
    class::ArrayClass,
    code::{
        method::DescriptorType,
        op::{ANewArray, CheckCast, InstanceOf, MultiANewArray, New, NewArray},
        types::JavaChar,
    },
    id::ClassId,
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, Methods, StepError,
};
use smallvec::SmallVec;
use usize_cast::IntoUsize;

use crate::{
    class_instance::{
        ClassInstance, Field, FieldAccess, Fields, OwnedFieldKey, PrimitiveArrayInstance,
        ReferenceArrayInstance, ReferenceInstance,
    },
    eval::EvalError,
    gc::GcRef,
    initialize_class, resolve_derive,
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::{self, Env},
    GeneralError, State,
};

use super::{RunInst, RunInstArgs, RunInstValue, ValueException};

pub(crate) fn add_fields_for_class<F: Fn(&FieldInfoOpt) -> bool>(
    env: &mut Env,
    class_id: ClassId,
    filter_fn: F,
    fields: &mut Fields,
) -> Result<Option<GcRef<ClassInstance>>, GeneralError> {
    let class_file = env
        .class_files
        .get(&class_id)
        .ok_or(EvalError::MissingMethodClassFile(class_id))?;

    // TODO: It'd be nice if we could avoid potentially allocating
    // We could probably do this if we cloned the Rc for the class file data
    let field_iter = class_file
        .load_field_values_iter()
        .collect::<SmallVec<[_; 8]>>();
    for field_info in field_iter {
        // Reget the class file
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let (field_info, constant_index) = field_info.map_err(GeneralError::ClassFileLoad)?;
        if !filter_fn(&field_info) {
            // Skip past this field
            continue;
        }

        // TODO: We could avoid allocations
        let field_name = class_file
            .get_text_b(field_info.name_index)
            .ok_or(GeneralError::BadClassFileIndex(
                field_info.name_index.into_generic(),
            ))?
            .to_owned();
        let field_descriptor = class_file.get_text_b(field_info.descriptor_index).ok_or(
            GeneralError::BadClassFileIndex(field_info.descriptor_index.into_generic()),
        )?;
        // Parse the type of the field
        let (field_type, rem) = DescriptorTypeCF::parse(field_descriptor)
            .map_err(GeneralError::InvalidDescriptorType)?;
        // There shouldn't be any remaining data.
        if !rem.is_empty() {
            return Err(GeneralError::UnparsedFieldType);
        }
        // Convert to alternative descriptor type
        let field_type = DescriptorType::from_class_file_desc(&mut env.class_names, field_type);
        let field_type: RuntimeType<ClassId> =
            RuntimeType::from_descriptor_type(&mut env.class_names, field_type)
                .map_err(StepError::BadId)?;

        // Reget the class file
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let is_final = field_info.access_flags.contains(FieldAccessFlags::FINAL);
        let field_access = FieldAccess::from_access_flags(field_info.access_flags);
        if let Some(constant_index) = constant_index {
            let constant = class_file
                .get_t(constant_index)
                .ok_or(GeneralError::BadClassFileIndex(constant_index))?
                .clone();
            let value = match constant {
                ConstantInfo::Integer(x) => RuntimeValuePrimitive::I32(x.value).into(),
                ConstantInfo::Float(x) => RuntimeValuePrimitive::F32(x.value).into(),

                ConstantInfo::Double(x) => RuntimeValuePrimitive::F64(x.value).into(),

                ConstantInfo::Long(x) => RuntimeValuePrimitive::I64(x.value).into(),

                ConstantInfo::String(x) => {
                    // TODO: Better string conversion
                    let text = class_file.get_text_t(x.string_index).ok_or(
                        GeneralError::BadClassFileIndex(x.string_index.into_generic()),
                    )?;
                    let text = text
                        .encode_utf16()
                        .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
                        .collect::<Vec<RuntimeValuePrimitive>>();

                    let string_ref = util::construct_string(env, text)?;
                    match string_ref {
                        ValueException::Value(string_ref) => {
                            RuntimeValue::Reference(string_ref.into_generic())
                        }

                        // TODO: include information that it was due to initializing a field
                        ValueException::Exception(exc) => {
                            return Ok(Some(exc));
                        }
                    }
                }
                // TODO: Better error
                _ => return Err(GeneralError::BadClassFileIndex(constant_index)),
            };

            // TODO: Validate that the value is the right type
            fields.insert(
                OwnedFieldKey::new(class_id, field_name),
                Field::new(value, field_type, is_final, field_access),
            );
        } else {
            // otherwise, we give it the default value for its type
            let default_value = field_type.default_value();
            fields.insert(
                OwnedFieldKey::new(class_id, field_name),
                Field::new(default_value, field_type, is_final, field_access),
            );
        }
    }

    Ok(None)
}

/// Loads all the fields, filtered by some function
/// initializing them with their value
/// Returns either an exception or the fields
/// Takes a filter function so that this can be used for static class initialization and normal
/// class init
///     Should return true if it should be kept
pub(crate) fn make_fields<F: Fn(&FieldInfoOpt) -> bool>(
    env: &mut Env,
    class_id: ClassId,
    filter_fn: F,
) -> Result<Either<Fields, GcRef<ClassInstance>>, GeneralError> {
    let mut fields = Fields::default();

    // Iterate over the super classes (which includes the current class)
    // Adding all of their fields to the map
    let mut tree_iter = rhojvm_base::load_super_classes_iter(class_id);
    while let Some(target_id) = tree_iter.next_item(
        &env.class_directories,
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
    ) {
        let target_id = target_id?;
        if let Some(exception) = add_fields_for_class(env, target_id, &filter_fn, &mut fields)? {
            return Ok(Either::Right(exception));
        }
    }

    Ok(Either::Left(fields))
}

impl RunInst for New {
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

        let target_class =
            class_file
                .get_t(self.index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let target_class_name = class_file.get_text_b(target_class.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(target_class.name_index.into_generic()),
        )?;
        let target_class_id = env.class_names.gcid_from_bytes(target_class_name);

        // TODO: This provides some errors that should be exceptions
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

        // TODO: Should we check if the status indicates that we already started (so we might be in
        // a loop?)
        // TODO: Some errors returned by this should be exceptions
        let status = initialize_class(env, target_class_id)?;
        let target_ref = match status.into_value() {
            ValueException::Value(target) => target,
            ValueException::Exception(exc) => return Ok(RunInstValue::Exception(exc)),
        };

        let target_class = env.classes.get(&target_class_id).unwrap();
        if target_class.is_interface()
            || target_class
                .access_flags()
                .contains(ClassAccessFlags::ABSTRACT)
        {
            todo!("return InstantiationError exception")
        }

        let fields = match make_fields(env, target_class_id, |field_info| {
            !field_info.access_flags.contains(FieldAccessFlags::STATIC)
        })? {
            Either::Left(fields) => fields,
            Either::Right(exc) => {
                return Ok(RunInstValue::Exception(exc));
            }
        };

        // new does not run a constructor, it only initializes it
        let class = ClassInstance {
            instanceof: target_class_id,
            static_ref: target_ref,
            fields,
        };

        // Allocate the class instance
        let class_ref = env.state.gc.alloc(class);
        frame
            .stack
            .push(RuntimeValue::Reference(class_ref.into_generic()))?;

        Ok(RunInstValue::Continue)
    }
}

impl RunInst for ANewArray {
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

        let elem_class =
            class_file
                .get_t(self.index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let elem_class_name = class_file.get_text_b(elem_class.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(elem_class.name_index.into_generic()),
        )?;
        let elem_class_id = env.class_names.gcid_from_bytes(elem_class_name);

        // TODO: This provides some errors that should be exceptions
        resolve_derive(
            &env.class_directories,
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            elem_class_id,
            class_id,
        )?;

        // TODO: Should we check if the status indicates that we already started (so we might be in
        // a loop?)
        // TODO: Some errors returned by this should be exceptions
        let _status = initialize_class(env, elem_class_id)?;

        let count = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let count = count
            .into_int()
            .ok_or(EvalError::ExpectedStackValueIntRepr)?;

        if count < 0 {
            todo!("Return NegativeArraySizeException")
        }

        #[allow(clippy::cast_sign_loss)]
        let count = (count as u32).into_usize();

        // Register the class for arrays of this type
        let array_id = env.classes.load_array_of_instances(
            &env.class_directories,
            &mut env.class_names,
            &mut env.class_files,
            &mut env.packages,
            elem_class_id,
        )?;

        let mut elements: Vec<Option<GcRef<ReferenceInstance>>> = Vec::new();
        elements.resize(count, None);

        let array_inst = ReferenceArrayInstance::new(array_id, elem_class_id, elements);
        let array_ref = env.state.gc.alloc(array_inst);
        frame
            .stack
            .push(RuntimeValue::Reference(array_ref.into_generic()))?;

        Ok(RunInstValue::Continue)
    }
}

impl RunInst for MultiANewArray {
    fn run(self, _: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        todo!()
    }
}

impl RunInst for NewArray {
    fn run(
        self,
        RunInstArgs { env, frame, .. }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let count = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let count = count
            .into_int()
            .ok_or(EvalError::ExpectedStackValueIntRepr)?;

        if count < 0 {
            todo!("Return NegativeArraySizeException")
        }

        #[allow(clippy::cast_sign_loss)]
        let count = (count as u32).into_usize();

        let elem_prim_type = self
            .get_atype_as_primitive_type()
            .ok_or(EvalError::InvalidNewArrayAType)?;
        let elem_type = RuntimeTypePrimitive::from(elem_prim_type);

        // Register the class for arrays of this type
        let array_id = env
            .classes
            .load_array_of_primitives(&mut env.class_names, elem_prim_type)?;

        let mut elements: Vec<RuntimeValuePrimitive> = Vec::new();
        elements.resize(count, elem_type.default_value());

        let array_inst = PrimitiveArrayInstance::new(array_id, elem_type, elements);
        let array_ref = env.state.gc.alloc(array_inst);
        frame
            .stack
            .push(RuntimeValue::Reference(array_ref.into_generic()))?;

        Ok(RunInstValue::Continue)
    }
}

pub(crate) enum CastResult {
    Success,
    Failure,
    Exception(GcRef<ClassInstance>),
}

fn classcast_exception(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    from: ClassId,
    to: ClassId,
    message: &str,
) -> GcRef<ClassInstance> {
    todo!("Construct CheckCast exception")
}

/// Tries to cast the class id to the other class id
/// Note that even if your `make_failure` method does not create exceptions, there may be other
/// exceptions created (such as during failure to resolve class)
pub(crate) fn try_casting(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    class_id: ClassId,
    desired_class_id: ClassId,
    make_failure: impl FnOnce(
        &ClassDirectories,
        &mut ClassNames,
        &mut ClassFiles,
        &mut Classes,
        &mut Packages,
        &mut Methods,
        &mut State,
        ClassId,
        ClassId,
        &str,
    ) -> Result<CastResult, GeneralError>,
) -> Result<CastResult, GeneralError> {
    // TODO: Some of the errors from this should be exceptions
    // Resolve the class from our class
    resolve_derive(
        class_directories,
        class_names,
        class_files,
        classes,
        packages,
        methods,
        state,
        desired_class_id,
        class_id,
    )?;

    let class = classes.get(&class_id).unwrap();
    let target_class = classes.get(&desired_class_id).unwrap();
    if class.is_array() {
        // Array
        if target_class.is_array() {
            // If the component types are the same
            // or if you can apply this same try_casting code to the component class
            let class = class.as_array().unwrap();
            let target_class = target_class.as_array().unwrap();

            let class_comp = class.component_type();
            let target_comp = target_class.component_type();
            // TODO: Can arrays of, ex, chars be cast to an array of ints? I don't think so?

            if class_comp == target_comp {
                // If the primitive types are equal, or if it is the same class, then we're good
                return Ok(CastResult::Success);
            }

            // If they weren't equal, we only want to consider class ids
            let class_comp = class_comp.into_class_id();
            let target_comp = target_comp.into_class_id();

            if let Some((class_comp, target_comp)) = class_comp.zip(target_comp) {
                try_casting(
                    class_directories,
                    class_names,
                    class_files,
                    classes,
                    packages,
                    methods,
                    state,
                    class_comp,
                    target_comp,
                    make_failure,
                )
            } else {
                Ok(make_failure(
                    class_directories,
                    class_names,
                    class_files,
                    classes,
                    packages,
                    methods,
                    state,
                    class_id,
                    desired_class_id,
                    "Arrays had differing component types",
                )?)
            }
        } else if target_class.is_interface() {
            let interfaces = ArrayClass::get_interface_names()
                .iter()
                .map(|x| class_names.gcid_from_bytes(x));
            for interface in interfaces {
                if interface == desired_class_id {
                    return Ok(CastResult::Success);
                }
            }

            // Otherwise, it was not an interface implemented by the array
            Ok(make_failure(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                class_id,
                desired_class_id,
                "Array could not be casted to interface which was not a super-interface",
            )?)
        } else if desired_class_id == class_names.object_id() {
            Ok(CastResult::Success)
        } else {
            Ok(make_failure(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                class_id,
                desired_class_id,
                "Array can not be casted to a class other than Object",
            )?)
        }
    } else if class.is_interface() {
        // Interface
        if target_class.is_interface() {
            if classes.implements_interface(
                class_directories,
                class_names,
                class_files,
                class_id,
                desired_class_id,
            )? {
                Ok(CastResult::Success)
            } else {
                Ok(make_failure(
                    class_directories,
                    class_names,
                    class_files,
                    classes,
                    packages,
                    methods,
                    state,
                    class_id,
                    desired_class_id,
                    "Interface could not be casted to other interface which was not a super-interface",
                )?)
            }
        } else if desired_class_id == class_names.object_id() {
            Ok(CastResult::Success)
        } else {
            Ok(make_failure(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                class_id,
                desired_class_id,
                "Interface can not be casted to a class other than Object",
            )?)
        }
    } else {
        // Normal class
        if target_class.is_interface() {
            if classes.implements_interface(
                class_directories,
                class_names,
                class_files,
                class_id,
                desired_class_id,
            )? {
                Ok(CastResult::Success)
            } else {
                Ok(make_failure(
                    class_directories,
                    class_names,
                    class_files,
                    classes,
                    packages,
                    methods,
                    state,
                    class_id,
                    desired_class_id,
                    "Class does not implement interface",
                )?)
            }
        } else if classes.is_super_class(
            class_directories,
            class_names,
            class_files,
            packages,
            class_id,
            desired_class_id,
        )? {
            Ok(CastResult::Success)
        } else {
            Ok(make_failure(
                class_directories,
                class_names,
                class_files,
                classes,
                packages,
                methods,
                state,
                class_id,
                desired_class_id,
                "Class does not extend casted class",
            )?)
        }
    }
}

impl RunInst for CheckCast {
    fn run(
        self,
        RunInstArgs {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let val = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let val = match val {
            RuntimeValue::NullReference => {
                frame.stack.push(RuntimeValue::NullReference)?;
                return Ok(RunInstValue::Continue);
            }
            RuntimeValue::Reference(gc_ref) => gc_ref,
            RuntimeValue::Primitive(_) => return Err(EvalError::ExpectedStackValueReference.into()),
        };
        let val_inst = env
            .state
            .gc
            .deref(val)
            .ok_or(EvalError::InvalidGcRef(val.into_generic()))?;

        let (class_id, _) = method_id.decompose();
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let cast_target =
            class_file
                .get_t(self.index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let cast_target_name = class_file.get_text_b(cast_target.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(cast_target.name_index.into_generic()),
        )?;
        let cast_target_id = env.class_names.gcid_from_bytes(cast_target_name);

        let id = val_inst.instanceof();

        // We currently represent the reference as completely unmodified, but we do have to
        // perform these checks so that we can determine if the cast is correct
        match try_casting(
            &env.class_directories,
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            id,
            cast_target_id,
            |class_directories,
             class_names,
             class_files,
             classes,
             packages,
             methods,
             state,
             class_id,
             desired_class_id,
             message| {
                Ok(CastResult::Exception(classcast_exception(
                    class_directories,
                    class_names,
                    class_files,
                    classes,
                    packages,
                    methods,
                    state,
                    class_id,
                    desired_class_id,
                    message,
                )))
            },
        )? {
            CastResult::Success => {
                frame.stack.push(RuntimeValue::Reference(val))?;
                Ok(RunInstValue::Continue)
            }
            CastResult::Failure => todo!("CheckedCast exception"),
            CastResult::Exception(exc) => Ok(RunInstValue::Exception(exc)),
        }
    }
}

impl RunInst for InstanceOf {
    fn run(
        self,
        RunInstArgs {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let val = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let val = match val {
            RuntimeValue::NullReference => {
                frame.stack.push(RuntimeValue::NullReference)?;
                return Ok(RunInstValue::Continue);
            }
            RuntimeValue::Reference(gc_ref) => gc_ref,
            RuntimeValue::Primitive(_) => return Err(EvalError::ExpectedStackValueReference.into()),
        };
        let val_inst = env
            .state
            .gc
            .deref(val)
            .ok_or(EvalError::InvalidGcRef(val.into_generic()))?;

        let (class_id, _) = method_id.decompose();
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let cast_target =
            class_file
                .get_t(self.index)
                .ok_or(EvalError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let cast_target_name = class_file.get_text_b(cast_target.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(cast_target.name_index.into_generic()),
        )?;
        let cast_target_id = env.class_names.gcid_from_bytes(cast_target_name);

        let id = val_inst.instanceof();

        // We currently represent the reference as completely unmodified, but we do have to
        // perform these checks so that we can determine if the cast is correct
        match try_casting(
            &env.class_directories,
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            id,
            cast_target_id,
            |_, _, _, _, _, _, _, _, _, _| Ok(CastResult::Failure),
        )? {
            CastResult::Success => {
                frame.stack.push(RuntimeValuePrimitive::I32(1))?;
                Ok(RunInstValue::Continue)
            }
            CastResult::Failure => {
                frame.stack.push(RuntimeValuePrimitive::I32(0))?;
                Ok(RunInstValue::Continue)
            }
            CastResult::Exception(exc) => Ok(RunInstValue::Exception(exc)),
        }
    }
}
