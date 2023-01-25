#![warn(clippy::pedantic)]
// The design of this library tends towards this, and grouping them together makes it harder to
// minimize dependencies on the data.
#![allow(clippy::too_many_arguments)]
// Clippy just isn't smart enough.
#![allow(clippy::needless_pass_by_value)]
// Not really useful.
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::similar_names)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::too_many_lines)]

use classfile_parser::constant_info::ConstantInfo;

use classfile_parser::{
    attribute_info::InstructionIndex,
    constant_info::{ClassConstant, FieldRefConstant, NameAndTypeConstant, Utf8Constant},
    constant_pool::ConstantPoolIndexRaw,
};
use rhojvm_base::class::ClassFileInfo;
use rhojvm_base::code::op::InstMapFunc;
use rhojvm_base::code::stack_map::{StackMapError, StackMapFramesProcessor};
use rhojvm_base::code::types::{
    Category, Instruction, LocalVariableInType, LocalVariableIndex, LocalVariableType, LocalsIn,
    LocalsOutAt, StackSizes,
};
use rhojvm_base::data::class_files::ClassFiles;
use rhojvm_base::data::class_names::ClassNames;
use rhojvm_base::data::classes::Classes;
use rhojvm_base::data::methods::Methods;
use rhojvm_base::id::{ExactMethodId, MethodIndex};
use rhojvm_base::{
    code::{
        op::Inst,
        stack_map::StackMapType,
        types::{PopIndex, PopType, PopTypeAt, PrimitiveType, PushType, PushTypeAt, Type},
        CodeInfo,
    },
    id::ClassId,
    package::Packages,
    StepError,
};
use smallvec::SmallVec;
use types::{ComplexFrameType, FrameType};

use crate::types::InstTypes;

mod types;

#[derive(Debug)]
pub enum VerifyStackMapGeneralError {
    StepError(StepError),
    VerifyStackMapError(VerifyStackMapError),
    StackMapError(StackMapError),
}
impl From<StepError> for VerifyStackMapGeneralError {
    fn from(err: StepError) -> Self {
        VerifyStackMapGeneralError::StepError(err)
    }
}
impl From<VerifyStackMapError> for VerifyStackMapGeneralError {
    fn from(err: VerifyStackMapError) -> Self {
        VerifyStackMapGeneralError::VerifyStackMapError(err)
    }
}

