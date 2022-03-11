use std::borrow::Cow;

use classfile_parser::{
    attribute_info::{
        stack_map_table_attribute_parser, InstructionIndex, StackMapFrame as StackMapFrameCF,
        StackMapTableAttribute, VerificationTypeInfo,
    },
    method_info::MethodAccessFlags,
};
use smallvec::SmallVec;

use crate::{class::ClassFileData, data::class_names::ClassNames, id::ClassId, BadIdError};

use super::{
    method::{DescriptorType, DescriptorTypeBasic, Method},
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
    /// Convert a descriptor type into a [`StackMapType`]
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
            DescriptorType::Array { .. } => {
                // Unwrap the option because we _know_ that it is an array
                Self::Object(desc.as_class_id(class_names)?.unwrap())
            }
        })
    }

    #[must_use]
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
                let class_name = class_file.get_text_b(class.name_index)?;
                let class_id = class_names.gcid_from_bytes(class_name);
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
}

#[derive(Debug, Clone)]
pub struct StackMapFrame {
    pub at: InstructionIndex,
    /// Operand Stack
    pub stack: SmallVec<[StackMapType; 4]>,
    /// Local variables
    pub locals: SmallVec<[StackMapType; 8]>,
}
impl StackMapFrame {
    fn new_locals(at: InstructionIndex, locals: SmallVec<[StackMapType; 8]>) -> StackMapFrame {
        StackMapFrame {
            at,
            stack: SmallVec::new(),
            locals,
        }
    }
}

/// The start of the `SameLocals1Item` frame, used for computing the `offset_delta`
/// since it is encoded in the frame type
const SAME_LOCALS_1_ITEM_START: u8 = 64;

