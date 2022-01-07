use classfile_parser::constant_info::ConstantInfo;
use classfile_parser::descriptor::DescriptorType as DescriptorTypeCF;
use classfile_parser::{
    attribute_info::InstructionIndex,
    constant_info::{ClassConstant, FieldRefConstant, NameAndTypeConstant, Utf8Constant},
    constant_pool::ConstantPoolIndexRaw,
};
use rhojvm_base::code::op::InstMapFunc;
use rhojvm_base::code::stack_map::{StackMapError, StackMapFrames};
use rhojvm_base::code::types::{
    Category, Instruction, LocalVariableInType, LocalVariableIndex, LocalVariableType, LocalsIn,
    LocalsOutAt, PopComplexType, StackSizes,
};
use rhojvm_base::id::MethodIndex;
use rhojvm_base::Methods;
use rhojvm_base::{
    class::ClassFileData,
    code::{
        method::{DescriptorType, DescriptorTypeBasic},
        op::Inst,
        stack_map::StackMapType,
        types::{
            ComplexType, HasStackInfo, PopIndex, PopType, PopTypeAt, PrimitiveType, PushType,
            PushTypeAt, Type, WithType,
        },
        CodeInfo,
    },
    id::{ClassId, MethodId},
    package::Packages,
    ClassDirectories, ClassFiles, ClassNames, Classes, StepError,
};
use smallvec::SmallVec;

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
#[derive(Debug, Clone)]
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
impl Default for StackMapVerificationLogging {
    fn default() -> Self {
        Self {
            log_method_name: false,
            log_received_frame: false,
            log_instruction: false,
            log_stack_modifications: false,
            log_local_variable_modifications: false,
        }
    }
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
impl From<PrimitiveType> for Local {
    fn from(p: PrimitiveType) -> Local {
        Local::FrameType(p.into())
    }
}
impl From<ComplexFrameType> for Local {
    fn from(c: ComplexFrameType) -> Local {
        Local::FrameType(c.into())
    }
}
impl From<FrameType> for Local {
    fn from(x: FrameType) -> Local {
        Local::FrameType(x)
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
    fn push(&mut self, v: impl Into<Local>) {
        self.locals.push(v.into())
    }

    fn len(&self) -> usize {
        self.locals.len()
    }

    fn resize_to(&mut self, len: usize) {
        self.locals.resize(len, Local::Unfilled);
    }

    fn from_stack_map_types(
        &mut self,
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
        code: &CodeInfo,
        types: &[StackMapType],
    ) -> Result<(), VerifyStackMapGeneralError> {
        self.locals.clear();

        let mut types_iter = types.iter().peekable();
        while let Some(typ) = types_iter.next() {
            match typ {
                StackMapType::Integer => self.push(PrimitiveType::Int),
                StackMapType::Float => self.push(PrimitiveType::Float),
                StackMapType::Long => {
                    // Skip the top type
                    if !matches!(types_iter.next(), Some(StackMapType::Top)) {
                        return Err(VerifyStackMapError::LocalCategory2HadNoTop.into());
                    }
                    self.push(PrimitiveType::Long);
                    self.push(Local::Top);
                }
                StackMapType::Double => {
                    // Skip the top type
                    if !matches!(types_iter.next(), Some(StackMapType::Top)) {
                        return Err(VerifyStackMapError::LocalCategory2HadNoTop.into());
                    }
                    self.push(PrimitiveType::Double);
                    self.push(Local::Top);
                }
                StackMapType::UninitializedThis(id) => {
                    self.push(ComplexFrameType::UninitializedReferenceClass(*id))
                }
                StackMapType::UninitializedVariable(idx) => {
                    // TODO(recover-faulty-stack-map): We could theoretically
                    // lossily recover from this
                    let inst = code
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
                    let class_name = class_file.get_text_t(class.name_index).ok_or(
                        VerifyStackMapError::BadNewClassNameIndex {
                            index: class.name_index,
                        },
                    )?;
                    let class_id = class_names.gcid_from_str(class_name);

                    self.push(ComplexFrameType::UninitializedReferenceClass(class_id))
                }
                StackMapType::Object(id) => self.push(ComplexFrameType::ReferenceClass(*id)),
                StackMapType::Null => self.push(ComplexFrameType::ReferenceNull),
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
                    }
                    .into())
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
pub fn verify_type_safe_method_stack_map(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    conf: StackMapVerificationLogging,
    class_file: &ClassFileData,
    method_index: MethodIndex,
) -> Result<(), VerifyStackMapGeneralError> {
    let _span = tracing::span!(tracing::Level::TRACE, "stackmap verification").entered();

    let class_id = class_file.id();
    let method_id = MethodId::unchecked_compose(class_id, method_index);

    methods.load_method_from_index(class_names, class_file, method_index)?;
    let method = methods.get_mut(&method_id).unwrap();
    method.load_code(class_files)?;

    let method = methods.get(&method_id).unwrap();

    if conf.log_method_name {
        tracing::info!(
            "! Checking {} :: {}{}",
            class_names
                .display_path_from_gcid(class_id)
                .unwrap_or_else(|_| "[BadIdError]".to_owned()),
            method.name(),
            method.descriptor().as_pretty_string(class_names),
        );
    }

    let code = if let Some(code) = method.code() {
        code
    } else {
        // We tried loading the code but there wasn't any.
        // Thus, there is no stack map to validate
        return Ok(());
    };

    if conf.log_method_name {
        tracing::info!("\tLocals: #{}", code.max_locals());
    }

    let stack_frames = if let Some(stack_frames) =
        StackMapFrames::parse_frames(class_names, class_file, method, code)
            .map_err(VerifyStackMapGeneralError::StackMapError)?
    {
        stack_frames
    } else {
        // If there were no stack frames then there is no need to verify them
        // This is because the types can be inferred easily, such as in a function
        // without control flow
        // FIXME: For methods without stack frames, we still need to type check them!
        return Ok(());
    };

    // Assert that there is a first entry that starts at the very start of the method
    debug_assert_eq!(
        stack_frames.iter().nth(0).map(|x| x.at),
        Some(InstructionIndex(0))
    );

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

    // Clone the code instance, because we need to load classes, and Rust isn't smart enough
    // to let us borrow the inInstExpectedTypeInStackGotstructions, which being heap allocated, would not move
    // TODO: Don't clone? We could presumably reget the class file, method, and then code
    // and continue at some indice. As well, to make that less extreme, constant size chunks
    // could be copied out of the instructions to process before needing to reget the code
    // Either way would avoid an alloc
    let code = code.clone();

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
    for (idx, inst) in code.instructions() {
        struct Data<'cd, 'cn, 'cf, 'c, 'p, 'cfd, 'ci, 'sf, 'af, 'it, 'oi> {
            class_directories: &'cd ClassDirectories,
            class_names: &'cn mut ClassNames,
            class_files: &'cf mut ClassFiles,
            classes: &'c mut Classes,
            packages: &'p mut Packages,
            class_file: &'cfd ClassFileData,
            conf: StackMapVerificationLogging,
            method_id: MethodId,
            code: &'ci CodeInfo,
            stack_frames: &'sf StackMapFrames,
            act_frame: &'af mut Frame,
            inst_types: &'it mut InstTypes,
            idx: InstructionIndex,
            outer_inst: &'oi Inst,
        }
        impl<'cd, 'cn, 'cf, 'c, 'p, 'cfd, 'ci, 'sf, 'af, 'it, 'oi> InstMapFunc<'oi>
            for Data<'cd, 'cn, 'cf, 'c, 'p, 'cfd, 'ci, 'sf, 'af, 'it, 'oi>
        {
            type Output = Result<(), VerifyStackMapGeneralError>;

            fn call(self, inst: &impl Instruction) -> Self::Output {
                check_instruction(
                    self.class_directories,
                    self.class_names,
                    self.class_files,
                    self.classes,
                    self.packages,
                    self.class_file,
                    self.conf,
                    self.method_id,
                    self.code,
                    self.stack_frames,
                    self.act_frame,
                    self.inst_types,
                    self.idx,
                    self.outer_inst,
                    inst,
                )
            }
        }

        inst.map(Data {
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            class_file,
            conf: conf.clone(),
            method_id,
            code: &code,
            stack_frames: &stack_frames,
            act_frame: &mut act_frame,
            inst_types: &mut inst_types,
            idx: *idx,
            outer_inst: inst,
        })?;
    }

    Ok(())
}

fn check_frame(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
    conf: &StackMapVerificationLogging,
    code: &CodeInfo,
    stack_frames: &StackMapFrames,
    act_frame: &mut Frame,
    idx: InstructionIndex,
    inst_name: &'static str,
) -> Result<(), VerifyStackMapGeneralError> {
    if let Some(frame) = stack_frames.iter().find(|x| x.at == idx) {
        // The frame given by the JVM takes precedence over our frame

        if conf.log_received_frame {
            tracing::info!("\t Received Frame: {:#?}", frame);
        }

        act_frame.stack.clear();
        FrameType::from_stack_map_types(
            class_names,
            class_file,
            &code,
            &frame.stack,
            &mut act_frame.stack,
        )?;
        act_frame
            .locals
            .from_stack_map_types(class_names, class_file, &code, &frame.locals)?;
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
    class_directories: &ClassDirectories,
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
            inst_name: inst_name,
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
            class_directories,
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
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    pop_type_o: Option<PopType>,
    pop_index: PopIndex,
    class_id: ClassId,
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
            class_directories,
            class_names,
            class_files,
            packages,
            &pop_type_o,
            &inst_types,
            &mut act_frame.locals,
            inst_name,
            class_id,
        )?
    } else {
        return Ok(());
    };

    debug_assert!(inst_types.pop[pop_index].is_none());
    inst_types.pop[pop_index] = Some(with_t);
    Ok(())
}

fn check_pop_types(
    class_directories: &ClassDirectories,
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
                inst_name: inst_name,
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
            class_directories,
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
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    conf: &StackMapVerificationLogging,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    class_id: ClassId,
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
        class_directories,
        class_names,
        class_files,
        packages,
        &local_type,
        &inst_types,
        &mut act_frame.locals,
        inst_name,
        class_id,
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
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    conf: &StackMapVerificationLogging,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    class_id: ClassId,
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
        class_directories,
        class_names,
        class_files,
        packages,
        &push_type,
        &inst_types,
        &mut act_frame.locals,
        inst_name,
        class_id,
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

fn check_instruction<'inst, T: Instruction>(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    class_file: &ClassFileData,
    conf: StackMapVerificationLogging,
    method_id: MethodId,
    code: &CodeInfo,
    stack_frames: &StackMapFrames,
    act_frame: &mut Frame,
    inst_types: &mut InstTypes,
    idx: InstructionIndex,
    outer_inst: &'inst Inst,
    inst: &'inst T,
) -> Result<(), VerifyStackMapGeneralError> {
    inst_types.clear();

    let class_id = class_file.id();

    let inst_name = inst.name();
    if conf.log_instruction {
        tracing::info!(
            "# ({}) {}",
            idx.0,
            outer_inst.as_pretty_string(class_names, class_file)
        );
    }

    check_frame(
        class_names,
        class_file,
        &conf,
        code,
        stack_frames,
        act_frame,
        idx,
        inst_name,
    )?;

    let stack_sizes: StackSizes = act_frame.stack_sizes();

    let stack_info = inst.stack_info(class_names, class_file, method_id, stack_sizes)?;
    let pop_count = stack_info.pop_count();
    let push_count = stack_info.push_count();

    inst_types.pop.resize(pop_count, None);

    // Initialize all simple pop types that do not depend on other pop types
    for i in 0..pop_count {
        // TODO: Make pop_types iterator? That would avoid these checks, and would probably wor
        // Get the pop type, which should exist because of the requirements on pop_type_at
        let pop_type_o = stack_info.pop_type_at(i);

        process_pop_type_early_load(
            class_directories,
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
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            act_frame,
            inst_types,
            pop_type_o,
            i,
            class_id,
            inst_name,
        )?;
    }

    check_pop_types(
        class_directories,
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
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            &conf,
            act_frame,
            inst_types,
            class_id,
            local_index,
            local_type,
            inst_name,
        )?;
    }

