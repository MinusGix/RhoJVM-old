use rhojvm_base::code::types::{LocalVariableType, PopComplexType};

use rhojvm_base::{
    class::ClassFileData,
    code::{
        op::Inst,
        stack_map::StackMapType,
        types::{ComplexType, PopType, PrimitiveType, PushType, Type, WithType},
        CodeInfo,
    },
    id::ClassId,
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, StepError,
};
use smallvec::SmallVec;

use crate::{Local, Locals, VerifyStackMapError, VerifyStackMapGeneralError};

pub(crate) struct InstTypes {
    pub(crate) pop: SmallVec<[Option<FrameType>; 6]>,
}
impl InstTypes {
    pub(crate) fn new() -> InstTypes {
        InstTypes {
            pop: SmallVec::new(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.pop.clear();
    }

    pub(crate) fn get_pop(&self, index: usize) -> Option<&FrameType> {
        // The inner value should not be None when this is used
        self.pop.get(index).map(|x| x.as_ref().unwrap())
    }
}

// TODO: We could theoretically use this for at least some extra type checking
/// A type for verifying frames
/// This does not use [`StackMapType`] because there is no sensible way to
/// convert something like a [`ComplexType::ReferenceAny`] into a specific type // without lookahead.
#[derive(Debug, Clone)]
pub enum FrameType {
    /// We simply use the [`PrimitiveType`] from opcodes, because they are the same.
    /// Technically, the stack represents several different types as integers, but we
    /// can handle that.
    Primitive(PrimitiveType),
    Complex(ComplexFrameType),
}
impl FrameType {
    pub(crate) fn is_category_1(&self) -> bool {
        !matches!(
            self,
            FrameType::Primitive(PrimitiveType::Long | PrimitiveType::Double)
        )
    }

    #[allow(clippy::match_same_arms)]
    /// Is the type on the right convertible into the type on the left on a stack
    pub(crate) fn is_stack_same_of_frame_type(
        &self,
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        right: &FrameType,
    ) -> Result<bool, VerifyStackMapGeneralError> {
        Ok(match (self, right) {
            (FrameType::Primitive(left), FrameType::Primitive(right)) => {
                left.is_same_type_on_stack(right)
            }
            (FrameType::Complex(left), FrameType::Complex(right)) => match (left, right) {
                (
                    ComplexFrameType::ReferenceClass(l_id),
                    ComplexFrameType::ReferenceClass(r_id),
                ) => {
                    l_id == r_id
                        || classes.is_super_class(
                            class_directories,
                            class_names,
                            class_files,
                            packages,
                            *r_id,
                            *l_id,
                        )?
                        || classes.implements_interface(
                            class_directories,
                            class_names,
                            class_files,
                            *r_id,
                            *l_id,
                        )?
                        || classes.is_castable_array(
                            class_directories,
                            class_names,
                            class_files,
                            packages,
                            *r_id,
                            *l_id,
                        )?
                }
                // TODO: We could try producing a stronger distinction between these, so that
                // reference classes are always initialized, but at the moment reference class
                // contains uninit reference classes
                (
                    ComplexFrameType::ReferenceClass(l_id),
                    ComplexFrameType::UninitializedReferenceClass(r_id),
                )
                | (
                    ComplexFrameType::UninitializedReferenceClass(r_id),
                    ComplexFrameType::ReferenceClass(l_id),
                ) => {
                    l_id == r_id
                        || classes.is_super_class(
                            class_directories,
                            class_names,
                            class_files,
                            packages,
                            *r_id,
                            *l_id,
                        )?
                        || classes.implements_interface(
                            class_directories,
                            class_names,
                            class_files,
                            *r_id,
                            *l_id,
                        )?
                        || classes.is_castable_array(
                            class_directories,
                            class_names,
                            class_files,
                            packages,
                            *r_id,
                            *l_id,
                        )?
                }
                (
                    ComplexFrameType::UninitializedReferenceClass(l_id),
                    ComplexFrameType::UninitializedReferenceClass(r_id),
                ) => {
                    l_id == r_id
                        || classes.is_super_class(
                            class_directories,
                            class_names,
                            class_files,
                            packages,
                            *r_id,
                            *l_id,
                        )?
                        || classes.implements_interface(
                            class_directories,
                            class_names,
                            class_files,
                            *r_id,
                            *l_id,
                        )?
                        || classes.is_castable_array(
                            class_directories,
                            class_names,
                            class_files,
                            packages,
                            *r_id,
                            *l_id,
                        )?
                }
                // null is a valid value for any class
                (
                    ComplexFrameType::ReferenceClass(_)
                    | ComplexFrameType::UninitializedReferenceClass(_)
                    | ComplexFrameType::ReferenceNull,
                    ComplexFrameType::ReferenceNull,
                )
                | (
                    ComplexFrameType::ReferenceNull,
                    ComplexFrameType::ReferenceClass(_)
                    | ComplexFrameType::UninitializedReferenceClass(_),
                ) => true,
            },
            (FrameType::Primitive(_), FrameType::Complex(_))
            | (FrameType::Complex(_), FrameType::Primitive(_)) => false,
        })
    }

    pub(crate) fn from_stack_map_types<const N: usize>(
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
        code: &CodeInfo,
        types: &[StackMapType],
        result: &mut SmallVec<[FrameType; N]>,
    ) -> Result<(), VerifyStackMapGeneralError> {
        for typ in types.iter() {
            let output = match typ {
                StackMapType::Integer => PrimitiveType::Int.into(),
                StackMapType::Float => PrimitiveType::Float.into(),
                StackMapType::Long => PrimitiveType::Long.into(),
                StackMapType::Double => PrimitiveType::Double.into(),
                StackMapType::UninitializedThis(id) => {
                    ComplexFrameType::UninitializedReferenceClass(*id).into()
                }
                StackMapType::UninitializedVariable(idx) => {
                    // TODO(recover-faulty-stack-map): We could theoretically
                    // lossily recover from this
                    let inst = code
                        .instructions()
                        .get_instruction_at(*idx)
                        .ok_or(VerifyStackMapError::UninitializedVariableBadIndex { idx: *idx })?;
                    let new_inst = if let Inst::New(new_inst) = inst {
                        new_inst
                    } else {
                        return Err(
                            VerifyStackMapError::UninitializedVariableIncorrectInstruction {
                                idx: *idx,
                                inst_name: inst.name(),
                            }
                            .into(),
                        );
                    };

                    let class_index = new_inst.index;
                    let class = class_file
                        .get_t(class_index)
                        .ok_or(VerifyStackMapError::BadNewClassIndex { index: class_index })?;
                    let class_name = class_file.get_text_b(class.name_index).ok_or(
                        VerifyStackMapError::BadNewClassNameIndex {
                            index: class.name_index,
                        },
                    )?;
                    let class_id = class_names.gcid_from_bytes(class_name);

                    ComplexFrameType::UninitializedReferenceClass(class_id).into()
                }
                StackMapType::Object(id) => ComplexFrameType::ReferenceClass(*id).into(),
                StackMapType::Null => ComplexFrameType::ReferenceNull.into(),
                // We can simply skip this, since it should always be paired
                // with the actual type entry, aka Long or Double, and so we
                // can just treat that as the type.
                // TODO: Though there should be some form of toggleable
                // verification step when first getting the stack frames from
                // the file to ensure that they do pair them together properly.
                StackMapType::Top => continue,
            };
            result.push(output);
        }

        Ok(())
    }

    pub(crate) fn from_opcode_primitive_type(primitive: PrimitiveType) -> FrameType {
        FrameType::Primitive(primitive)
    }

    pub(crate) fn from_opcode_complex_type(
        class_names: &mut ClassNames,
        complex: &ComplexType,
    ) -> Result<FrameType, VerifyStackMapGeneralError> {
        Ok(match complex {
            ComplexType::RefArrayPrimitive(prim) => {
                let array_id = class_names.gcid_from_array_of_primitives(*prim);
                ComplexFrameType::ReferenceClass(array_id).into()
            }
            ComplexType::RefArrayPrimitiveLevels { level, primitive } => {
                let array_id = class_names.gcid_from_level_array_of_primitives(*level, *primitive);
                ComplexFrameType::ReferenceClass(array_id).into()
            }
            ComplexType::RefArrayLevels { level, class_id } => {
                let array_id = class_names
                    .gcid_from_level_array_of_class_id(*level, *class_id)
                    .map_err(StepError::BadId)?;
                ComplexFrameType::ReferenceClass(array_id).into()
            }
            ComplexType::ReferenceClass(id) => ComplexFrameType::ReferenceClass(*id).into(),
            ComplexType::ReferenceNull => ComplexFrameType::ReferenceNull.into(),
        })
    }

    pub(crate) fn from_opcode_with_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        with_t: &WithType,
        inst_types: &InstTypes,
        locals: &mut Locals,
        inst_name: &'static str,
    ) -> Result<FrameType, VerifyStackMapGeneralError> {
        // TODO: Don't unwrap on accessing class id
        Ok(match with_t {
            WithType::Type(pop_index) => {
                let typ = inst_types.get_pop(*pop_index);
                let typ = if let Some(typ) = typ {
                    typ
                } else {
                    tracing::error!(
                        "Missing entry at pop index {} for inst {}",
                        pop_index,
                        inst_name
                    );
                    return Err(VerifyStackMapError::NonexistentPopIndex {
                        inst_name,
                        index: *pop_index,
                    }
                    .into());
                };

                typ.clone()
            }
            // Note: This is the type held by the array at the index.
            WithType::RefArrayRefType(pop_index) => {
                let typ = inst_types.get_pop(*pop_index);
                let typ = if let Some(typ) = typ {
                    typ
                } else {
                    tracing::error!(
                        "Missing entry at pop index {} for inst {}",
                        pop_index,
                        inst_name
                    );
                    return Err(VerifyStackMapError::NonexistentPopIndex {
                        inst_name,
                        index: *pop_index,
                    }
                    .into());
                };

                // TODO: This is kinda rough
                match typ {
                    FrameType::Primitive(_) => {
                        return Err(VerifyStackMapError::RefArrayRefTypeNonArray.into())
                    }
                    FrameType::Complex(complex) => match complex {
                        ComplexFrameType::ReferenceClass(id) => {
                            let arr = classes
                                .get_array_class(
                                    class_directories,
                                    class_names,
                                    class_files,
                                    packages,
                                    *id,
                                )?
                                .ok_or(VerifyStackMapError::RefArrayRefTypeNonArray)?;
                            let elem = arr.component_type();
                            let elem_id = elem
                                .into_class_id()
                                .ok_or(VerifyStackMapError::RefArrayRefTypePrimitive)?;
                            ComplexFrameType::ReferenceClass(elem_id).into()
                        }
                        ComplexFrameType::UninitializedReferenceClass(_)
                        | ComplexFrameType::ReferenceNull => {
                            return Err(VerifyStackMapError::RefArrayRefTypeUncertainType.into())
                        }
                    },
                }
            }
            WithType::RefArrayPrimitiveLen { element_type, .. } => {
                let array_id = classes.load_array_of_primitives(class_names, *element_type)?;
                ComplexFrameType::ReferenceClass(array_id).into()
            }
            WithType::LocalVariableRefAtIndexNoRetAddr(index) => {
                // This is fine because the locals don't change in the frame while the instruction
                // is being processed
                let local =
                    locals
                        .get(*index)
                        .ok_or(VerifyStackMapError::BadLocalVariableIndex {
                            inst_name,
                            index: *index,
                        })?;
                match local {
                    Local::Unfilled => {
                        return Err(VerifyStackMapError::UninitializedLocalVariableIndex {
                            inst_name,
                            index: *index,
                        }
                        .into())
                    }
                    Local::Top => {
                        return Err(VerifyStackMapError::TopLocalVariableIndex {
                            inst_name,
                            index: *index,
                        }
                        .into())
                    }
                    Local::FrameType(local) => local.clone(),
                }
            }
            WithType::RefClassOf { class_name, .. } => {
                let id = class_names.gcid_from_bytes(class_name);
                ComplexFrameType::ReferenceClass(id).into()
            }
            WithType::IntArrayIndexInto(_idx) => PrimitiveType::Int.into(),
            WithType::LiteralInt(_val) => PrimitiveType::Int.into(),
        })
    }

    pub(crate) fn from_opcode_pop_complex_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        typ: &PopComplexType,
        last_frame_type: &FrameType,
        inst_name: &'static str,
    ) -> Result<FrameType, VerifyStackMapGeneralError> {
        // The way this type works means that if we want to ground it as a specific type
        // (which we do), we have to do some validity checking in it.
        Ok(match typ {
            PopComplexType::RefArrayPrimitiveOr(l_typ, r_typ) => {
                let l_id = classes.load_array_of_primitives(class_names, *l_typ)?;
                let r_id = classes.load_array_of_primitives(class_names, *r_typ)?;
                match last_frame_type {
                    FrameType::Primitive(_) => {
                        return Err(VerifyStackMapError::InstExpectedTypeInStackGotFrameType {
                            inst_name,
                            expected_type: PopType::Complex(typ.clone()),
                            got_type: last_frame_type.clone(),
                        }
                        .into())
                    }
                    FrameType::Complex(complex) => match complex {
                        ComplexFrameType::ReferenceClass(id)
                        | ComplexFrameType::UninitializedReferenceClass(id) => {
                            if *id == l_id {
                                ComplexFrameType::ReferenceClass(l_id).into()
                            } else if *id == r_id {
                                ComplexFrameType::ReferenceClass(r_id).into()
                            } else {
                                return Err(
                                    VerifyStackMapError::InstExpectedTypeInStackGotFrameType {
                                        inst_name,
                                        expected_type: PopType::Complex(typ.clone()),
                                        got_type: last_frame_type.clone(),
                                    }
                                    .into(),
                                );
                            }
                        }
                        ComplexFrameType::ReferenceNull => todo!(),
                    },
                }
            }
            PopComplexType::RefArrayRefAny => match last_frame_type {
                FrameType::Primitive(_) => {
                    return Err(VerifyStackMapError::InstExpectedTypeInStackGotFrameType {
                        inst_name,
                        expected_type: PopType::Complex(typ.clone()),
                        got_type: last_frame_type.clone(),
                    }
                    .into())
                }
                FrameType::Complex(complex) => match complex {
                    ComplexFrameType::ReferenceClass(id)
                    | ComplexFrameType::UninitializedReferenceClass(id) => {
                        let array_class = classes.get_array_class(
                            class_directories,
                            class_names,
                            class_files,
                            packages,
                            *id,
                        )?;
                        if let Some(array_class) = array_class {
                            if array_class.component_type().is_primitive() {
                                return Err(VerifyStackMapError::InstExpectedArrayOfReferencesGotPrimitives {
                                    inst_name,
                                    got_class: *id,
                                }.into());
                            }
                            complex.clone().into()
                        } else {
                            return Err(VerifyStackMapError::InstExpectedArrayGotClass {
                                inst_name,
                                got_class: *id,
                            }
                            .into());
                        }
                    }
                    ComplexFrameType::ReferenceNull => ComplexFrameType::ReferenceNull.into(),
                },
            },
            PopComplexType::RefArrayAny => match last_frame_type {
                FrameType::Primitive(_) => {
                    return Err(VerifyStackMapError::InstExpectedTypeInStackGotFrameType {
                        inst_name,
                        expected_type: PopType::Complex(typ.clone()),
                        got_type: last_frame_type.clone(),
                    }
                    .into())
                }
                FrameType::Complex(complex) => match complex {
                    ComplexFrameType::ReferenceClass(id)
                    | ComplexFrameType::UninitializedReferenceClass(id) => {
                        let array_class = classes.get_array_class(
                            class_directories,
                            class_names,
                            class_files,
                            packages,
                            *id,
                        )?;
                        if array_class.is_none() {
                            return Err(VerifyStackMapError::InstExpectedArrayGotClass {
                                inst_name,
                                got_class: *id,
                            }
                            .into());
                        }
                        complex.clone().into()
                    }
                    ComplexFrameType::ReferenceNull => ComplexFrameType::ReferenceNull.into(),
                },
            },
            PopComplexType::ReferenceAny => match last_frame_type {
                FrameType::Primitive(_) => {
                    return Err(VerifyStackMapError::InstExpectedTypeInStackGotFrameType {
                        inst_name,
                        expected_type: PopType::Complex(typ.clone()),
                        got_type: last_frame_type.clone(),
                    }
                    .into())
                }
                FrameType::Complex(complex) => match complex {
                    ComplexFrameType::ReferenceClass(_)
                    | ComplexFrameType::UninitializedReferenceClass(_)
                    | ComplexFrameType::ReferenceNull => complex.clone().into(),
                },
            },
        })
    }

