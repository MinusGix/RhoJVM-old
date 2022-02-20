// LocalVariableIndex seems to trip up clippy
#![allow(clippy::needless_pass_by_value)]

use classfile_parser::{
    attribute_info::InstructionIndex, constant_info::ConstantInfo,
    constant_pool::ConstantPoolIndexRaw, descriptor::method::MethodDescriptorError,
};
use rhojvm_base::{
    code::{
        op::{Inst, Wide, WideInst},
        types::{Instruction, LocalVariableIndex},
    },
    convert_classfile_text,
    id::{ClassId, MethodId},
    map_inst,
    package::Packages,
    util::MemorySizeU16,
    ClassDirectories, ClassFiles, ClassNames, Classes, Methods,
};

use crate::{
    class_instance::{ClassInstance, Instance},
    gc::GcRef,
    rv::{RuntimeType, RuntimeValue, RuntimeValuePrimitive},
    GeneralError, State,
};

mod control_flow;
mod func;
mod instances;
mod operation;
mod store_load;

// TODO: It would be good to make a transformed instruction enumeration, because there are many
// instructions which could have certain parts of their data computed once and never again
// currently, we do these checks everytime it is ran, which is overly expensive
// (ex: `New` currently looks up in the constant pool for the class it should created
//       it should simply store the class id!, it also checks if the dest class is accessible
//        every single time...)
// It would be really cool to get instructions to a situation where the only errors they can throw
// are due to missing values on the stack/locals (or even then, those shouldn't be possible if
// stackmap verification works properly), too.

#[derive(Debug, Clone)]
pub enum EvalError {
    /// The class that holds the method we are executing isn't loaded
    MissingMethodClass(ClassId),
    /// The class file that holds the method we are executing isn't loaded
    MissingMethodClassFile(ClassId),
    /// It was expected that this method should be loaded
    /// Likely because it was given to the function to evaluate
    MissingMethod(MethodId),
    /// We tried continuing but the instruction at that index was missing, or it was in the middle
    /// of an instruction
    MissingInstruction(InstructionIndex),

    /// The index into the constant pool was invalid, either out of bounds or incorrect type
    /// Should have been caught in stack map verification
    InvalidConstantPoolIndex(ConstantPoolIndexRaw<ConstantInfo>),
    /// A field descriptor was invalid
    /// Should have been caught in stack map verification
    InvalidFieldDescriptor,

    InvalidGcRef(GcRef<Instance>),

    /// Expected a value on the top of the stack (probably for popping)
    ExpectedStackValue,
    /// Expected a value gotten from the stack to be a reference
    ExpectedStackValueReference,
    /// Expected a value that would be represented as an integer
    ExpectedStackValueIntRepr,
    /// Expected a float
    ExpectedStackValueFloat,
    /// Expected a long
    ExpectedStackValueLong,
    /// Expected a double
    ExpectedStackValueDouble,
    /// Expected a value that is category 1
    ExpectedStackValueCategory1,
    /// Expected a value that is category 2
    ExpectedStackValueCategory2,

    /// It was expected that there would be a local variable at the given index
    ExpectedLocalVariable(LocalVariableIndex),
    /// It was expected that the local variable would have a value
    ExpectedLocalVariableWithValue(LocalVariableIndex),
    /// Expected the local variable at the given index to be a reference
    ExpectedLocalVariableReference(LocalVariableIndex),
    /// Expected the local variable at the given index to be representable as an int
    ExpectedLocalVariableIntRepr(LocalVariableIndex),
    /// Expected the local variable at the given index to be a float
    ExpectedLocalVariableFloat(LocalVariableIndex),
    /// Expected the local variable at the given index to be a long
    ExpectedLocalVariableLong(LocalVariableIndex),
    /// Expected the local variable at the given index to be a double
    ExpectedLocalVariableDouble(LocalVariableIndex),