    for i in 0..push_count {
        // TODO: make push_types an iterator?
        let push_type = stack_info.push_type_at(i);

        check_push_type(
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            &conf,
            act_frame,
            inst_types,
            class_id,
            i,
            push_type,
            inst_name,
        )?;
    }

    Ok(())
}

struct InstTypes {
    pop: SmallVec<[Option<FrameType>; 6]>,
}
impl InstTypes {
    fn new() -> InstTypes {
        InstTypes {
            pop: SmallVec::new(),
        }
    }

    fn clear(&mut self) {
        self.pop.clear();
    }

    fn get_pop(&self, index: usize) -> Option<&FrameType> {
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
    fn is_category_1(&self) -> bool {
        match self {
            FrameType::Primitive(prim) => match prim {
                PrimitiveType::Long | PrimitiveType::Double => false,
                // All other primitives are category 1
                _ => true,
            },
            FrameType::Complex(_) => true,
        }
    }

    /// Is the type on the right convertible into the type on the left on a stack
    fn is_stack_same_of_frame_type(
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
                // TODO: casting to base class
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
                (ComplexFrameType::ReferenceClass(_), ComplexFrameType::ReferenceNull)
                | (ComplexFrameType::ReferenceNull, ComplexFrameType::ReferenceClass(_))
                | (
                    ComplexFrameType::UninitializedReferenceClass(_),
                    ComplexFrameType::ReferenceNull,
                )
                | (
                    ComplexFrameType::ReferenceNull,
                    ComplexFrameType::UninitializedReferenceClass(_),
                )
                | (ComplexFrameType::ReferenceNull, ComplexFrameType::ReferenceNull) => true,
            },
            (FrameType::Primitive(_), FrameType::Complex(_))
            | (FrameType::Complex(_), FrameType::Primitive(_)) => false,
        })
    }

    fn from_stack_map_types<const N: usize>(
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
        code: &CodeInfo,
        types: &[StackMapType],
        result: &mut SmallVec<[FrameType; N]>,
    ) -> Result<(), VerifyStackMapGeneralError> {
        let mut types_iter = types.iter();
        while let Some(typ) = types_iter.next() {
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
                    let class_name = class_file.get_text_t(class.name_index).ok_or(
                        VerifyStackMapError::BadNewClassNameIndex {
                            index: class.name_index,
                        },
                    )?;
                    let class_id = class_names.gcid_from_str(class_name);

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

    fn from_opcode_primitive_type(primitive: &PrimitiveType) -> FrameType {
        FrameType::Primitive(primitive.clone())
    }

    fn from_opcode_complex_type(
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

    fn from_opcode_with_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        with_t: &WithType,
        inst_types: &InstTypes,
        locals: &mut Locals,
        inst_name: &'static str,
        class_id: ClassId,
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
            WithType::ReferenceIndex(index) => {
                let class_file = class_files.get(&class_id).unwrap();
                let elem_class = class_file
                    .get_t(index)
                    .ok_or(VerifyStackMapError::BadClassIndex(*index))?;
                let elem_name = class_file.get_text_t(elem_class.name_index).ok_or(
                    VerifyStackMapError::BadClassNameIndex(elem_class.name_index),
                )?;
                let elem_id = class_names.gcid_from_str(elem_name);

                ComplexFrameType::ReferenceClass(elem_id).into()
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
                        ComplexFrameType::UninitializedReferenceClass(_) => {
                            return Err(VerifyStackMapError::RefArrayRefTypeUncertainType.into())
                        }
                        ComplexFrameType::ReferenceNull => {
                            return Err(VerifyStackMapError::RefArrayRefTypeUncertainType.into())
                        }
                    },
                }
            }
            WithType::RefArrayRefFromIndexLen { index, .. } => {
                let class_file = class_files.get(&class_id).unwrap();
                let elem_class = class_file
                    .get_t(index)
                    .ok_or(VerifyStackMapError::BadClassIndex(*index))?;
                let elem_name = class_file.get_text_t(elem_class.name_index).ok_or(
                    VerifyStackMapError::BadClassNameIndex(elem_class.name_index),
                )?;
                let elem_id = class_names.gcid_from_str(elem_name);

                let array_id = classes.load_array_of_instances(
                    class_directories,
                    class_names,
                    class_files,
                    packages,
                    elem_id,
                )?;

                ComplexFrameType::ReferenceClass(array_id).into()
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
                let id = class_names.gcid_from_slice(class_name);
                ComplexFrameType::ReferenceClass(id).into()
            }
            WithType::Category1Constant { index } => {
                let class_file = class_files.get(&class_id).unwrap();
                let value = class_file
                    .get_t(index)
                    .ok_or(VerifyStackMapError::BadConstantIndex(*index))?;
                match value {
                    ConstantInfo::Integer(_) => PrimitiveType::Int.into(),
                    ConstantInfo::Float(_) => PrimitiveType::Float.into(),
                    ConstantInfo::Class(_class) => ComplexFrameType::ReferenceClass(
                        class_names.gcid_from_slice(&["java", "lang", "Class"]),
                    )
                    .into(),
                    ConstantInfo::String(_) => {
                        let string_id = class_names.gcid_from_slice(&["java", "lang", "String"]);
                        ComplexFrameType::ReferenceClass(string_id).into()
                    }
                    ConstantInfo::MethodHandle(_) => {
                        ComplexFrameType::ReferenceClass(class_names.gcid_from_slice(&[
                            "java",
                            "lang",
                            "invoke",
                            "MethodHandle",
                        ]))
                        .into()
                    }
                    ConstantInfo::MethodType(_) => {
                        ComplexFrameType::ReferenceClass(class_names.gcid_from_slice(&[
                            "java",
                            "lang",
                            "invoke",
                            "MethodType",
                        ]))
                        .into()
                    }
                    _ => return Err(VerifyStackMapError::BadConstantType.into()),
                }
            }
            WithType::Category2Constant { index } => {
                let class_file = class_files.get(&class_id).unwrap();
                let value = class_file
                    .get_t(index)
                    .ok_or(VerifyStackMapError::BadConstantIndex(*index))?;
                match value {
                    ConstantInfo::Long(_) => PrimitiveType::Long.into(),
                    ConstantInfo::Double(_) => PrimitiveType::Double.into(),
                    _ => return Err(VerifyStackMapError::BadConstantType.into()),
                }
            }
            WithType::FieldType { index } => {
                let class_file = class_files.get(&class_id).unwrap();
                let field = class_file
                    .get_t(index)
                    .ok_or(VerifyStackMapError::BadFieldIndex(*index))?;
                let nat_index = field.name_and_type_index;
                let nat = class_file
                    .get_t(nat_index)
                    .ok_or(VerifyStackMapError::BadFieldNatIndex(nat_index))?;

                let field_descriptor = class_file.get_text_t(nat.descriptor_index).ok_or(
                    VerifyStackMapError::BadFieldDescriptorIndex(nat.descriptor_index),
                )?;
                let (field_type, rem) = DescriptorTypeCF::parse(field_descriptor)
                    .map_err(VerifyStackMapError::InvalidFieldDescriptor)?;
                if !rem.is_empty() {
                    return Err(VerifyStackMapError::UnparsedFieldType.into());
                }

                let field_type = DescriptorType::from_class_file_desc(class_names, field_type);
                FrameType::from_descriptor_type(
                    classes,
                    class_directories,
                    class_names,
                    class_files,
                    packages,
                    field_type,
                )?
            }
            WithType::UninitializedObject { index } => {
                let class_file = class_files.get(&class_id).unwrap();
                let class = class_file
                    .get_t(index)
                    .ok_or(VerifyStackMapError::BadClassIndex(*index))?;
                let name = class_file
                    .get_text_t(class.name_index)
                    .ok_or(VerifyStackMapError::BadClassNameIndex(class.name_index))?;
                let id = class_names.gcid_from_str(name);
                ComplexFrameType::UninitializedReferenceClass(id).into()
            }
            WithType::IntArrayIndexInto(_idx) => PrimitiveType::Int.into(),
            WithType::LiteralInt(_val) => PrimitiveType::Int.into(),
        })
    }

    fn from_opcode_pop_complex_type(
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
                            } else {
                                complex.clone().into()
                            }
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
    fn from_opcode_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        typ: &Type,
        inst_types: &InstTypes,
        locals: &mut Locals,
        inst_name: &'static str,
        class_id: ClassId,
    ) -> Result<FrameType, VerifyStackMapGeneralError> {
        match typ {
            Type::Primitive(primitive) => Ok(FrameType::from_opcode_primitive_type(primitive)),
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
                class_id,
            ),
        }
    }

    fn from_opcode_type_no_with(
        class_names: &mut ClassNames,
        typ: &Type,
    ) -> Result<Option<FrameType>, VerifyStackMapGeneralError> {
        match typ {
            Type::Primitive(primitive) => {
                Ok(Some(FrameType::from_opcode_primitive_type(primitive)))
            }
            Type::Complex(complex) => {
                FrameType::from_opcode_complex_type(class_names, complex).map(Some)
            }
            Type::With(_) => Ok(None),
        }
    }

    fn from_opcode_local_out_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        typ: &LocalVariableType,
        inst_types: &InstTypes,
        locals: &mut Locals,
        inst_name: &'static str,
        class_id: ClassId,
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
                class_id,
            ),
        }
    }

    fn from_opcode_push_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        typ: &PushType,
        inst_types: &InstTypes,
        locals: &mut Locals,
        inst_name: &'static str,
        class_id: ClassId,
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
                class_id,
            ),
        }
    }

    fn from_descriptor_type(
        classes: &mut Classes,
        class_directories: &ClassDirectories,
        class_names: &mut ClassNames,
        class_files: &mut ClassFiles,
        packages: &mut Packages,
        typ: DescriptorType,
    ) -> Result<FrameType, StepError> {
        Ok(match typ {
            DescriptorType::Basic(basic) => FrameType::from_basic_descriptor_type(basic),
            DescriptorType::Array { level, component } => {
                let array_id = classes.load_level_array_of_desc_type_basic(
                    class_directories,
                    class_names,
                    class_files,
                    packages,
                    level,
                    component,
                )?;

                ComplexFrameType::ReferenceClass(array_id).into()
            }
        })
    }

    fn from_basic_descriptor_type(typ: DescriptorTypeBasic) -> FrameType {
        match typ {
            DescriptorTypeBasic::Byte => PrimitiveType::Byte.into(),
            DescriptorTypeBasic::Char => PrimitiveType::Char.into(),
            DescriptorTypeBasic::Double => PrimitiveType::Double.into(),
            DescriptorTypeBasic::Float => PrimitiveType::Float.into(),
            DescriptorTypeBasic::Int => PrimitiveType::Int.into(),
            DescriptorTypeBasic::Long => PrimitiveType::Long.into(),
            DescriptorTypeBasic::Class(id) => ComplexFrameType::ReferenceClass(id).into(),
            DescriptorTypeBasic::Short => PrimitiveType::Short.into(),
            DescriptorTypeBasic::Boolean => PrimitiveType::Boolean.into(),
        }
    }

    fn as_pretty_string(&self, class_names: &ClassNames) -> String {
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
                if let Ok(name) = class_names.display_path_from_gcid(*id) {
                    format!("#{}", name)
                } else {
                    format!("#[{}]", *id)
                }
            }
            ComplexFrameType::UninitializedReferenceClass(id) => {
                if let Ok(name) = class_names.display_path_from_gcid(*id) {
                    format!("!#{}", name)
                } else {
                    format!("!#[{}]", *id)
                }
            }
            ComplexFrameType::ReferenceNull => "#null".to_owned(),
        }
    }
}