    /// Convert from a type used in defining opcodes to one that we use for
    /// verifying frames.
    /// This means that we turn them into more strict versions of the same
    /// types, 'resolving' (not in jvm sense) them to actual types.
    /// `inst_name` field is just for debugging purposes.
    /// Note: This (and the functions it calls) should be careful in their usage of `Frame`
    /// since it will have the most recent modifications as the instruction is processed
    /// not just the ones from before the instruction was activated
    pub(crate) fn from_opcode_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        typ: &Type,
        inst_types: &InstTypes,
        locals: &mut Locals,
        inst_name: &'static str,
    ) -> Result<FrameType, VerifyStackMapGeneralError> {
        match typ {
            Type::Primitive(primitive) => Ok(FrameType::from_opcode_primitive_type(*primitive)),
            Type::Complex(complex) => FrameType::from_opcode_complex_type(class_names, complex),
            Type::With(with_t) => FrameType::from_opcode_with_type(
                classes,
                class_directories,
                class_names,
                class_files,
                packages,
                with_t,
                inst_types,
                locals,
                inst_name,
            ),
        }
    }

    pub(crate) fn from_opcode_type_no_with(
        class_names: &mut ClassNames,
        typ: &Type,
    ) -> Result<Option<FrameType>, VerifyStackMapGeneralError> {
        match typ {
            Type::Primitive(primitive) => {
                Ok(Some(FrameType::from_opcode_primitive_type(*primitive)))
            }
            Type::Complex(complex) => {
                FrameType::from_opcode_complex_type(class_names, complex).map(Some)
            }
            Type::With(_) => Ok(None),
        }
    }

    pub(crate) fn from_opcode_local_out_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        typ: &LocalVariableType,
        inst_types: &InstTypes,
        locals: &mut Locals,
        inst_name: &'static str,
    ) -> Result<FrameType, VerifyStackMapGeneralError> {
        match typ {
            LocalVariableType::Type(typ) => Self::from_opcode_type(
                classes,
                class_directories,
                class_names,
                class_files,
                packages,
                typ,
                inst_types,
                locals,
                inst_name,
            ),
        }
    }

    pub(crate) fn from_opcode_push_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        typ: &PushType,
        inst_types: &InstTypes,
        locals: &mut Locals,
        inst_name: &'static str,
    ) -> Result<FrameType, VerifyStackMapGeneralError> {
        match typ {
            PushType::Type(typ) => Self::from_opcode_type(
                classes,
                class_directories,
                class_names,
                class_files,
                packages,
                typ,
                inst_types,
                locals,
                inst_name,
            ),
        }
    }

    pub(crate) fn as_pretty_string(&self, class_names: &ClassNames) -> String {
        match self {
            FrameType::Primitive(prim) => format!("{:?}", prim),
            FrameType::Complex(complex) => complex.as_pretty_string(class_names),
        }
    }
}
impl From<PrimitiveType> for FrameType {
    fn from(prim: PrimitiveType) -> FrameType {
        FrameType::Primitive(prim)
    }
}
impl From<ComplexFrameType> for FrameType {
    fn from(complex: ComplexFrameType) -> FrameType {
        FrameType::Complex(complex)
    }
}
// TODO: Should we be moving the idea of an array out? We could keep it around and just do the comparisons
// between it and a normal referenceclass.
#[derive(Debug, Clone)]
pub enum ComplexFrameType {
    /// A reference to a class of this id
    ReferenceClass(ClassId),
    /// UninitializedThis and UninitializedVariable rolled into one
    UninitializedReferenceClass(ClassId),
    /// Null
    ReferenceNull,
}
impl ComplexFrameType {
    fn as_pretty_string(&self, class_names: &ClassNames) -> String {
        match self {
            ComplexFrameType::ReferenceClass(id) => {
                format!("#{}", class_names.tpath(*id))
            }
            ComplexFrameType::UninitializedReferenceClass(id) => {
                format!("!#{}", class_names.tpath(*id))
            }
            ComplexFrameType::ReferenceNull => "#null".to_owned(),
        }
    }
}
