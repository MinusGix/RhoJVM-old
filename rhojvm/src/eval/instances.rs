use classfile_parser::ClassAccessFlags;
use rhojvm_base::{
    class::ArrayClass,
    code::op::{ANewArray, CheckCast, InstanceOf, MultiANewArray, New, NewArray},
    id::ClassId,
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, Methods,
};
use usize_cast::IntoUsize;

use crate::{
    class_instance::{
        ClassInstance, Fields, Instance, PrimitiveArrayInstance, ReferenceArrayInstance,
    },
    eval::EvalError,
    gc::GcRef,
    initialize_class, resolve_derive,
    rv::{RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    GeneralError, State,
};

use super::{RunInst, RunInstArgs, RunInstValue, ValueException};

impl RunInst for New {
    fn run(
        self,
        RunInstArgs {
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            method_id,
            frame,
            ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (class_id, _) = method_id.decompose();
        let class_file = class_files
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
        let target_class_id = class_names.gcid_from_bytes(target_class_name);

        // TODO: This provides some errors that should be exceptions
        resolve_derive(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            target_class_id,
            class_id,
        )?;

        // TODO: Should we check if the status indicates that we already started (so we might be in
        // a loop?)
        // TODO: Some errors returned by this should be exceptions
        let status = initialize_class(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            target_class_id,
        )?;
        let target_ref = match status.into_value() {
            ValueException::Value(target) => target,
            ValueException::Exception(exc) => return Ok(RunInstValue::Exception(exc)),
        };

        let target_class = classes.get(&target_class_id).unwrap();
        if target_class.is_interface()
            || target_class
                .access_flags()
                .contains(ClassAccessFlags::ABSTRACT)
        {
            todo!("return InstantiationError exception")
        }

        // new does not run a constructor, it only initializes it
        let class = ClassInstance {
            instanceof: target_class_id,
            static_ref: target_ref,
            fields: Fields::default(),
        };

        // Allocate the class instance
        let class_ref = state.gc.alloc(class);
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
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            method_id,
            frame,
            ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (class_id, _) = method_id.decompose();
        let class_file = class_files
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
        let elem_class_id = class_names.gcid_from_bytes(elem_class_name);

        // TODO: This provides some errors that should be exceptions
        resolve_derive(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            elem_class_id,
            class_id,
        )?;

        // TODO: Should we check if the status indicates that we already started (so we might be in
        // a loop?)
        // TODO: Some errors returned by this should be exceptions
        let _status = initialize_class(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            elem_class_id,
        )?;

        let count = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let count = count
            .into_int()
            .ok_or(EvalError::ExpectedStackValueIntRepr)?;

        if count < 0 {
            todo!("Return NegativeArraySizeException")
        }

        let count = (count as u32).into_usize();

        // Register the class for arrays of this type
        let array_id = classes.load_array_of_instances(
            class_directories,
            class_names,
            class_files,
            packages,
            elem_class_id,
        )?;

        let mut elements: Vec<Option<GcRef<Instance>>> = Vec::new();
        elements.resize(count, None);

        let array_inst = ReferenceArrayInstance::new(array_id, elem_class_id, elements);
        let array_ref = state.gc.alloc(array_inst);
        frame
            .stack
            .push(RuntimeValue::Reference(array_ref.into_generic()))?;

        Ok(RunInstValue::Continue)
    }
}

impl RunInst for MultiANewArray {
    fn run(self, args: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        todo!()
    }
}

impl RunInst for NewArray {
    fn run(
        self,
        RunInstArgs {
            class_names,
            classes,
            state,
            frame,
            ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let count = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let count = count
            .into_int()
            .ok_or(EvalError::ExpectedStackValueIntRepr)?;

        if count < 0 {
            todo!("Return NegativeArraySizeException")
        }

        let count = (count as u32).into_usize();

        let elem_prim_type = self
            .get_atype_as_primitive_type()
            .ok_or(EvalError::InvalidNewArrayAType)?;
        let elem_type = RuntimeTypePrimitive::from(elem_prim_type);

        // Register the class for arrays of this type
        let array_id = classes.load_array_of_primitives(class_names, elem_prim_type)?;

        let mut elements: Vec<RuntimeValuePrimitive> = Vec::new();
        elements.resize(count, elem_type.default_value());

        let array_inst = PrimitiveArrayInstance::new(array_id, elem_type, elements);
        let array_ref = state.gc.alloc(array_inst);
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
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
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
        let val_inst = state.gc.deref(val).ok_or(EvalError::InvalidGcRef(val))?;

        let (class_id, _) = method_id.decompose();
        let class_file = class_files
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
        let cast_target_id = class_names.gcid_from_bytes(cast_target_name);

        let id = match val_inst {
            Instance::Class(class) => class.instanceof,
            Instance::StaticClass(_) => todo!("Return checkcast exception"),
            Instance::PrimitiveArray(array) => array.instanceof,
            Instance::ReferenceArray(array) => array.instanceof,
        };

        // We currently represent the reference as completely unmodified, but we do have to
        // perform these checks so that we can determine if the cast is correct
        match try_casting(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
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
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
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
        let val_inst = state.gc.deref(val).ok_or(EvalError::InvalidGcRef(val))?;

        let (class_id, _) = method_id.decompose();
        let class_file = class_files
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
        let cast_target_id = class_names.gcid_from_bytes(cast_target_name);

        let id = match val_inst {
            Instance::Class(class) => class.instanceof,
            Instance::StaticClass(_) => todo!("Return exception"),
            Instance::PrimitiveArray(array) => array.instanceof,
            Instance::ReferenceArray(array) => array.instanceof,
        };

        // We currently represent the reference as completely unmodified, but we do have to
        // perform these checks so that we can determine if the cast is correct
        match try_casting(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
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