    /// When computing branch target, the result {over,under}flowed
    BranchOverflows,
    /// When getting an instance, we expected it to be [`ClassInstance`] specifically
    ExpectedClassInstance,
    /// When getting an instance, we expected it to be [`ArrayInstance`] specifically
    ExpectedArrayInstance,
    /// When getting an array instance, we expected the component type to be this type
    ExpectedArrayInstanceOf(RuntimeType),
    /// We expected a certain element type for this array but we got a different type
    ExpectedArrayInstanceOfClass {
        element: ClassId,
        got: ClassId,
    },
    /// When throwing an exception, we tried throwing a value which wasn't an instance of Throwable
    /// Should have been caught in stack map verification
    ExpectedThrowable,
    /// The A-Type (argument type) for the NewArray instruction was incorrect
    InvalidNewArrayAType,
    /// The constant info for an invoke static was invalid
    /// This should have been caught in verification
    InvalidInvokeStaticConstantInfo,
    /// We tried to parse the method descriptor but failed
    InvalidMethodDescriptor(MethodDescriptorError),
}

#[derive(Debug, Clone)]
pub enum Local {
    /// The upper part of a Long/Double
    Top,
    /// No value
    Empty,
    Value(RuntimeValue),
}
impl Local {
    #[must_use]
    pub fn as_value(&self) -> Option<&RuntimeValue> {
        match self {
            Local::Value(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_value_mut(&mut self) -> Option<&mut RuntimeValue> {
        match self {
            Local::Value(v) => Some(v),
            _ => None,
        }
    }

    fn from_runtime_value(v: RuntimeValue) -> [Option<Local>; 2] {
        match v {
            RuntimeValue::Primitive(prim) => match prim {
                RuntimeValuePrimitive::I64(_) | RuntimeValuePrimitive::F64(_) => {
                    [Some(Local::Value(v)), Some(Local::Top)]
                }
                _ => [Some(Local::Value(v)), None],
            },
            RuntimeValue::Reference(_) | RuntimeValue::NullReference => {
                [Some(Local::Value(v)), None]
            }
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Locals {
    locals: Vec<Local>,
}
impl Locals {
    #[must_use]
    pub fn new_with_array<const N: usize>(locals: [RuntimeValue; N]) -> Locals {
        let locals = locals
            .into_iter()
            .flat_map(Local::from_runtime_value)
            .flatten()
            .collect();
        Locals { locals }
    }

    /// Push a value onto the locals stack, transforming it into as many instances as it needs.
    /// Because, values like Long/Double take up two indices on the local stack.
    pub fn push_transform(&mut self, value: RuntimeValue) {
        let local = Local::from_runtime_value(value);
        for l in local.into_iter().flatten() {
            self.locals.push(l);
        }
    }

    #[must_use]
    pub fn get(&self, index: LocalVariableIndex) -> Option<&Local> {
        self.locals.get(usize::from(index))
    }

    pub fn get_mut(&mut self, index: LocalVariableIndex) -> Option<&mut Local> {
        self.locals.get_mut(usize::from(index))
    }

    pub fn set_value_at(&mut self, index: LocalVariableIndex, value: RuntimeValue) {
        let index = usize::from(index);
        // If the index is out of bounds then resize the vec to include it
        if index >= self.locals.len() {
            self.locals.resize(index + 1, Local::Empty);
        }

        self.locals[index] = Local::Value(value);
    }
}

#[derive(Default, Debug, Clone)]
pub struct Stack {
    stack: Vec<RuntimeValue>,
}
impl Stack {
    pub fn push(&mut self, value: impl Into<RuntimeValue>) -> Result<(), GeneralError> {
        // TODO: Check if this would exceed the maximum stack size?
        self.stack.push(value.into());
        Ok(())
    }

    pub fn pop(&mut self) -> Option<RuntimeValue> {
        self.stack.pop()
    }

    /// Pop 2 values at once, returning None if either of them don't exist
    pub fn pop2(&mut self) -> Option<(RuntimeValue, RuntimeValue)> {
        let v1 = self.pop();
        if let Some(v1) = v1 {
            self.pop().map(|v2| (v1, v2))
        } else {
            None
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Frame {
    pub stack: Stack,
    /// This uses None as both the 'Top' and as an unfilled value
    pub locals: Locals,
}
impl Frame {
    #[must_use]
    pub fn new_locals(locals: Locals) -> Frame {
        Frame {
            stack: Stack::default(),
            locals,
        }
    }
}

/// Either a value or an exception
#[derive(Debug, Clone, Copy)]
pub enum ValueException<V> {
    Value(V),
    Exception(GcRef<ClassInstance>),
}

pub enum EvalMethodValue {
    /// We returned nothing
    ReturnVoid,
    /// We returned this value
    Return(RuntimeValue),
    /// There was an exception
    Exception(GcRef<ClassInstance>),
}

/// `method_id` should already be loaded
pub fn eval_method(
    class_directories: &ClassDirectories,
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    methods: &mut Methods,
    state: &mut State,
    method_id: MethodId,
    frame: Frame,
) -> Result<EvalMethodValue, GeneralError> {
    let method = methods
        .get_mut(&method_id)
        .ok_or(EvalError::MissingMethod(method_id))?;
    method.load_code(class_files)?;

    {
        let (class_id, _) = method_id.decompose();
        if let Some(class_file) = class_files.get(&class_id) {
            let method_name = class_file.get_text_b(method.name_index()).unwrap();
            let class_name = class_names.tpath(class_id);
            tracing::info!(
                "Executing Method: {}::{}",
                class_name,
                convert_classfile_text(method_name)
            );
        } else {
            tracing::info!("Executing Method (No Backing Class File):");
        }
    }

    // TODO: Handle native methods

    let mut frame = frame;
    let mut pc = InstructionIndex(0);

    loop {
        let method = methods
            .get_mut(&method_id)
            .ok_or(EvalError::MissingMethod(method_id))?;

        let inst = method
            .code()
            .unwrap()
            .instructions()
            .get_instruction_at(pc)
            .ok_or(EvalError::MissingInstruction(pc))?
            .clone();
        let size = inst.memory_size_u16();

        {
            let (class_id, _) = method_id.decompose();
            if let Some(class_file) = class_files.get(&class_id) {
                tracing::info!(
                    "# ({}) {}",
                    pc.0,
                    inst.as_pretty_string(class_names, class_file)
                );
            } else {
                tracing::info!("# ({}) {:?}", pc.0, inst);
            }
        }

        let args = RunInstArgs {
            class_directories,
            class_names,
            class_files,
            classes,
            packages,
            methods,
            state,
            method_id,
            frame: &mut frame,
            inst_index: pc,
        };

        // TODO: Some of the returned errors should maybe be exceptions
        let res = map_inst!(inst; x; x.run(args))?;

        match res {
            // TODO: Should we throw an exception if we return a value of the wrong type?
            RunInstValue::Continue => pc.0 += size,
            RunInstValue::ContinueAt(i) => pc = i,
            RunInstValue::ReturnVoid => return Ok(EvalMethodValue::ReturnVoid),
            RunInstValue::Return(x) => return Ok(EvalMethodValue::Return(x)),
            RunInstValue::Exception(exc) => return Ok(EvalMethodValue::Exception(exc)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RunInstValue {
    /// We returned nothing
    ReturnVoid,
    /// We returned with a value
    Return(RuntimeValue),
    // TODO: Give this a value
    /// There was an exception
    Exception(GcRef<ClassInstance>),
    /// Continue executing to the next instruction
    Continue,
    /// Continue executing at a specific instruction
    /// (such as, due to a goto)
    ContinueAt(InstructionIndex),
}

pub struct RunInstArgs<'cd, 'cn, 'cf, 'c, 'p, 'm, 's, 'f> {
    pub class_directories: &'cd ClassDirectories,
    pub class_names: &'cn mut ClassNames,
    pub class_files: &'cf mut ClassFiles,
    pub classes: &'c mut Classes,
    pub packages: &'p mut Packages,
    pub methods: &'m mut Methods,
    pub state: &'s mut State,
    pub method_id: MethodId,
    pub frame: &'f mut Frame,
    /// Index into 'bytes' of instructions, which is more commonly used in code
    pub inst_index: InstructionIndex,
}
pub trait RunInst: Instruction {
    fn run(self, args: RunInstArgs) -> Result<RunInstValue, GeneralError>;
}

impl RunInst for Wide {
    fn run(self, args: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        // TODO: The passed in inst index is wrong, but that doesn't matter for the current set of
        // instructions
        match self.0 {
            WideInst::WideIntLoad(x) => x.run(args),
            WideInst::WideIntIncrement(x) => x.run(args),
        }
    }
}
