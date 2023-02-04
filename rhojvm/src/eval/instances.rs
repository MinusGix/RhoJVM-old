use std::num::NonZeroUsize;

use classfile_parser::{
    constant_info::ConstantInfo,
    descriptor::DescriptorType as DescriptorTypeCF,
    field_info::{FieldAccessFlags, FieldInfoOpt},
    ClassAccessFlags,
};
use rhojvm_base::{
    class::ArrayClass,
    code::{
        method::{DescriptorType, DescriptorTypeBasic},
        op::{ANewArray, CheckCast, InstanceOf, MultiANewArray, New, NewArray},
        types::JavaChar,
    },
    data::{
        class_files::ClassFiles,
        class_names::ClassNames,
        classes::{load_super_classes_iter, Classes},
    },
    id::ClassId,
    package::Packages,
    StepError,
};
use smallvec::SmallVec;
use usize_cast::IntoUsize;

use crate::{
    class_instance::{
        ClassInstance, Field, FieldAccess, FieldId, FieldIndex, Fields, PrimitiveArrayInstance,
        ReferenceArrayInstance, ReferenceInstance, ThreadInstance,
    },
    eval::EvalError,
    exc_value,
    gc::{Gc, GcRef},
    initialize_class, resolve_derive,
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::{self, Env},
    GeneralError,
};