// TODO: Include method id?
#[derive(Debug)]
pub enum VerifyStackMapError {
    /// The local at the frame (likely a received frame) had a category 2
    /// type but had no Top entry after it
    LocalCategory2HadNoTop,
    /// Tried setting a category 2 value at the index but there wasn't enough space to put Top
    LocalSetCategory2NoSpaceForTop {
        inst_name: &'static str,
        base_index: LocalVariableIndex,
    },
    /// Tried setting a category 2 value at the index but the place where it should put top
    /// alreday had a value.
    LocalSetCategory2TopHadValue {
        inst_name: &'static str,
        base_index: LocalVariableIndex,
    },
    /// Expected a type but did not find any value
    InstExpectedTypeInStack {
        // TODO: New enum that is the name of each instruction?
        inst_name: &'static str,
        expected_type: PopType,
    },
    InstExpectedFrameTypeInStack {
        inst_name: &'static str,
        expected_type: FrameType,
    },
    InstExpectedTypeInStackGot {
        inst_name: &'static str,
        expected_type: PopType,
        got_type: StackMapType,
    },
    InstExpectedTypeInStackGotFrameType {
        inst_name: &'static str,
        expected_type: PopType,
        got_type: FrameType,
    },
    InstExpectedFrameTypeInStackGotFrameType {
        inst_name: &'static str,
        expected_type: FrameType,
        got_type: FrameType,
    },
    InstExpectedArrayGotClass {
        inst_name: &'static str,
        got_class: ClassId,
    },
    InstExpectedArrayOfReferencesGotPrimitives {
        inst_name: &'static str,
        got_class: ClassId,
    },
    InstExpectedCategory1GotFrameType {
        inst_name: &'static str,
        got_type: FrameType,
    },
    InstExpectedCategory2GotFrameType {
        inst_name: &'static str,
        got_type: FrameType,
    },
    /// The frame had more locals than the method allows
    ReceivedFrameTooManyLocals {
        inst_name: &'static str,
        inst_index: InstructionIndex,
    },
    /// There was no Top, or anything, to pop after the cat2 type
    Category2HadNoTop { inst_name: &'static str },
    /// There was a value to pop after the cat2 type, but it was not Top
    Category2HadWrongTop {
        inst_name: &'static str,
        wrong_type: StackMapType,
    },
    /// There was no data at that index, when there should have been a `new`
    /// instruction
    UninitializedVariableBadIndex { idx: InstructionIndex },
    /// There was an instruction at the index but it was not a `new` instruction
    UninitializedVariableIncorrectInstruction {
        idx: InstructionIndex,
        /// The name of the incorrect instruction
        inst_name: &'static str,
    },
    /// The index held by a `new` instruction (retrieved for
    /// `UninitializedVariable`) was invalid
    BadNewClassIndex {
        index: ConstantPoolIndexRaw<ClassConstant>,
    },
    BadNewClassNameIndex {
        index: ConstantPoolIndexRaw<Utf8Constant>,
    },
    /// The pop index that was desired didn't exist
    /// This is likely a sign of a bug with the library itself
    NonexistentPopIndex {
        inst_name: &'static str,
        index: PopIndex,
    },
    /// The ref array ref type (withtype) referenced a type that was not an
    /// array
    RefArrayRefTypeNonArray,
    /// The ref array ref type referenced a type that was a primitive, when it
    /// should be a reference.
    RefArrayRefTypePrimitive,
    /// The type was uncertain. This should hopefully never actually occur, but
    /// if it does then it may be further indication that some redesign needs
    /// to be done
    RefArrayRefTypeUncertainType,
    /// A bad index for a class into the constant pool
    BadClassIndex(ConstantPoolIndexRaw<ClassConstant>),
    /// A bad index for a class name into the constant pool
    BadClassNameIndex(ConstantPoolIndexRaw<Utf8Constant>),
    /// A bad index for a field into the constant pool
    BadFieldIndex(ConstantPoolIndexRaw<FieldRefConstant>),
    /// A bad index for a field's name and type into the constant pool
    BadFieldNatIndex(ConstantPoolIndexRaw<NameAndTypeConstant>),
    /// A bad index for a field's descriptor into the constant pool
    BadFieldDescriptorIndex(ConstantPoolIndexRaw<Utf8Constant>),
    /// There was an error in parsing the field descriptor
    InvalidFieldDescriptor(classfile_parser::descriptor::DescriptorTypeError),
    /// There was a failure in parsing the field type.
    /// This might be a library error or a problem with the class file
    UnparsedFieldType,
    /// The definition of a multidimensional array specified zero dimensions
    MultidimensionalArrayZeroDimensions,
    /// The index was outside of the allowed bounds for local variables in this method
    BadLocalVariableIndex { inst_name: &'static str, index: u16 },
    /// Expected a type to be stored in the local variable, got a different type
    ExpectedLocalVariableType {
        inst_name: &'static str,
        expected_type: LocalVariableInType,
        got_type: Local,
    },
    /// The index for a local variable was valid, but there was no type there to read
    UninitializedLocalVariableIndex { inst_name: &'static str, index: u16 },
    /// The index accessed a Top
    TopLocalVariableIndex { inst_name: &'static str, index: u16 },
    /// The index for a constant into the constant pool was invalid
    BadConstantIndex(ConstantPoolIndexRaw<ConstantInfo>),
    /// The type at the index was not an accepted constant info
    BadConstantType,
}

/// Settings for logging in the stack map verification.
#[derive(Debug, Clone, Default)]
pub struct StackMapVerificationLogging {
    /// Whether to log the name of the method and class as we start verifying it
    pub log_method_name: bool,
    /// Whether to log each frame received from the class file.
    /// Should be paired with `log_instruction` to know which instruction it was located at
    pub log_received_frame: bool,
    /// Whether to log each instruction as they are processed
    pub log_instruction: bool,
    /// Whether to log each PUSH/POP
    /// intended to be used with `log_instruction` but can be standalone
    pub log_stack_modifications: bool,
    /// Whether to log each READ/WRITE to local variables
    /// intended to be used with `log_instruction` but can be standalone
    pub log_local_variable_modifications: bool,
    // TODO: Option to log individual frame parts
}

/// Variants of this enumeration are unstable and should not be relied upon.
#[derive(Debug, Clone)]
pub enum Local {
    /// It has not yet received a type but can hold one
    Unfilled,
    /// It is the top type, unfortunately
    Top,
    FrameType(FrameType),
}
impl Local {
    fn is_category_2(&self) -> bool {
        match self {
            Local::Unfilled | Local::Top => false,
            Local::FrameType(x) => match x {
                FrameType::Primitive(prim) => prim.is_category_2(),
                FrameType::Complex(_) => false,
            },
        }
    }

    fn as_pretty_string(&self, class_names: &ClassNames) -> String {
        match self {
            Local::Unfilled => "Unfilled".to_owned(),
            Local::Top => "Top".to_owned(),
            Local::FrameType(frame_type) => frame_type.as_pretty_string(class_names),
        }
    }
}

#[derive(Debug)]
struct Locals {
    locals: SmallVec<[Local; 16]>,
}
impl Default for Locals {
    fn default() -> Locals {
        Locals {
            locals: SmallVec::new(),
        }
    }
}
impl Locals {
    fn push(&mut self, v: Local) {
        self.locals.push(v);
    }

    fn len(&self) -> usize {
        self.locals.len()
    }

    fn resize_to(&mut self, len: usize) {
        self.locals.resize(len, Local::Unfilled);
    }

    fn ingest_stack_map_types(
        &mut self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        code: &CodeInfo,
        types: &[StackMapType],
    ) -> Result<(), VerifyStackMapGeneralError> {
        self.locals.clear();

        let mut types_iter = types.iter().peekable();
        while let Some(typ) = types_iter.next() {
            match typ {
                StackMapType::Integer => {
                    self.push(Local::FrameType(FrameType::Primitive(PrimitiveType::Int)));
                }
                StackMapType::Float => {
                    self.push(Local::FrameType(FrameType::Primitive(PrimitiveType::Float)));
                }
                StackMapType::Long => {
                    // Skip the top type
                    if !matches!(types_iter.next(), Some(StackMapType::Top)) {
                        return Err(VerifyStackMapError::LocalCategory2HadNoTop.into());
                    }
                    self.push(Local::FrameType(FrameType::Primitive(PrimitiveType::Long)));
                    self.push(Local::Top);
                }
                StackMapType::Double => {
                    // Skip the top type
                    if !matches!(types_iter.next(), Some(StackMapType::Top)) {
                        return Err(VerifyStackMapError::LocalCategory2HadNoTop.into());
                    }
                    self.push(Local::FrameType(FrameType::Primitive(
                        PrimitiveType::Double,
                    )));
                    self.push(Local::Top);
                }
                StackMapType::UninitializedThis(id) => {
                    self.push(Local::FrameType(FrameType::Complex(
                        ComplexFrameType::UninitializedReferenceClass(*id),
                    )));
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

                    self.push(Local::FrameType(FrameType::Complex(
                        ComplexFrameType::UninitializedReferenceClass(class_id),
                    )));
                }
                StackMapType::Object(id) => self.push(Local::FrameType(FrameType::Complex(
                    ComplexFrameType::ReferenceClass(*id),
                ))),
                StackMapType::Null => self.push(Local::FrameType(FrameType::Complex(
                    ComplexFrameType::ReferenceNull,
                ))),
                StackMapType::Top => {
                    // Category 2 types manually grab their Top
                    // The received frames use Top as essentially an empty spot, so we just use
                    // unfilled here.
                    self.push(Local::Unfilled);
                }
            };
        }
        Ok(())
    }

    fn get(&self, index: LocalVariableIndex) -> Option<&Local> {
        self.locals.get(usize::from(index))
    }

    fn set(
        &mut self,
        inst_name: &'static str,
        index: LocalVariableIndex,
        value: Local,
    ) -> Result<(), VerifyStackMapError> {
        let uindex = usize::from(index);
        if uindex >= self.len() {
            return Err(VerifyStackMapError::BadLocalVariableIndex { inst_name, index });
        }

        let current = &self.locals[uindex];

        match &current {
            Local::Unfilled => {}
            Local::Top => {
                if let Some(prev_index) = uindex.checked_sub(1) {
                    if self.locals[prev_index].is_category_2() {
                        // Reset it, since it has been invalidated
                        self.locals[prev_index] = Local::Unfilled;
                    } else {
                        tracing::warn!("Set got non category 2 before top {:#?}", self);
                    }
                }
                self.locals[uindex] = Local::Unfilled;
            }
            Local::FrameType(x) => {
                if let FrameType::Primitive(PrimitiveType::Long | PrimitiveType::Double) = x {
                    let next_index = usize::from(index + 1);
                    if let Some(Local::Top) = self.locals.get(next_index) {
                        self.locals[next_index] = Local::Unfilled;
                    } else {
                        tracing::warn!("Category-2 did not have Top after it, {:#?}", self);
                    }
                } else {
                }
            }
        }

        if let Local::FrameType(FrameType::Primitive(PrimitiveType::Long | PrimitiveType::Double)) =
            value
        {
            let next_index = usize::from(index + 1);
            match self.locals.get(next_index) {
                Some(Local::Unfilled) => {
                    self.locals[next_index] = Local::Top;
                }
                Some(Local::Top) => { /* no-op */ }
                Some(_) => {
                    tracing::warn!("Changing type in locals via writing of category-two");
                    return Err(VerifyStackMapError::LocalSetCategory2TopHadValue {
                        inst_name,
                        base_index: index,
                    });
                }
                None => {
                    return Err(VerifyStackMapError::LocalSetCategory2NoSpaceForTop {
                        inst_name,
                        base_index: index,
                    })
                }
            }
        }
        self.locals[uindex] = value;

        Ok(())
    }
}

#[derive(Debug)]
struct Frame {
    at: InstructionIndex,
    stack: SmallVec<[FrameType; 20]>,
    locals: Locals,
}
impl Frame {
    fn stack_sizes(&self) -> StackSizes {
        let mut res = [None; 4];
        for (i, entry) in self.stack.iter().rev().take(res.len()).enumerate() {
            res[i] = Some(if entry.is_category_1() {
                Category::One
            } else {
                Category::Two
            });
        }
        res
    }
}

/// Verify the type safety of a method's code using stack maps
/// `class_names` should have the name of the class file within it
/// `class_files` does not have to have `class_file` in it (or a duplicate, since mut/immut can't
/// be merged anyway)
/// `methods` has `method_id` loaded from it
/// `method_code` MUST be the same code that the method would have.
///   You may note that it wouldn't be possible to load the method from `methods` and pass it in
///   Due to the design of the code, you will possibly have to clone it.
///   If there is no code, then the code is verified (assuming that it shouldn't have code)
/// # Panics
pub fn verify_type_safe_method_stack_map(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    conf: StackMapVerificationLogging,
    class_file: &ClassFileInfo,
    method_index: MethodIndex,
    method_code: &CodeInfo,
) -> Result<(), VerifyStackMapGeneralError> {
    let _span = tracing::span!(tracing::Level::TRACE, "stackmap verification").entered();

    let class_id = class_file.id();
    let method_id = ExactMethodId::unchecked_compose(class_id, method_index);

    methods.load_method_from_index(class_names, class_file, method_index)?;
    let method = methods.get_mut(&method_id).unwrap();
    method.load_code(class_files)?;

    let method = methods.get(&method_id).unwrap();

    if conf.log_method_name {
        tracing::info!(
            "! Checking {} :: {}{}",
            class_names.tpath(class_id),
            class_file
                .get_text_t(method.name_index())
                .unwrap_or_else(|| std::borrow::Cow::Owned("[BadMethodNameIndex]".to_owned())),
            method.descriptor().as_pretty_string(class_names),
        );
    }

    if conf.log_method_name {
        tracing::info!("\tLocals: #{}", method_code.max_locals());
    }

    let mut stack_frames =
        StackMapFramesProcessor::new(class_names, class_file, method, method_code)
            .map_err(VerifyStackMapGeneralError::StackMapError)?;

    // TODO: Verify max stack size from code
    // TODO: Verify max stack size from state
    // TODO: If we are verifying max stack size usage above, then it would be nice if the stack map
    // frame parsing let us do some checks for each iteration of it, so that we could produce
    // an error without parsing everything.

    // We don't bother doing the somewhat odd merging of stack map and code that the JVM
    // documentation does, since it seems pointless.

    // Note: This checking is theoretically not the best type checking that we could do with the
    // information extractable from stack maps and instruction behavior, but it is the proper way
    // of doing JVM stack map frame verification, and thus should verify anything that the official
    // JVM verifies.

    // The acting frame, which is used to keep track of what is active, and thus do the checking
    // if an instruction requries an int at the top of the stack and it isn't there, then that's
    // an error
    let mut act_frame = Frame {
        stack: SmallVec::new(),
        locals: Locals::default(),
        at: InstructionIndex(0),
    };

    // The types that have been resolved for a single instruction
    let mut inst_types = InstTypes::new();

    // Iterate over all instructions, performing type checking of each instruction with the given
    // stack frame.
    // Transformations of the type sthat the instructions have is done, because they encode more
    // information than the main code uses.
    for (idx, inst) in method_code.instructions().iter() {
        struct Data<'cn, 'cf, 'c, 'p, 'cfd, 'af, 'it> {
            class_names: &'cn mut ClassNames,
            class_files: &'cf mut ClassFiles,
            classes: &'c mut Classes,
            packages: &'p mut Packages,
            class_file: &'cfd ClassFileInfo,
            conf: StackMapVerificationLogging,
            method_id: ExactMethodId,
            act_frame: &'af mut Frame,
            inst_types: &'it mut InstTypes,
        }
        impl<'cn, 'cf, 'c, 'p, 'cfd, 'af, 'it> InstMapFunc<'_> for Data<'cn, 'cf, 'c, 'p, 'cfd, 'af, 'it> {
            type Output = Result<(), VerifyStackMapGeneralError>;

            fn call(self, inst: &impl Instruction) -> Self::Output {
                check_instruction(
                    self.class_names,
                    self.class_files,
                    self.classes,
                    self.packages,
                    self.class_file,
                    self.conf,
                    self.method_id,
                    self.act_frame,
                    self.inst_types,
                    inst,
                )
            }
        }

        if conf.log_instruction {
            tracing::info!(
                "# ({}) {}",
                idx.0,
                inst.as_pretty_string(class_names, class_file)
            );
        }

        // Update the current frame if there is an injected one
        check_frame(
            class_names,
            class_file,
            &conf,
            method_code,
            &mut stack_frames,
            &mut act_frame,
            *idx,
            inst.name(),
        )?;

        // Check the instruction
        // This maps the data to the generic version of check_instruction so that Rust
        // can optimize each variant, since many can have statically known sizes and more
        inst.map(Data {
            class_names,
            class_files,
            classes,
            packages,
            class_file,
            conf: conf.clone(),
            method_id,
            act_frame: &mut act_frame,
            inst_types: &mut inst_types,
        })?;
    }

    Ok(())
}

fn check_frame(
    class_names: &mut ClassNames,
    class_file: &ClassFileInfo,
    conf: &StackMapVerificationLogging,
    code: &CodeInfo,
    stack_frames: &mut StackMapFramesProcessor,
    act_frame: &mut Frame,
    idx: InstructionIndex,
    inst_name: &'static str,
) -> Result<(), VerifyStackMapGeneralError> {
    if stack_frames.has_next_frame_at(idx) {
        let frame = stack_frames
            .next_frame(class_names, class_file)
            .map_err(VerifyStackMapGeneralError::StackMapError)?
            .expect("has_next_frame_at was expected to not be incorrect about there being a frame");

        // The frame given by the JVM takes precedence over our frame
        if conf.log_received_frame {
            tracing::info!("\t Received Frame: {:#?}", frame);
        }

        act_frame.stack.clear();
        FrameType::from_stack_map_types(
            class_names,
            class_file,
            code,
            &frame.stack,
            &mut act_frame.stack,
        )?;
        act_frame
            .locals
            .ingest_stack_map_types(class_names, class_file, code, &frame.locals)?;
        if act_frame.locals.len() > usize::from(code.max_locals()) {
            return Err(VerifyStackMapError::ReceivedFrameTooManyLocals {
                inst_name,
                inst_index: idx,
            }
            .into());
        }

        // Fill in the rest of the allowed locals with `None`
        act_frame.locals.resize_to(usize::from(code.max_locals()));
        act_frame.at = frame.at;
    }

    Ok(())
}

fn process_pop_type_early_load(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    pop_type_o: Option<PopType>,
    pop_index: PopIndex,
    inst_name: &'static str,
) -> Result<(), VerifyStackMapGeneralError> {
    let pop_type_o = if let Some(pop_type) = pop_type_o {
        pop_type
    } else {
        // This is a library error, since it means that the instruction violates that there
        // should be types at each index.
        panic!(
            "Expected push type index ({}) for instruction ({}) in method to be valid",
            pop_index, inst_name
        );
    };

    let last_frame_type = if let Some(last_frame_type) = act_frame.stack.iter().rev().nth(pop_index)
    {
        last_frame_type
    } else {
        return Err(VerifyStackMapError::InstExpectedTypeInStack {
            inst_name,
            expected_type: pop_type_o,
        }
        .into());
    };

    let typ = match &pop_type_o {
        PopType::Category1 => {
            if !last_frame_type.is_category_1() {
                return Err(VerifyStackMapError::InstExpectedCategory1GotFrameType {
                    inst_name,
                    got_type: last_frame_type.clone(),
                }
                .into());
            }

            Some(last_frame_type.clone())
        }
        PopType::Category2 => {
            if last_frame_type.is_category_1() {
                return Err(VerifyStackMapError::InstExpectedCategory2GotFrameType {
                    inst_name,
                    got_type: last_frame_type.clone(),
                }
                .into());
            }

            Some(last_frame_type.clone())
        }
        PopType::Type(typ) => FrameType::from_opcode_type_no_with(class_names, typ)?,
        PopType::Complex(complex) => Some(FrameType::from_opcode_pop_complex_type(
            classes,
            class_names,
            class_files,
            packages,
            complex,
            last_frame_type,
            inst_name,
        )?),
    };

    if let Some(typ) = typ {
        debug_assert!(inst_types.pop[pop_index].is_none());
        inst_types.pop[pop_index] = Some(typ);
    }
    // otherwise, it was a type we will process after

    Ok(())
}

fn process_pop_type_with_load(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    pop_type_o: Option<PopType>,
    pop_index: PopIndex,
    inst_name: &'static str,
) -> Result<(), VerifyStackMapGeneralError> {
    let pop_type_o = if let Some(pop_type) = pop_type_o {
        pop_type
    } else {
        // This is a library error, since it means that the instruction violates that there
        // should be types at each index.
        panic!(
            "Expected push type index ({}) for instruction ({}) in method to be valid",
            pop_index, inst_name
        );
    };

    let with_t = if let PopType::Type(Type::With(pop_type_o)) = pop_type_o {
        FrameType::from_opcode_with_type(
            classes,
            class_names,
            class_files,
            packages,
            &pop_type_o,
            inst_types,
            &mut act_frame.locals,
            inst_name,
        )?
    } else {
        return Ok(());
    };

    debug_assert!(inst_types.pop[pop_index].is_none());
    inst_types.pop[pop_index] = Some(with_t);
    Ok(())
}

fn check_pop_types(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    conf: &StackMapVerificationLogging,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    pop_count: usize,
    inst_name: &'static str,
) -> Result<(), VerifyStackMapGeneralError> {
    // Check that there are all the needed types on the stack to be popped
    // This also performs the popping
    for i in 0..pop_count {
        // This should always have been initialized already
        // and so an entry should exist at the index and it should have a value inside it
        let pop_type = inst_types
            .get_pop(i)
            .expect("Expected pop type index to be valid");
        // This uses last now because we are actually modifying the frame's stack
        let last_frame_type = if let Some(last_frame_type) = act_frame.stack.last() {
            last_frame_type
        } else {
            // If this didn't exist, then this would have already been returned by the previous
            // initialization
            return Err(VerifyStackMapError::InstExpectedFrameTypeInStack {
                inst_name,
                expected_type: pop_type.clone(),
            }
            .into());
        };

        // We check if it is represented the same on the stack
        // This is because we keep some information (such as if something is a byte)
        // even though smaller types are expanded to an int on the stack.
        // As well, there are several reference types which are interconvertible
        if pop_type.is_stack_same_of_frame_type(
            classes,
            class_names,
            class_files,
            packages,
            last_frame_type,
        )? {
            if conf.log_stack_modifications {
                tracing::info!(
                    "\t\tPOP {}    -- {:?}",
                    last_frame_type.as_pretty_string(class_names),
                    last_frame_type
                );
            }
            act_frame.stack.pop().expect(
                "There should be a type here since it was being actively used as a reference",
            );
        } else {
            return Err(
                VerifyStackMapError::InstExpectedFrameTypeInStackGotFrameType {
                    inst_name,
                    expected_type: pop_type.clone(),
                    got_type: last_frame_type.clone(),
                }
                .into(),
            );
        }
    }

    Ok(())
}

fn check_locals_in_type(
    class_names: &mut ClassNames,
    conf: &StackMapVerificationLogging,
    act_frame: &mut Frame,
    local_index: LocalVariableIndex,
    local_type: LocalVariableInType,
    inst_name: &'static str,
) -> Result<(), VerifyStackMapGeneralError> {
    let local =
        act_frame
            .locals
            .get(local_index)
            .ok_or(VerifyStackMapError::BadLocalVariableIndex {
                inst_name,
                index: local_index,
            })?;

    let is_matching_type = match (local, &local_type) {
        (
            Local::FrameType(FrameType::Primitive(l_prim)),
            LocalVariableInType::Primitive(r_prim),
        ) => l_prim.is_same_type_on_stack(r_prim),
        (Local::FrameType(FrameType::Complex(_)), LocalVariableInType::ReferenceAny) => true,
        _ => false,
    };

    if !is_matching_type {
        return Err(VerifyStackMapError::ExpectedLocalVariableType {
            inst_name,
            expected_type: local_type,
            got_type: local.clone(),
        }
        .into());
    }

    if conf.log_local_variable_modifications {
        tracing::info!(
            "\t\tLLOAD [{}] = {}    -- {:?}",
            local_index,
            local.as_pretty_string(class_names),
            local
        );
    }

    Ok(())
}

fn check_locals_out_type(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    conf: &StackMapVerificationLogging,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    local_index: LocalVariableIndex,
    local_type: LocalVariableType,
    inst_name: &'static str,
) -> Result<(), VerifyStackMapGeneralError> {
    if usize::from(local_index) >= act_frame.locals.len() {
        return Err(VerifyStackMapError::BadLocalVariableIndex {
            inst_name,
            index: local_index,
        }
        .into());
    }

    let local_type = FrameType::from_opcode_local_out_type(
        classes,
        class_names,
        class_files,
        packages,
        &local_type,
        inst_types,
        &mut act_frame.locals,
        inst_name,
    )?;
    if conf.log_local_variable_modifications {
        tracing::info!(
            "\t\tLSTORE [{}] = {}    -- {:?}",
            local_index,
            local_type.as_pretty_string(class_names),
            local_type
        );
    }

    act_frame
        .locals
        .set(inst_name, local_index, Local::FrameType(local_type))?;
    Ok(())
}

fn check_push_type(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    conf: &StackMapVerificationLogging,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    push_index: usize,
    push_type: Option<PushType>,
    inst_name: &'static str,
) -> Result<(), VerifyStackMapGeneralError> {
    let push_type = if let Some(push_type) = push_type {
        push_type
    } else {
        // This is a library error, since it means that the instruction violates that there
        // should be types at each index.
        panic!(
            "Expected push type index ({}) for instruction ({}) in method to be valid",
            push_index, inst_name
        );
    };

    let push_type = FrameType::from_opcode_push_type(
        classes,
        class_names,
        class_files,
        packages,
        &push_type,
        inst_types,
        &mut act_frame.locals,
        inst_name,
    )?;

    if conf.log_stack_modifications {
        tracing::info!(
            "\t\tPUSH {}    -- {:?}",
            push_type.as_pretty_string(class_names),
            push_type
        );
    }
    act_frame.stack.push(push_type);

    Ok(())
}

fn check_instruction<T: Instruction>(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    class_file: &ClassFileInfo,
    conf: StackMapVerificationLogging,
    method_id: ExactMethodId,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    inst: &T,
) -> Result<(), VerifyStackMapGeneralError> {
    inst_types.clear();

    let inst_name = inst.name();

    let stack_sizes: StackSizes = act_frame.stack_sizes();

    let stack_info = inst.stack_info(class_names, class_file, method_id, stack_sizes)?;
    let pop_count = stack_info.pop_count();
    let push_count = stack_info.push_count();

    inst_types.pop.resize(pop_count, None);

    if pop_count != 0 {
        // Initialize all simple pop types that do not depend on other pop types
        for i in 0..pop_count {
            // TODO: Make pop_types iterator? That would avoid these checks, and would probably wor
            // Get the pop type, which should exist because of the requirements on pop_type_at
            let pop_type_o = stack_info.pop_type_at(i);

            process_pop_type_early_load(
                class_names,
                class_files,
                classes,
                packages,
                act_frame,
                inst_types,
                pop_type_o,
                i,
                inst_name,
            )?;
        }

        // Initialize pop types that depend on other pop types
        // we have to do these two stages separately because in the jvm,
        // it is common for the fields that need another field to be put before
        // and so we cannot simply evaluate them in order
        for i in 0..pop_count {
            let pop_type_o = stack_info.pop_type_at(i);

            process_pop_type_with_load(
                class_names,
                class_files,
                classes,
                packages,
                act_frame,
                inst_types,
                pop_type_o,
                i,
                inst_name,
            )?;
        }

        check_pop_types(
            class_names,
            class_files,
            classes,
            packages,
            &conf,
            act_frame,
            inst_types,
            pop_count,
            inst_name,
        )?;
    }

    for (local_index, local_type) in stack_info.locals_in_type_iter() {
        check_locals_in_type(
            class_names,
            &conf,
            act_frame,
            local_index,
            local_type,
            inst_name,
        )?;
    }

    for (local_index, local_type) in stack_info.locals_out_type_iter() {
        check_locals_out_type(
            class_names,
            class_files,
            classes,
            packages,
            &conf,
            act_frame,
            inst_types,
            local_index,
            local_type,
            inst_name,
        )?;
    }

    for i in 0..push_count {
        // TODO: make push_types an iterator?
        let push_type = stack_info.push_type_at(i);

        check_push_type(
            class_names,
            class_files,
            classes,
            packages,
            &conf,
            act_frame,
            inst_types,
            i,
            push_type,
            inst_name,
        )?;
    }

    Ok(())
}