/// Used to parse the stack map frames one at a time
/// This avoids unecessary allocations by keeping the current frame inside it
/// and reusing it for future frames, since they rely on previous frames.
pub struct StackMapFramesProcessor {
    table: StackMapTableAttribute,
    current_frame: StackMapFrame,
    /// None signifies that this we are at the initial frame
    /// Note that when it is Some, the value inside might not be a valid index
    next_table_index: Option<usize>,
    // This becomes None when there is no more instructions for it to process
    next_instruction_index: Option<InstructionIndex>,
}
impl StackMapFramesProcessor {
    pub fn new<'a>(
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
        method: &'a Method,
        method_code: &'a CodeInfo,
    ) -> Result<StackMapFramesProcessor, StackMapError> {
        let descriptor = method.descriptor();
        // TODO: Should we skip adding the initial frame if it is empty? Some code may rely on it
        // being nonempty
        let initial_frame = {
            let this_type = if class_file.id() == class_names.object_id() && method.is_init() {
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
            let mut locals = SmallVec::with_capacity(count);
            if let Some(this_type) = this_type {
                locals.push(this_type);
            }

            for parameter in descriptor.parameters().iter() {
                let typ = StackMapType::from_desc(class_names, parameter)
                    .map_err(StackMapError::BadDescriptorTypeId)?;
                match typ {
                    StackMapType::Integer | StackMapType::Float | StackMapType::Object(_) => {
                        locals.push(typ)
                    }
                    StackMapType::Long | StackMapType::Double => {
                        locals.push(typ);
                        locals.push(StackMapType::Top);
                    }

                    StackMapType::Top
                    | StackMapType::Null
                    | StackMapType::UninitializedVariable(_)
                    | StackMapType::UninitializedThis(_) => unreachable!(),
                }
            }

            StackMapFrame::new_locals(InstructionIndex(0), locals)
        };

        // Stack map table attribute
        let smt = method_code.attributes().iter().find(|x| {
            class_file.get_text_t(x.attribute_name_index) == Some(Cow::Borrowed("StackMapTable"))
        });

        // TODO: Performance with this could be improved notably by only parsing the table to get
        // what we need, and could potentially avoid all the allocations besides the essential.
        // TODO: Once we get that, it may even be possible to get rid of the remaining allocations
        // through returning an iterator for locals/stack, so that the caller can feed them into
        // their own vectors.
        let smt = if let Some(smt) = smt {
            let (rem_data, smt) =
                stack_map_table_attribute_parser(class_file.parse_data_for(smt.info.clone()))
                    .map_err(|_| StackMapError::ParseError)?;
            debug_assert!(rem_data.is_empty());
            smt
        } else if class_file.version().map_or(false, |x| x.major <= 50) {
            // TODO: Allow nonexistent stack map table for earlier versions of the bytecode
            // We will have to figure out how the type inference is meant to work
            tracing::warn!("Class File Version: {:?}", class_file.version());
            return Err(StackMapError::NoStackMap);
        } else {
            // There is no entries
            StackMapTableAttribute {
                number_of_entries: 0,
                entries: Vec::new(),
            }
        };

        Ok(StackMapFramesProcessor {
            table: smt,
            current_frame: initial_frame,
            next_instruction_index: Some(InstructionIndex(0)),
            // Next is the initial frame!
            next_table_index: None,
        })
    }

    #[must_use]
    /// Check whether the next frame is at this index
    /// Should likely be used to decide whether to call `next_frame`
    /// Note that since this only checks the next index, this could have issues for
    /// malformed class files where things are out of order, but they can't be out of order
    /// due to the way it does the adding!
    /// Though, it won't let you skip ahead, even if you really want to do that.
    pub fn has_next_frame_at(&self, idx: InstructionIndex) -> bool {
        self.next_instruction_index == Some(idx)
    }

    /// Updates the held next instruction index because it is likely to be checked
    /// This also takes the job of skipping over any `SameFrame`'s that only affect the initial
    /// frame, which are generated by the JVM at times
    fn update_next_instruction_index(&mut self, is_direct_initial: bool, with_table_index: usize) {
        let entry = if let Some(entry) = self.table.entries.get(with_table_index) {
            entry
        } else {
            // Otherwise, there is no entry, so we set it to None
            self.next_instruction_index = None;
            return;
        };

        let offset_delta = match entry {
            StackMapFrameCF::SameFrame { frame_type } => {
                let offset_delta = u16::from(*frame_type);
                if offset_delta == 0 && is_direct_initial {
                    // TODO: This issue might imply that we also need to be more carefuly with other
                    // types of frame-zero frames, since those could maybe be valid but we don't
                    // currently handle them.
                    // We may simply have to write some code that peeks forward and processes as
                    // many frames that are after the initial frame if they're no-ops.

                    // There can be a same frame that is at the first entry
                    // and so its not advanced by 1
                    // and so is at index 0, but we've already generated the initial frame
                    // and it cannot even have the side effect of removing the stack
                    // because initial frames don't get a stack!
                    // So we skip past it to keep the implementation somewhat simpler

                    self.next_table_index = self.next_table_index.map(|x| x + 1);
                    self.update_next_instruction_index(false, with_table_index + 1);

                    return;
                }

                offset_delta
            }
            // TODO: Checked sub
            StackMapFrameCF::SameLocals1StackItemFrame { frame_type, .. } => {
                u16::from(frame_type - SAME_LOCALS_1_ITEM_START)
            }

            StackMapFrameCF::ChopFrame { offset_delta, .. }
            | StackMapFrameCF::SameLocals1StackItemFrameExtended { offset_delta, .. }
            | StackMapFrameCF::SameFrameExtended { offset_delta, .. }
            | StackMapFrameCF::AppendFrame { offset_delta, .. }
            | StackMapFrameCF::FullFrame { offset_delta, .. } => *offset_delta,
        };

        let next_instruction_index = if is_direct_initial {
            // Offsets from the initial stack frame are just the offset
            InstructionIndex(offset_delta)
        } else {
            InstructionIndex(self.current_frame.at.0 + offset_delta + 1)
        };

        self.next_instruction_index = Some(next_instruction_index);
    }

    // TODO: In the code that uses this we could technically specialize the parsing code for the
    // case where there are no more frames and so it doesn't need to check anymore.
    /// Parses the next frame and returns a reference to it
    /// The existing allocation is reused.
    pub fn next_frame(
        &mut self,
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
    ) -> Result<Option<&StackMapFrame>, StackMapError> {
        let (self_table_index, next_frame) = if let Some(index) = self.next_table_index {
            if let Some(frame) = self.table.entries.get(index) {
                self.next_table_index = Some(index + 1);
                (index, frame)
            } else {
                // We've finished
                return Ok(None);
            }
        } else {
            // Current frame is the initial frame.
            self.next_table_index = Some(0);
            self.update_next_instruction_index(true, 0);
            return Ok(Some(&self.current_frame));
        };

        // TODO: We could make Rust generate two generic functions for whether it is directly
        // after the initial frame or not, to avoid useless branching

        // Get the last inst index for the previous frame, because we need that to compute
        // this frames inst index
        let last_index = self.current_frame.at;
        // Whether this is the entry right after the initial entry
        let is_direct_initial = self_table_index == 0;
        let next_frame_index = |offset_delta: u16| {
            if is_direct_initial {
                // Offsets from the initial stack frame are just the offset
                InstructionIndex(offset_delta)
            } else {
                InstructionIndex(last_index.0 + offset_delta + 1)
            }
        };

        match next_frame {
            StackMapFrameCF::SameFrame { frame_type } => {
                let offset_delta = *frame_type;

                self.current_frame.at = next_frame_index(u16::from(offset_delta));
                // Despite being 'same frame', the stack is empty
                self.current_frame.stack.clear()
                // but the locals are unmodified
            }
            StackMapFrameCF::SameLocals1StackItemFrame { frame_type, stack } => {
                // TODO: Checked subtraction
                let offset_delta = frame_type - SAME_LOCALS_1_ITEM_START;
                self.current_frame.at = next_frame_index(u16::from(offset_delta));
                // There is a single value inside the frame
                let stack = StackMapType::from_verif_type_info(*stack, class_names, class_file)
                    .ok_or(StackMapError::VerificationTypeToStackMapTypeFailure)?;
                self.current_frame.stack.clear();
                self.current_frame.stack.push(stack);
                // locals are unchanged
            }
            // Similar to the SameLocals1StackItemFrame, but this explicitly includes the offset
            // This has the same local variables as the previous frame and the stack has one entry
            StackMapFrameCF::SameLocals1StackItemFrameExtended {
                offset_delta,
                stack,
                ..
            } => {
                self.current_frame.at = next_frame_index(*offset_delta);

                let stack = StackMapType::from_verif_type_info(*stack, class_names, class_file)
                    .ok_or(StackMapError::VerificationTypeToStackMapTypeFailure)?;
                self.current_frame.stack.clear();
                self.current_frame.stack.push(stack);

                // locals are unchanged
            }
            // Removes a certain amount of local variables at the end
            StackMapFrameCF::ChopFrame {
                frame_type,
                offset_delta,
            } => {
                const CHOP_FRAME_END: u8 = 251;

                self.current_frame.at = next_frame_index(*offset_delta);

                // TODO: checked sub
                // The amount of entries that should be chopped off is stored in frame type
                let missing = CHOP_FRAME_END - frame_type;

                // This cuts off the expanded versions, since the chop frame refers to long/doubles
                // as one entry, but we want to store them as the expanded because of how the code
                // works.
                // TODO: This could do better. Likely it could just count the amount needed to pop
                // off, and then resize to smaller size, and it would work better
                let mut consumed = 0;
                while consumed < missing {
                    // TODO: Don't unwrap on a missing value, that just means the frames are bad
                    let last = self.current_frame.locals.last().unwrap();
                    if matches!(last, StackMapType::Top) {
                        if let Some(last_before_idx) =
                            self.current_frame.locals.len().checked_sub(2)
                        {
                            // It has to exist
                            let last_before =
                                self.current_frame.locals.get(last_before_idx).unwrap();
                            if matches!(last_before, StackMapType::Long | StackMapType::Double) {
                                // Pop the top, the fall through will pop the cat2 type
                                self.current_frame.locals.pop();
                            }
                        }
                        // otherwise, it is a lone Top (which does occur), and is just popped
                    }

                    self.current_frame.locals.pop();
                    consumed += 1;
                }
                // Stack is empty
                self.current_frame.stack.clear()
            }
            // Same local variables
            // but no stack
            // SameFrame but with the offset delta given explicitly
            StackMapFrameCF::SameFrameExtended { offset_delta, .. } => {
                self.current_frame.at = next_frame_index(*offset_delta);
                self.current_frame.stack.clear();
            }
            StackMapFrameCF::AppendFrame {
                offset_delta,
                locals,
                ..
            } => {
                self.current_frame.at = next_frame_index(*offset_delta);
                // Stack is empty
                self.current_frame.stack.clear();

                append_frame_verif_to_locals(
                    class_names,
                    class_file,
                    &mut self.current_frame.locals,
                    locals.as_slice(),
                )?;
            }
            StackMapFrameCF::FullFrame {
                offset_delta,
                locals,
                stack,
                ..
            } => {
                self.current_frame.at = next_frame_index(*offset_delta);
                // Both stack and locals are being overwritten
                self.current_frame.stack.clear();
                self.current_frame.locals.clear();

                for verif in stack {
                    let verif = StackMapType::from_verif_type_info(*verif, class_names, class_file)
                        .ok_or(StackMapError::VerificationTypeToStackMapTypeFailure)?;
                    self.current_frame.stack.push(verif);
                }

                append_frame_verif_to_locals(
                    class_names,
                    class_file,
                    &mut self.current_frame.locals,
                    locals,
                )?;
            }
        }

        self.update_next_instruction_index(false, self_table_index + 1);

        Ok(Some(&self.current_frame))
    }
}

/// Append the verification types, expanding them as needed
/// Should only be used for Locals, since that is where we expand.
fn append_frame_verif_to_locals<const N: usize>(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
    data: &mut SmallVec<[StackMapType; N]>,
    verif: &[VerificationTypeInfo],
) -> Result<(), StackMapError> {
    for v in verif {
        let stack = StackMapType::from_verif_type_info(*v, class_names, class_file)
            .ok_or(StackMapError::VerificationTypeToStackMapTypeFailure)?;
        let is_category_2 = stack.is_category_2();
        data.push(stack);
        if is_category_2 {
            data.push(StackMapType::Top);
        }
    }

    Ok(())
}
