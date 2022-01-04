use classfile_parser::{
    attribute_info::{
        stack_map_table_attribute_parser, InstructionIndex, StackMapFrame as StackMapFrameCF,
        StackMapTableAttribute, VerificationTypeInfo,
    },
    method_info::MethodAccessFlags,
};

use crate::{class::ClassFileData, id::ClassId, BadIdError, ClassNames};

use super::{
    method::{DescriptorType, DescriptorTypeBasic, Method, MethodDescriptor},
    CodeInfo,
};

#[derive(Debug)]
pub enum StackMapError {
    /// There should have been a stack map
    NoStackMap,
    /// Failed to parse the stack map attribute
    ParseError,
    /// The id of a descriptor type was incorrect
    BadDescriptorTypeId(BadIdError),
    /// Failed to convert a verification type to a stack map type
    VerificationTypeToStackMapTypeFailure,
}

#[derive(Debug, Clone)]
pub enum StackMapType {
    Integer,
    Float,
    // 8 Bytes
    Long,
    // 8 Bytes
    Double,
    /// An uninitialized class intance ref
    /// The id is going to be the id of the class which has this stack map type
    UninitializedThis(ClassId),
    /// Contains index of the new instruction that created (creates?) the object being stored here
    UninitializedVariable(InstructionIndex),
    /// A class instance ref
    Object(ClassId),
    Null,
    /// This is used with Category 2 types (8 bytes), with it being part of the stack types
    /// This means that in an actual stack map (rather than this abstract typechecking one), there
    /// would be a stack map entry which you shouldn't access directly because it was the other part
    /// of an 8 byte type, since in the 'physical' they take up two four byte slots.
    Top,
    // TODO: We could have a specific array object type for when we know the size due to the desc
    // type?
}
impl StackMapType {
    /// Convert a descriptor type into a StackMapType
    /// Can never return `Top`, `UninitializedThis`, `UninitializedVariable`, or `Null`
    fn from_desc(
        class_names: &mut ClassNames,
        desc: &DescriptorType,
    ) -> Result<StackMapType, BadIdError> {
        Ok(match desc {
            DescriptorType::Basic(b) => match b {
                // Many of these types are 'upgraded' in size when they are put on stack
                // because there is only really category-1 (four byte) and category-2 (eight-byte)
                // types on the stack for simplicity
                // so, byte/short/char/bool just get sign extended to the nearest, aka integer size
                // we could theoretically include more information about the types to allow greater
                // re-use later on, but for now, we just approach the problem how the jvm docs
                // describe it
                DescriptorTypeBasic::Byte
                | DescriptorTypeBasic::Char
                | DescriptorTypeBasic::Int
                | DescriptorTypeBasic::Short
                | DescriptorTypeBasic::Boolean => Self::Integer,
                DescriptorTypeBasic::Double => Self::Double,
                DescriptorTypeBasic::Float => Self::Float,
                DescriptorTypeBasic::Long => Self::Long,
                // We assume that all classes passed in are not of uninitializedThis, since
                // this shouldn't be represented as a descriptor type anyway
                DescriptorTypeBasic::Class(id) => Self::Object(*id),
            },
            // TODO: Do we need to register this?
            DescriptorType::Array { level, component } => {
                // Unwrap the option because we _know_ that it is an array
                Self::Object(desc.as_class_id(class_names)?.unwrap())
            }
        })
    }

    pub fn is_category_2(&self) -> bool {
        matches!(self, Self::Double | Self::Long)
    }

    fn from_verif_type_info(
        v: VerificationTypeInfo,
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
    ) -> Option<StackMapType> {
        Some(match v {
            VerificationTypeInfo::Integer => StackMapType::Integer,
            VerificationTypeInfo::Float => StackMapType::Float,
            VerificationTypeInfo::Double => StackMapType::Double,
            VerificationTypeInfo::Long => StackMapType::Long,
            VerificationTypeInfo::Null => StackMapType::Null,
            VerificationTypeInfo::UninitializedThis => {
                StackMapType::UninitializedThis(class_file.id())
            }
            // TODO: Return a Result instead?
            VerificationTypeInfo::Object { class } => {
                let class = class_file.get_t(class)?;
                let class_name = class_file.get_text_t(class.name_index)?;
                let class_id = class_names.gcid_from_str(class_name);
                StackMapType::Object(class_id)
            }
            VerificationTypeInfo::Uninitialized { offset } => {
                StackMapType::UninitializedVariable(InstructionIndex(offset))
            }
            // The type in verification type info is more of an unit
            // while stack map type uses Top as 'other bits of cat2 type'
            VerificationTypeInfo::Top => StackMapType::Top,
        })
    }

