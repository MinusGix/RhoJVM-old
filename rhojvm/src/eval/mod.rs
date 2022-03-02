// LocalVariableIndex seems to trip up clippy
#![allow(clippy::needless_pass_by_value)]

use classfile_parser::{
    attribute_info::InstructionIndex, constant_info::ConstantInfo,
    constant_pool::ConstantPoolIndexRaw, descriptor::method::MethodDescriptorError,
    method_info::MethodAccessFlags,
};
use rhojvm_base::{
    code::{
        method::Method,
        op::{Inst, Wide, WideInst},
        types::{Instruction, LocalVariableIndex},
    },
    convert_classfile_text,
    id::{ClassId, MethodId},
    map_inst,
    util::MemorySizeU16,
    StepError,
};

use crate::{
    class_instance::{ClassInstance, Instance, ReferenceInstance},
    gc::GcRef,
    jni::{self, JObject, JValue},
    method::NativeMethod,
    rv::{RuntimeType, RuntimeValue, RuntimeValuePrimitive},
    util::Env,
    GeneralError, State,
};

mod control_flow;
mod func;
pub mod instances;
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
    /// Expected there to be a static class reference for the given class
    MissingStaticClassRef(ClassId),

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

    pub fn prepush_transform(&mut self, value: RuntimeValue) {
        let local = Local::from_runtime_value(value);
        for l in local.into_iter().rev().flatten() {
            self.locals.insert(0, l);
        }
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

// FIXME: Class ref should probably give the function the Class<T> instance?
/// Helper function to get the class ref value for static/nonstatic methods
/// For native functions, which take a pointer to a gcref
fn get_class_ref(
    state: &mut State,
    frame: &mut Frame,
    class_id: ClassId,
    method: &Method,
) -> Result<ValueException<GcRef<Instance>>, GeneralError> {
    Ok(
        if method.access_flags().contains(MethodAccessFlags::STATIC) {
            ValueException::Value(
                state
                    .find_static_class_instance(class_id)
                    .ok_or(EvalError::MissingStaticClassRef(class_id))?,
            )
        } else {
            // The first local parameter is the this ptr in non-static methods
            let refer = frame
                .locals
                .get(0)
                .ok_or(EvalError::ExpectedLocalVariable(0))?
                .as_value()
                .ok_or(EvalError::ExpectedLocalVariableWithValue(0))?
                .into_reference()
                .ok_or(EvalError::ExpectedLocalVariableReference(0))?;
            if let Some(refer) = refer {
                ValueException::Value(refer.into_generic())
            } else {
                todo!("Null pointer exception?")
            }
        },
    )
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

// TODO: Should this be unsafe? It could trigger unsafety, since it may require us to load
// arbitrary native functions and run them, based on safe input from java!
// It still feels like a wrong level to make it unsafe, though.

/// `method_id` should already be loaded
pub fn eval_method(
    env: &mut Env,
    method_id: MethodId,
    mut frame: Frame,
) -> Result<EvalMethodValue, GeneralError> {
    let (class_id, _) = method_id.decompose();

    let method = env
        .methods
        .get_mut(&method_id)
        .ok_or(EvalError::MissingMethod(method_id))?;
    method.load_code(&mut env.class_files)?;

    {
        if let Some(class_file) = env.class_files.get(&class_id) {
            let method_name = class_file.get_text_b(method.name_index()).unwrap();
            let class_name = env.class_names.tpath(class_id);
            tracing::info!(
                "Executing Method: {}::{}",
                class_name,
                convert_classfile_text(method_name)
            );
        } else {
            tracing::info!("Executing Method (No Backing Class File):");
        }
    }

    // TODO: Move to separate function to make it easier to reason about and maintain safety
    if method.access_flags().contains(MethodAccessFlags::NATIVE) {
        tracing::info!("\tNative Method");
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(GeneralError::MissingLoadedClassFile(class_id))?;
        let (class_name, _) = env
            .class_names
            .name_from_gcid(class_id)
            .map_err(StepError::BadId)?;

        // Get the native function if it exists
        // If it does not exist then we find it given the name of the native function
        let native_func = if let Some(native_func) = env
            .state
            .method_info
            .get(method_id)
            .and_then(|x| x.native_func.clone())
        {
            native_func
        } else {
            let method_name = class_file.get_text_b(method.name_index()).ok_or(
                GeneralError::BadClassFileIndex(method.name_index().into_generic()),
            )?;

            let name = jni::name::make_native_method_name(class_name.get(), method_name);

            let native_func = unsafe {
                env.state
                    .native
                    .find_symbol_blocking_jni_opaque_method(&name)
            };
            let native_func = match native_func {
                Ok(native_func) => native_func,
                Err(err) => {
                    tracing::error!(
                        "Failed to find native function({:?}): {}",
                        err,
                        convert_classfile_text(&name)
                    );
                    return Err(err.into());
                }
            };
            let native_func = NativeMethod::OpaqueFound(native_func);

            env.state.method_info.modify_init_with(method_id, |data| {
                data.native_func = Some(native_func.clone());
            });

            native_func
        };
        let native_func = native_func.get().clone();

        let is_static = method.access_flags().contains(MethodAccessFlags::STATIC);
        let return_type = method.descriptor().return_type().copied();

        if method.descriptor().is_nullary_void() {
            let class_ref = match get_class_ref(&mut env.state, &mut frame, class_id, method)? {
                ValueException::Value(class_ref) => class_ref,
                ValueException::Exception(exc) => return Ok(EvalMethodValue::Exception(exc)),
            };
            let class_ref_jobject: JObject = unsafe { env.get_local_jobject_for(class_ref) };

            // Safety: We rely on the declared parameter types of java being correct
            // In this case, we know that it is a nullary void function which means that it takes
            // in a `*mut JNIEnv`, and `JObject` and returns nothing.
            // However, the safety of this call depends entirely on the safety of the function
            // itself.
            // The native code can store this pointer for use later, but the JVM spec only allows
            // it to be valid on the same thread. Since they can't use it from a different thread,
            // I believe we can rely on that they can't reasonably use it unless we call into them.
            // The pointer.
            // TODO: Is it valid for there to maybe be live mutable pointers to a mutable
            // reference, if they aren't used? I'm not sure how we'd get around that if it
            // isn't... the code we call can simply hold a `*mut Env`, even if it can't modify it
            // until we call into it, which will always be giving it the `*mut Env`
            let env_ptr = env as *mut Env<'_>;
            let native_func = native_func.get();
            unsafe {
                (native_func)(env_ptr, class_ref_jobject);
            };
            return Ok(EvalMethodValue::ReturnVoid);
        }

        if method.descriptor().parameters().len() == 1
            && method.descriptor().parameters()[0].is_reference()
        {
            // fn(JNIEnv*, JObject, JObject) -> ?

            let class_ref = match get_class_ref(&mut env.state, &mut frame, class_id, method)? {
                ValueException::Value(class_ref) => class_ref,
                ValueException::Exception(exc) => return Ok(EvalMethodValue::Exception(exc)),
            };

            let param_index = if is_static { 0 } else { 1 };
            let param: Option<GcRef<Instance>> = frame
                .locals
                .get(param_index)
                .ok_or(EvalError::ExpectedLocalVariable(param_index))?
                .as_value()
                .ok_or(EvalError::ExpectedLocalVariableWithValue(param_index))?
                .into_reference()
                .ok_or(EvalError::ExpectedLocalVariableReference(param_index))?
                .map(GcRef::into_generic);

            // TODO: We could just use a match
            if let Some(return_type) = return_type {
                // fn(JNIEnv*, JObject, JObject) -> jvalue

                let class_ref_jobject: JObject = unsafe { env.get_local_jobject_for(class_ref) };
                let param_ptr: JObject = if let Some(param) = param {
                    unsafe { env.get_local_jobject_for(param) }
                } else {
                    JObject::null()
                };

                let env_ptr = env as *mut Env<'_>;
                // The native function is only partially defined, we have to convert it into a more
                // specific form
                let native_func = native_func.get();
                // Safety: Relying on java's declared parameter types
                let native_func = unsafe {
                    std::mem::transmute::<
                        unsafe extern "C" fn(*mut Env, JObject),
                        unsafe extern "C" fn(*mut Env, JObject, JObject) -> JValue,
                    >(native_func)
                };

                // Safety: Relying on java's declared types and the safety of the code we are
                // calling.
                let value = unsafe { (native_func)(env_ptr, class_ref_jobject, param_ptr) };

                // For value, we can only assume the value is of the same type as the one were
                // given as the return type
                let value = unsafe { value.narrow_from_desc_type_into_value(env, return_type) };

                // TODO: Typecheck return value for classes since they could return a different
                // gcref pointer

                let value: RuntimeValue<ReferenceInstance> = match value {
                    RuntimeValue::Primitive(prim) => prim.into(),
                    RuntimeValue::NullReference => RuntimeValue::NullReference,
                    RuntimeValue::Reference(re) => {
                        let inst = env.state.gc.deref(re).ok_or(EvalError::InvalidGcRef(re))?;
                        if matches!(inst, Instance::Reference(_)) {
                            RuntimeValue::Reference(re.unchecked_as())
                        } else {
                            todo!("Native function return gcref to static class");
                        }
                    }
                };

                return Ok(EvalMethodValue::Return(value));
            }

            // fn(JNIEnv*, JObject, JObject) -> void
            todo!("impl native (one) -> void");
        }
        todo!("Fully implement native methods");
    }

    let mut frame = frame;
    let mut pc = InstructionIndex(0);

    loop {
        let method = env
            .methods
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
            if let Some(class_file) = env.class_files.get(&class_id) {
                tracing::info!(
                    "# ({}) {}",
                    pc.0,
                    inst.as_pretty_string(&mut env.class_names, class_file)
                );
            } else {
                tracing::info!("# ({}) {:?}", pc.0, inst);
            }
        }

        let args = RunInstArgs {
            env,
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

pub struct RunInstArgs<'e, 'i, 'f> {
    pub env: &'e mut Env<'i>,
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