use super::{RunInstArgsC, RunInstContinue, RunInstContinueValue, ValueException};

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
    for (field_index, field_info) in field_iter.into_iter().enumerate() {
        let field_index = FieldIndex::new_unchecked(field_index as u16);
        let field_id = FieldId::unchecked_compose(class_id, field_index);

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
                field_id,
                Field::new(value, field_type, is_final, field_access),
            );
        } else {
            // otherwise, we give it the default value for its type
            let default_value = field_type.default_value();
            fields.insert(
                field_id,
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
pub fn make_fields<F: Fn(&FieldInfoOpt) -> bool>(
    env: &mut Env,
    class_id: ClassId,
    filter_fn: F,
) -> Result<ValueException<Fields>, GeneralError> {
    let mut fields = Fields::default();

    // Iterate over the super classes (which includes the current class)
    // Adding all of their fields to the map
    let mut tree_iter = load_super_classes_iter(class_id);
    while let Some(target_id) = tree_iter.next_item(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
    ) {
        let target_id = target_id?;
        let (_, target_info) = env
            .class_names
            .name_from_gcid(target_id)
            .map_err(StepError::BadId)?;
        if target_info.has_class_file() {
            if let Some(exception) = add_fields_for_class(env, target_id, &filter_fn, &mut fields)?
            {
                return Ok(ValueException::Exception(exception));
            }
        }
        // TODO: This may not be correct
        // otherwise, if no class file, then no fields for this one
    }

    Ok(ValueException::Value(fields))
}

pub fn make_instance_fields(
    env: &mut Env,
    class_id: ClassId,
) -> Result<ValueException<Fields>, GeneralError> {
    make_fields(env, class_id, |field_info| {
        !field_info.access_flags.contains(FieldAccessFlags::STATIC)
    })
}

impl RunInstContinue for New {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
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
        // Thread is handled specially
        let is_thread_class = target_class_name == b"java/lang/Thread";
        let target_class_id = env.class_names.gcid_from_bytes(target_class_name);

        // TODO: This provides some errors that should be exceptions
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

        // TODO: Should we check if the status indicates that we already started (so we might be in
        // a loop?)
        // TODO: Some errors returned by this should be exceptions
        let status = initialize_class(env, target_class_id)?;
        let target_ref = match status.into_value() {
            ValueException::Value(target) => target,
            ValueException::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
        };

        let target_class = env.classes.get(&target_class_id).unwrap();
        if target_class.is_interface()
            || target_class
                .access_flags()
                .contains(ClassAccessFlags::ABSTRACT)
        {
            todo!("return InstantiationError exception")
        }

        let fields = make_instance_fields(env, target_class_id)?;
        let fields = exc_value!(ret inst: fields);

        // new does not run a constructor, it only initializes it
        let class = ClassInstance {
            instanceof: target_class_id,
            static_ref: target_ref,
            fields,
        };

        let class_ref = if is_thread_class {
            let thread = ThreadInstance::new(class, None);
            env.state.gc.alloc(thread).into_generic()
        } else {
            env.state.gc.alloc(class).into_generic()
        };
        frame.stack.push(RuntimeValue::Reference(class_ref))?;

        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for ANewArray {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
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

        Ok(RunInstContinueValue::Continue)
    }
}

fn make_array_desc_type_basic(
    classes: &mut Classes,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    packages: &mut Packages,
    gc: &mut Gc,
    count: u32,
    arr_elem: DescriptorType,
) -> Result<GcRef<ReferenceInstance>, GeneralError> {
    let count = count.into_usize();

    let array_id = classes.load_level_array_of_desc_type(
        class_names,
        class_files,
        packages,
        NonZeroUsize::new(1).unwrap(),
        arr_elem,
    )?;

    match arr_elem {
        DescriptorType::Basic(b) => {
            if let DescriptorTypeBasic::Class(class_id) = b {
                let elements = vec![None; count];
                let arr = ReferenceArrayInstance::new(array_id, class_id, elements);
                let arr_ref = gc.alloc(arr);
                Ok(gc.checked_as(arr_ref).unwrap())
            } else {
                let element_type = RuntimeTypePrimitive::from_desc_type_basic(b).unwrap();
                let elements = vec![element_type.default_value(); count];
                let arr = PrimitiveArrayInstance::new(array_id, element_type, elements);
                let arr_ref = gc.alloc(arr);
                Ok(gc.checked_as(arr_ref).unwrap())
            }
        }
        DescriptorType::Array { level, component } => {
            let class_id = classes.load_level_array_of_desc_type_basic(
                class_names,
                class_files,
                packages,
                level,
                component,
            )?;
            let elements = vec![None; count];
            let arr = ReferenceArrayInstance::new(array_id, class_id, elements);
            let arr_ref = gc.alloc(arr);
            Ok(gc.checked_as(arr_ref).unwrap())
        }
    }
}

impl RunInstContinue for MultiANewArray {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (class_id, _) = method_id.decompose();

        // Dimension is the level of the array, 1 being a 1-dimensional array and 2 being a
        // 2-dimensional array.
        let dimension = self.dimensions;

        // The sizes for the individual arrays are on the stack
        // A smallvec due to the number of dimensions likely being quite small.
        let mut sizes: SmallVec<[u32; 4]> = SmallVec::new();

        for _ in 0..dimension {
            let size = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
            let size = size
                .into_int()
                .ok_or(EvalError::ExpectedStackValueIntRepr)?;

            let Ok(size) = size.try_into() else {
                todo!("Return NegativeArraySizeException")
            };

            sizes.push(size);
        }

        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let arr_class = class_file.getr(self.index)?;
        // TODO: use smallvec here?
        let arr_class_name = class_file.getr_text_b(arr_class.name_index)?.to_vec();
        let arr_class_id = env.class_names.gcid_from_bytes(&arr_class_name);

        resolve_derive(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.classes,
            &mut env.packages,
            &mut env.methods,
            &mut env.state,
            arr_class_id,
            class_id,
        )?;

        // TODO: Should I do something with the status?
        let _status = initialize_class(env, arr_class_id)?;

        // TODO: This is more expensive than it needs to be. It creates a bunch of vectors and
        // iterates over them.
        let mut root_array: Option<GcRef<ReferenceInstance>> = None;
        let mut prev_arrays: Option<Vec<GcRef<ReferenceInstance>>> = None;
        for (i, size) in sizes.into_iter().enumerate() {
            if size == 0 {
                break;
            }

            let arr_elem = {
                let mut name = arr_class_name.as_slice();
                for _ in 0..(i + 1) {
                    name = name.strip_prefix(&[b'[']).unwrap();
                }

                let (name, _) = DescriptorTypeCF::parse(name).unwrap();
                DescriptorType::from_class_file_desc(&mut env.class_names, name)
            };

            if let Some(prev_arrays_ref) = &prev_arrays {
                let mut new_prev_arrays = Vec::new();
                for prev_array_ref in prev_arrays_ref.iter().copied() {
                    let Some(ReferenceInstance::ReferenceArray(prev_array)) = env.state.gc.deref(prev_array_ref) else { unreachable!() };
                    let prev_array_size = prev_array.elements.len();

                    let elements = std::iter::repeat(())
                        .take(prev_array_size)
                        .map(|_| {
                            make_array_desc_type_basic(
                                &mut env.classes,
                                &mut env.class_names,
                                &mut env.class_files,
                                &mut env.packages,
                                &mut env.state.gc,
                                size,
                                arr_elem,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    new_prev_arrays.extend(elements.iter().copied());

                    let Some(ReferenceInstance::ReferenceArray(prev_array)) = env.state.gc.deref_mut(prev_array_ref) else { unreachable!() };
                    prev_array.elements = elements.into_iter().map(Some).collect();
                }

                prev_arrays = Some(new_prev_arrays);
            } else {
                let re = make_array_desc_type_basic(
                    &mut env.classes,
                    &mut env.class_names,
                    &mut env.class_files,
                    &mut env.packages,
                    &mut env.state.gc,
                    size,
                    arr_elem,
                )?;
                root_array = Some(re);
                prev_arrays = Some(vec![re]);
            }
        }

        // TODO: root_array could be None if the size is 0?
        let root_array = root_array.unwrap();

        frame
            .stack
            .push(RuntimeValue::Reference(root_array.into_generic()))?;

        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for NewArray {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
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

        Ok(RunInstContinueValue::Continue)
    }
}

pub(crate) enum CastResult {
    Success,
    Failure,
    Exception(GcRef<ClassInstance>),
}

fn classcast_exception(
    _env: &mut Env,
    _from: ClassId,
    _to: ClassId,
    message: &str,
) -> GcRef<ClassInstance> {
    panic!("Construct CheckCast exception, msg: {}", message);
}

/// Tries to cast the class id to the other class id
/// Note that even if your `make_failure` method does not create exceptions, there may be other
/// exceptions created (such as during failure to resolve class)
pub(crate) fn try_casting(
    env: &mut Env,
    loading_from_id: ClassId,
    class_id: ClassId,
    desired_class_id: ClassId,
    make_failure: impl FnOnce(&mut Env, ClassId, ClassId, &str) -> Result<CastResult, GeneralError>,
) -> Result<CastResult, GeneralError> {
    // TODO: Some of the errors from this should be exceptions
    // Resolve the class from our class
    resolve_derive(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
        &mut env.methods,
        &mut env.state,
        desired_class_id,
        loading_from_id,
    )?;

    if class_id == desired_class_id {
        return Ok(CastResult::Success);
    }

    let class = env.classes.get(&class_id).unwrap();
    let target_class = env.classes.get(&desired_class_id).unwrap();
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
                try_casting(env, loading_from_id, class_comp, target_comp, make_failure)
            } else {
                Ok(make_failure(
                    env,
                    class_id,
                    desired_class_id,
                    "Arrays had differing component types",
                )?)
            }
        } else if target_class.is_interface() {
            let interfaces = ArrayClass::get_interface_names()
                .iter()
                .map(|x| env.class_names.gcid_from_bytes(x));
            for interface in interfaces {
                if interface == desired_class_id {
                    return Ok(CastResult::Success);
                }
            }

            // Otherwise, it was not an interface implemented by the array
            Ok(make_failure(
                env,
                class_id,
                desired_class_id,
                "Array could not be casted to interface which was not a super-interface",
            )?)
        } else if desired_class_id == env.class_names.object_id() {
            Ok(CastResult::Success)
        } else {
            Ok(make_failure(
                env,
                class_id,
                desired_class_id,
                "Array can not be casted to a class other than Object",
            )?)
        }
    } else if class.is_interface() {
        // Interface
        if target_class.is_interface() {
            if env.classes.implements_interface(
                &mut env.class_names,
                &mut env.class_files,
                class_id,
                desired_class_id,
            )? {
                Ok(CastResult::Success)
            } else {
                Ok(make_failure(
                    env,
                    class_id,
                    desired_class_id,
                    "Interface could not be casted to other interface which was not a super-interface",
                )?)
            }
        } else if desired_class_id == env.class_names.object_id() {
            Ok(CastResult::Success)
        } else {
            Ok(make_failure(
                env,
                class_id,
                desired_class_id,
                "Interface can not be casted to a class other than Object",
            )?)
        }
    } else {
        // Normal class
        if target_class.is_interface() {
            if env.classes.implements_interface(
                &mut env.class_names,
                &mut env.class_files,
                class_id,
                desired_class_id,
            )? {
                Ok(CastResult::Success)
            } else {
                Ok(make_failure(
                    env,
                    class_id,
                    desired_class_id,
                    "Class does not implement interface",
                )?)
            }
        } else if env.classes.is_super_class(
            &mut env.class_names,
            &mut env.class_files,
            &mut env.packages,
            class_id,
            desired_class_id,
        )? {
            Ok(CastResult::Success)
        } else {
            Ok(make_failure(
                env,
                class_id,
                desired_class_id,
                "Class does not extend casted class",
            )?)
        }
    }
}

impl RunInstContinue for CheckCast {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let val = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let val = match val {
            RuntimeValue::NullReference => {
                frame.stack.push(RuntimeValue::NullReference)?;
                return Ok(RunInstContinueValue::Continue);
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
            env,
            class_id,
            id,
            cast_target_id,
            |env, class_id, desired_class_id, message| {
                Ok(CastResult::Exception(classcast_exception(
                    env,
                    class_id,
                    desired_class_id,
                    message,
                )))
            },
        )? {
            CastResult::Success => {
                frame.stack.push(RuntimeValue::Reference(val))?;
                Ok(RunInstContinueValue::Continue)
            }
            CastResult::Failure => todo!("CheckedCast exception"),
            CastResult::Exception(exc) => Ok(RunInstContinueValue::Exception(exc)),
        }
    }
}

impl RunInstContinue for InstanceOf {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let val = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let val = match val {
            RuntimeValue::NullReference => {
                frame.stack.push(RuntimeValuePrimitive::I32(0))?;
                return Ok(RunInstContinueValue::Continue);
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
        match try_casting(env, class_id, id, cast_target_id, |_env, _, _, _| {
            Ok(CastResult::Failure)
        })? {
            CastResult::Success => {
                frame.stack.push(RuntimeValuePrimitive::I32(1))?;
                Ok(RunInstContinueValue::Continue)
            }
            CastResult::Failure => {
                frame.stack.push(RuntimeValuePrimitive::I32(0))?;
                Ok(RunInstContinueValue::Continue)
            }
            CastResult::Exception(exc) => Ok(RunInstContinueValue::Exception(exc)),
        }
    }
}