    /// Skips the Top type, for cases where it shouldn't appear
    fn from_verif_type_info_ignore_top(
        v: VerificationTypeInfo,
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
    ) -> Option<StackMapType> {
        if matches!(v, VerificationTypeInfo::Top) {
            None
        } else {
            Self::from_verif_type_info(v, class_names, class_file)
        }
    }
}

#[derive(Debug, Clone)]
pub struct StackMapFrame {
    pub at: InstructionIndex,
    /// Operand Stack
    pub stack: Vec<StackMapType>,
    /// Local variables
    pub locals: Vec<StackMapType>,
}
impl StackMapFrame {
    /// Whether this frame has an uninitialized this, which constrains behavior of methods that
    /// have such.
    fn has_uninit_this(&self) -> bool {
        self.locals
            .iter()
            .any(|x| matches!(x, StackMapType::UninitializedThis(_)))
    }
}
/// This currently simply stores the stack map types at each needed indice
/// but there are other methods which could be me more efficient in memory/computation
/// such as storing only the changes, which are not done yet due to complexity.
#[derive(Debug)]
pub struct StackMapFrames {
    /// There has to be at least one.
    frames: Vec<StackMapFrame>,
}
impl StackMapFrames {
    /// The given code MUST be from the passed in method
    pub fn parse_frames<'a>(
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
        method: &'a Method,
        method_code: &'a CodeInfo,
    ) -> Result<Option<StackMapFrames>, StackMapError> {
        let descriptor = method.descriptor();
        let initial_frame = {
            let this_type = if class_file.id()
                == class_names.gcid_from_slice(&["java", "lang", "Object"])
                && method.is_init()
            {
                // Object's init has some special handling
                Some(StackMapType::Object(class_file.id()))
            } else if method.is_init() {
                Some(StackMapType::UninitializedThis(class_file.id()))
            } else if !method.access_flags().contains(MethodAccessFlags::STATIC) {
                Some(StackMapType::Object(class_file.id()))
            } else {
                None
            };

            let count = descriptor.parameters().len() + if this_type.is_some() { 1 } else { 0 };
            // Not seeing if this_type is expanded is fine since it can't be a cat-2
            let mut locals = Vec::with_capacity(count);
            if let Some(this_type) = this_type {
                locals.push(this_type);
            }

            for parameter in descriptor.parameters().iter() {
                let typ = StackMapType::from_desc(class_names, parameter)
                    .map_err(StackMapError::BadDescriptorTypeId)?;
                match typ {
                    StackMapType::Integer => locals.push(typ),
                    StackMapType::Float => locals.push(typ),
                    StackMapType::Long => {
                        locals.push(typ);
                    }
                    StackMapType::Double => {
                        locals.push(typ);
                    }
                    StackMapType::Object(_) => locals.push(typ),
                    StackMapType::Top
                    | StackMapType::Null
                    | StackMapType::UninitializedVariable(_)
                    | StackMapType::UninitializedThis(_) => unreachable!(),
                }
            }

            StackMapFrame {
                at: InstructionIndex(0),
                stack: Vec::new(),
                locals,
            }
        };

        // Should always have one value when being constructed
        // NOTE: The current implementation is probably inefficient in terms of processing and memory
        // We currently store the stack map frame for each indice that has the information
        // but we could likely do a modification based version which would cut down the number of
        // allocations by a large amount, but would complicate the code.
        // It is hoped to implement that in the future, especially if it would be a notable gain.
        let stack_frames = vec![initial_frame];
        let mut stack_frames = StackMapFrames {
            frames: stack_frames,
        };

        // Stack map table attribute
        let smt = method_code
            .attributes()
            .iter()
            .find(|x| class_file.get_text_t(x.attribute_name_index) == Some("StackMapTable"));

        let smt = if let Some(smt) = smt {
            let (rem_data, smt) = stack_map_table_attribute_parser(&smt.info)
                .map_err(|_| StackMapError::ParseError)?;
            debug_assert!(rem_data.is_empty());
            smt
        } else if class_file.version().map(|x| x.major <= 50).unwrap_or(false) {
            // TODO: Allow nonexistent stack map table for earlier versions of the bytecode
            // We will have to figure out how the type inference is meant to work
            tracing::warn!("Class File Version: {:?}", class_file.version());
            return Err(StackMapError::NoStackMap.into());
        } else {
            // There is no entries
            StackMapTableAttribute {
                number_of_entries: 0,
                entries: Vec::new(),
            }
        };

        for frame in smt.entries {
            // Note: For some entries, data is stored within the frame_type
            match frame {
                // Has the same local variables but an empty operand stack
                // So only loosely the same frame
                StackMapFrameCF::SameFrame { frame_type } => {
                    // The offset delta for this frame is just its frame type
                    let offset_delta = frame_type;

                    let frame = StackMapFrame {
                        at: stack_frames.get_new_index(offset_delta.into()),
                        stack: Vec::new(),
                        locals: stack_frames.last_r().locals.clone(),
                    };
                    stack_frames.push(frame);
                }
                // Frame which has the same local variables as the previous frame and that the
                // operand stack has 1 entry
                StackMapFrameCF::SameLocals1StackItemFrame { frame_type, stack } => {
                    // The start of the frame types for SameLocals1StackItemFrame
                    const SAME_LOCALS_1_ITEM_START: u8 = 64;
                    // TODO: checked sub
                    let offset_delta = frame_type - SAME_LOCALS_1_ITEM_START;
                    let stack = StackMapType::from_verif_type_info(stack, class_names, class_file)
                        .ok_or(StackMapError::VerificationTypeToStackMapTypeFailure)?;
                    let frame = StackMapFrame {
                        at: stack_frames.get_new_index(offset_delta.into()),
                        stack: vec![stack],
                        locals: stack_frames.last_r().locals.clone(),
                    };
                    stack_frames.push(frame);
                }
                // Similar to the SameLocals1StackItemFrame, but this explicitly includes the
                // offset_delta.
                // This has the same local variables as the previous frame and the operand stack
                // has 1 entry.
                StackMapFrameCF::SameLocals1StackItemFrameExtended {
                    frame_type,
                    offset_delta,
                    stack,
                } => {
                    let stack = StackMapType::from_verif_type_info(stack, class_names, class_file)
                        .ok_or(StackMapError::VerificationTypeToStackMapTypeFailure)?;
                    let frame = StackMapFrame {
                        at: stack_frames.get_new_index(offset_delta),
                        stack: vec![stack],
                        locals: stack_frames.last_r().locals.clone(),
                    };
                    stack_frames.push(frame);
                }
                // Same local variables as previous frame,
                // except the last several local variables are absent
                // operand staa=ck is empty
                StackMapFrameCF::ChopFrame {
                    frame_type,
                    offset_delta,
                } => {
                    // The entry right after the frame types that chop frame occupies
                    const CHOP_FRAME_END: u8 = 251;
                    // TODO: checked_sub
                    let missing = CHOP_FRAME_END - frame_type;
                    let existing = stack_frames.last_r().locals.len();

                    // TODO: Checked sub
                    let new_count = existing - usize::from(missing);

                    let locals = stack_frames
                        .last_r()
                        .locals
                        .iter()
                        .take(new_count)
                        .cloned()
                        .collect::<Vec<_>>();
                    debug_assert_eq!(locals.len(), new_count);

                    let frame = StackMapFrame {
                        at: stack_frames.get_new_index(offset_delta),
                        stack: Vec::new(),
                        locals,
                    };
                    stack_frames.push(frame);
                }
                // Same local variables
                // Empty operand stack
                // This is like SameFrame, except that the offset delta is given directly
                StackMapFrameCF::SameFrameExtended {
                    frame_type,
                    offset_delta,
                } => {
                    let frame = StackMapFrame {
                        at: stack_frames.get_new_index(offset_delta),
                        stack: Vec::new(),
                        locals: stack_frames.last_r().locals.clone(),
                    };
                    stack_frames.push(frame);
                }
                // Same locals, but with some additional locals
                // Empty operand stack
                StackMapFrameCF::AppendFrame {
                    frame_type,
                    offset_delta,
                    locals,
                } => {
                    // The entry right before the start of the frame types for AppendFrame
                    const APPEND_FRAME_PRE_START: u8 = 251;
                    debug_assert_eq!((frame_type - APPEND_FRAME_PRE_START) as usize, locals.len());

                    let frame = StackMapFrame {
                        at: stack_frames.get_new_index(offset_delta),
                        stack: Vec::new(),
                        locals: {
                            let mut new_locals = stack_frames.last_r().locals.clone();
                            let locals = iter_verif_to_stack_map_types(
                                locals.into_iter(),
                                class_names,
                                class_file,
                            )?;
                            new_locals.extend(locals.into_iter());
                            new_locals
                        },
                    };
                    stack_frames.push(frame);
                }
                // Specifies all the information
                StackMapFrameCF::FullFrame {
                    frame_type: _frame_type,
                    offset_delta,
                    number_of_locals,
                    locals,
                    number_of_stack_items,
                    stack,
                } => {
                    debug_assert_eq!(locals.len(), number_of_locals as usize);
                    debug_assert_eq!(stack.len(), number_of_stack_items as usize);

                    let locals =
                        iter_verif_to_stack_map_types(locals.into_iter(), class_names, class_file)?;
                    let stack =
                        iter_verif_to_stack_map_types(stack.into_iter(), class_names, class_file)?;

                    let frame = StackMapFrame {
                        at: stack_frames.get_new_index(offset_delta),
                        stack,
                        locals,
                    };
                    stack_frames.push(frame);
                }
            }
        }

        // TODO: Don't expand here. It would probably be more efficient to expand in the frames
        // but that requires special handling.

        // We expand category two types, since they don't have `Top` written explicitly.
        for frame in stack_frames.frames.iter_mut() {
            let mut output = Vec::new();
            let mut locals_iter = std::mem::take(&mut frame.locals).into_iter().peekable();
            while let Some(local) = locals_iter.next() {
                if matches!(local, StackMapType::Double | StackMapType::Long) {
                    output.push(local);
                    output.push(StackMapType::Top);
                } else {
                    output.push(local);
                }
            }
            frame.locals = output;
        }

        Ok(Some(stack_frames))
    }

    /// Returns the last stack frame, which has to exist
    fn last_r(&self) -> &StackMapFrame {
        self.frames
            .last()
            .expect("Expected at least one stack frame")
    }

    fn push(&mut self, frame: StackMapFrame) {
        self.frames.push(frame);
    }

    /// Get the new instruction index for a stack frame that would be added
    fn get_new_index(&self, offset_delta: u16) -> InstructionIndex {
        if self.frames.len() == 1 {
            // Offsets from the initial stack frame are just the offset
            InstructionIndex(offset_delta)
        } else {
            // TODO: checked add
            let last = self.last_r();
            InstructionIndex(last.at.0 + offset_delta + 1)
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, StackMapFrame> {
        self.frames.iter()
    }

    pub fn into_vec(self) -> Vec<StackMapFrame> {
        self.frames
    }
}

fn iter_verif_to_stack_map_types(
    iter: impl Iterator<Item = VerificationTypeInfo>,
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
) -> Result<Vec<StackMapType>, StackMapError> {
    iter.map(|v| {
        StackMapType::from_verif_type_info(v, class_names, class_file)
            .ok_or(StackMapError::VerificationTypeToStackMapTypeFailure)
    })
    .collect::<Result<Vec<_>, _>>()
}
