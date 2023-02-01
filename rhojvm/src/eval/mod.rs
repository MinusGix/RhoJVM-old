// LocalVariableIndex seems to trip up clippy
#![allow(clippy::needless_pass_by_value)]

use classfile_parser::{
    attribute_info::InstructionIndex, constant_info::ConstantInfo,
    constant_pool::ConstantPoolIndexRaw, descriptor::method::MethodDescriptorError,
    method_info::MethodAccessFlags,
};
use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic},
        op::{Inst, Wide, WideInst},
        types::{Instruction, JavaChar, LocalVariableIndex},
    },
    id::{ClassId, ExactMethodId, MethodId},
    map_inst,
    util::{convert_classfile_text, MemorySizeU16},
    StepError,
};

use crate::{
    class_instance::{ClassInstance, FieldId, Instance},
    gc::GcRef,
    jni::{self, JBoolean, JByte, JChar, JDouble, JFloat, JInt, JLong, JObject, JShort},
    method::NativeMethod,
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::{make_class_form_of, Env},
    GeneralError,
};

mod bootstrap;
pub mod class_util;
mod control_flow;
mod func;
pub mod instances;
pub mod internal_repl;
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
    MissingMethod(ExactMethodId),
    /// We tried continuing but the instruction at that index was missing, or it was in the middle
    /// of an instruction
    MissingInstruction(InstructionIndex),
    /// Expected there to be a static class reference for the given class
    MissingStaticClassRef(ClassId),
    /// We expected the field to exist but it did not.
    MissingField(FieldId),

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
    // These bootstrap errors should really be caught in initial verification
    /// We reached an invoke dynamic instruction but there was no bootstrap methods table
    NoBootstrapTable,
    /// We failed to parse the bootstrap methods table
    InvalidBootstrapTable,
    InvalidBootstrapTableIndex(u16),
}
impl From<rhojvm_base::class::InvalidConstantPoolIndex> for EvalError {
    fn from(v: rhojvm_base::class::InvalidConstantPoolIndex) -> EvalError {
        EvalError::InvalidConstantPoolIndex(v.0)
    }
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

/// Helper function to get the class ref value for static/nonstatic methods
/// For native functions, which take a pointer to a gcref
fn get_class_ref(
    env: &mut Env,
    frame: &Frame,
    class_id: ClassId,
    is_static: bool,
) -> Result<ValueException<GcRef<Instance>>, GeneralError> {
    Ok(if is_static {
        // TODO: The from class id is obviously incorrect
        let static_form = make_class_form_of(env, class_id, class_id)?;
        match static_form {
            ValueException::Value(static_form) => ValueException::Value(static_form.into_generic()),
            ValueException::Exception(exc) => ValueException::Exception(exc),
        }
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
    })
}

macro_rules! convert_rv {
    ($env:ident ; $v:ident ; $p_index:ident ; JObject) => {{
        let v = $v
            .into_reference()
            .ok_or(EvalError::ExpectedLocalVariableReference($p_index))?
            .map(GcRef::into_generic);
        if let Some(v) = v {
            unsafe { $env.get_local_jobject_for(v) }
        } else {
            JObject::null()
        }
    }};
    ($env:ident ; $v:ident ; $p_index:ident ; JDouble) => {{
        $v.into_f64()
            .ok_or(EvalError::ExpectedLocalVariableDouble($p_index))?
    }};
    ($env:ident ; $v:ident ; $p_index:ident ; JFloat) => {{
        $v.into_f32()
            .ok_or(EvalError::ExpectedLocalVariableFloat($p_index))?
    }};
    ($env:ident ; $v:ident ; $p_index:ident ; JLong) => {{
        $v.into_i64()
            .ok_or(EvalError::ExpectedLocalVariableLong($p_index))?
    }};
    ($env:ident ; $v:ident ; $p_index:ident ; JInt) => {{
        $v.into_int()
            .ok_or(EvalError::ExpectedLocalVariableIntRepr($p_index))?
    }};
    ($env:ident ; $v:ident ; $p_index:ident ; JShort) => {{
        $v.into_short()
            .ok_or(EvalError::ExpectedLocalVariableIntRepr($p_index))?
    }};
    ($env:ident ; $v:ident ; $p_index:ident ; JChar) => {{
        $v.into_char()
            .ok_or(EvalError::ExpectedLocalVariableIntRepr($p_index))?
            .as_i16() as u16
    }};
    ($env:ident ; $v:ident ; $p_index:ident ; JBoolean) => {{
        $v.into_bool_loose()
            .ok_or(EvalError::ExpectedLocalVariableIntRepr($p_index))?
    }};
    ($env:ident ; $v:ident ; $p_index:ident ; JByte) => {{
        $v.into_byte()
            .ok_or(EvalError::ExpectedLocalVariableIntRepr($p_index))?
    }};
}
macro_rules! impl_call_native_method {
    ($env:ident, $frame:ident, $class_id:ident, $method:ident, $native_func:ident; ($($pname:ident: $typ:ident),*)) => {
        let return_type: Option<DescriptorType> = $method.descriptor().return_type().cloned();
        let is_static = $method.access_flags().contains(MethodAccessFlags::STATIC);
        let class_ref = match get_class_ref($env, &$frame, $class_id, is_static)? {
            ValueException::Value(class_ref) => class_ref,
            ValueException::Exception(exc) => return Ok(EvalMethodValue::Exception(exc)),
        };

        let param_base_index = if is_static {
            0
        } else {
            1
        };
        #[allow(unused_mut, unused_variables)]
        let mut param_index = param_base_index;
        $(
            let $pname = $frame
                .locals
                .get(param_index)
                .ok_or(EvalError::ExpectedLocalVariable(param_index))?
                .as_value()
                .ok_or(EvalError::ExpectedLocalVariableWithValue(param_index))?;
            if $pname.is_category_2() {
                param_index += 2;
            } else {
                param_index += 1;
            }
            let $pname = convert_rv!($env ; $pname ; param_index ; $typ);
        )*

        let class_ref_jobject: JObject = unsafe { $env.get_local_jobject_for(class_ref) };
        let env_ptr: *mut Env<'_> = $env as *mut Env<'_>;
        // The native function is only partially defined, we have to convert it into a more
        // specific form
        let native_func = $native_func.get();
        if let Some(return_type) = return_type {
            // fn(JNIENv*, JObject, ...) -> SomeType

            let rv = RuntimeType::from_descriptor_type(&mut $env.class_names, return_type.clone())
                .map_err(StepError::BadId)?;


            // Safety: Relying on java's declared parameter types
            let return_value: RuntimeValue = match rv {
                RuntimeType::Primitive(prim) => match prim {
                    RuntimeTypePrimitive::I64 => {
                        let native_func = unsafe {
                            std::mem::transmute::<
                                unsafe extern "C" fn(*mut Env, JObject),
                                unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JLong,
                            >(native_func)
                        };
                        let value: JLong = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                        RuntimeValuePrimitive::I64(value).into()
                    },
                    RuntimeTypePrimitive::I32 => {
                        let native_func = unsafe {
                            std::mem::transmute::<
                                unsafe extern "C" fn(*mut Env, JObject),
                                unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JInt,
                            >(native_func)
                        };
                        let value: JInt = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                        RuntimeValuePrimitive::I32(value).into()
                    },
                    RuntimeTypePrimitive::I16 => {
                        let native_func = unsafe {
                            std::mem::transmute::<
                                unsafe extern "C" fn(*mut Env, JObject),
                                unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JShort,
                            >(native_func)
                        };
                        let value: JShort = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                        RuntimeValuePrimitive::I16(value).into()
                    },
                    RuntimeTypePrimitive::I8 => {
                        // TODO: This might need more handling for bools
                        let native_func = unsafe {
                            std::mem::transmute::<
                                unsafe extern "C" fn(*mut Env, JObject),
                                unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JByte,
                            >(native_func)
                        };
                        let value: JByte = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                        RuntimeValuePrimitive::I8(value).into()
                    },
                    RuntimeTypePrimitive::Bool => {
                        // TODO: This might need more handling for bools
                        let native_func = unsafe {
                            std::mem::transmute::<
                                unsafe extern "C" fn(*mut Env, JObject),
                                unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JBoolean,
                            >(native_func)
                        };
                        let value: JBoolean = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                        RuntimeValuePrimitive::Bool(value).into()
                    },
                    RuntimeTypePrimitive::F32 => {
                        let native_func = unsafe {
                            std::mem::transmute::<
                                unsafe extern "C" fn(*mut Env, JObject),
                                unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JFloat,
                            >(native_func)
                        };
                        let value: JFloat = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                        RuntimeValuePrimitive::F32(value).into()
                    },
                    RuntimeTypePrimitive::F64 => {
                        let native_func = unsafe {
                            std::mem::transmute::<
                                unsafe extern "C" fn(*mut Env, JObject),
                                unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JDouble,
                            >(native_func)
                        };
                        let value: JDouble = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                        RuntimeValuePrimitive::F64(value).into()
                    },
                    RuntimeTypePrimitive::Char => {
                        let native_func = unsafe {
                            std::mem::transmute::<
                                unsafe extern "C" fn(*mut Env, JObject),
                                unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JChar,
                            >(native_func)
                        };
                        let value: JChar = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                        let value = JavaChar(value);
                        RuntimeValuePrimitive::Char(value).into()
                    },
                },
                RuntimeType::Reference(_) => {
                    let native_func = unsafe {
                        std::mem::transmute::<
                            unsafe extern "C" fn(*mut Env, JObject),
                            unsafe extern "C" fn(*mut Env, JObject, $($typ),*) -> JObject,
                        >(native_func)
                    };
                    let value: JObject = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
                    // TODO: Check validity
                    let value: Option<GcRef<_>> = unsafe { $env.get_jobject_as_gcref(value) };
                    if let Some(value) = value {
                        let inst = $env.state.gc.deref(value).ok_or(EvalError::InvalidGcRef(value))?;
                        if matches!(inst, Instance::Reference(_)) {
                            RuntimeValue::Reference(value.unchecked_as())
                        } else {
                            todo!("native function returned gcref to static class");
                        }
                    } else {
                        RuntimeValue::NullReference
                    }
                },
            };

            if let Some(native_exception) = $env.state.native_exception.take() {
                // Ignore the return value, assume it is garbage
                return Ok(EvalMethodValue::Exception(native_exception));
            } else {
                return Ok(EvalMethodValue::Return(return_value));
            }
        } else {
            let native_func = unsafe {
                std::mem::transmute::<
                    unsafe extern "C" fn(*mut Env, JObject),
                    unsafe extern "C" fn(*mut Env, JObject, $($typ),*),
                >(native_func)
            };
            let _: () = unsafe { (native_func)(env_ptr, class_ref_jobject, $($pname),*) };
            if let Some(native_exception) = $env.state.native_exception.take() {
                return Ok(EvalMethodValue::Exception(native_exception));
            } else {
                return Ok(EvalMethodValue::ReturnVoid);
            }
        }
    };
}

/// Either a value or an exception
#[derive(Debug, Clone, Copy)]
pub enum ValueException<V> {
    Value(V),
    Exception(GcRef<ClassInstance>),
}
impl<V> ValueException<V> {
    pub fn map<A, F: FnOnce(V) -> A>(self, op: F) -> ValueException<A> {
        match self {
            ValueException::Value(v) => ValueException::Value((op)(v)),
            ValueException::Exception(exc) => ValueException::Exception(exc),
        }
    }
}
impl ValueException<GcRef<ClassInstance>> {
    #[must_use]
    pub fn flatten(self) -> GcRef<ClassInstance> {
        match self {
            ValueException::Value(v) | ValueException::Exception(v) => v,
        }
    }
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
    frame: Frame,
) -> Result<EvalMethodValue, GeneralError> {
    let method_id = match method_id {
        MethodId::Exact(exact) => exact,
        MethodId::ArrayClone => return eval_array_clone(env, frame),
    };

    let (class_id, _) = method_id.decompose();

    let method = env
        .methods
        .get_mut(&method_id)
        .ok_or(EvalError::MissingMethod(method_id))?;
    method.load_code(&mut env.class_files)?;

    let span = tracing::span!(tracing::Level::INFO, "eval_method");
    let _guard = span.enter();

    {
        if let Some(class_file) = env.class_files.get(&class_id) {
            let method_name = class_file.get_text_b(method.name_index()).unwrap();
            let class_name = env.class_names.tpath(class_id);
            let desc = method.descriptor().as_pretty_string(&env.class_names);
            tracing::info!(
                "Executing Method: {}::{} {}",
                class_name,
                convert_classfile_text(method_name),
                desc,
            );
        } else {
            tracing::info!("Executing Method (No Backing Class File):");
        }
    }

    // TODO: native exceptions
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
                    // TODO: When this happens it seems like there's an extra garbage character at the end?
                    // Am I forgetting to remove that?
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

        // This is doing a hellish way of calling the native function, because we don't have a good
        // way to dynamically push the arguments the functions need. I think we would *have* to
        // write manual assembler to do the full general case. However I want to support platforms
        // that Rust can compile to, even if it can only interpret the JVM code rather than jit
        // However, in the future, we could write various versions for common platforms and then
        // only compile these absurd manual calls for non-directly-supported platforms
        let param_count = method.descriptor().parameters().len();
        if param_count == 0 {
            impl_call_native_method!(env, frame, class_id, method, native_func; ());
        } else if param_count == 1 {
            let first = method.descriptor().parameters()[0];
            match first {
                DescriptorType::Array { .. }
                | DescriptorType::Basic(DescriptorTypeBasic::Class(_)) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject));
                }
                DescriptorType::Basic(DescriptorTypeBasic::Byte) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JByte));
                }
                DescriptorType::Basic(DescriptorTypeBasic::Boolean) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JBoolean));
                }
                DescriptorType::Basic(DescriptorTypeBasic::Char) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JChar));
                }
                DescriptorType::Basic(DescriptorTypeBasic::Double) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JDouble));
                }
                DescriptorType::Basic(DescriptorTypeBasic::Float) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JFloat));
                }
                DescriptorType::Basic(DescriptorTypeBasic::Int) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JInt));
                }
                DescriptorType::Basic(DescriptorTypeBasic::Long) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JLong));
                }
                DescriptorType::Basic(DescriptorTypeBasic::Short) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JShort));
                }
            }
        } else if param_count == 2 {
            let first = method.descriptor().parameters()[0];
            let second = method.descriptor().parameters()[1];
            match (first, second) {
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JInt, param2: JInt));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Long),
                    DescriptorType::Basic(DescriptorTypeBasic::Long),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JLong, param2: JLong));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Long),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JLong, param2: JInt));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Boolean),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JBoolean));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JInt));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Long),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JLong));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JObject));
                }
                _ => todo!("Fully implement two parameter native methods"),
            }
        } else if param_count == 3 {
            let first = method.descriptor().parameters()[0];
            let second = method.descriptor().parameters()[1];
            let third = method.descriptor().parameters()[2];
            match (first, second, third) {
                (
                    DescriptorType::Array { .. }
                    | DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JInt, param3: JInt));
                }
                (
                    DescriptorType::Array { .. }
                    | DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Long),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JLong, param3: JInt));
                }
                (
                    DescriptorType::Array { .. }
                    | DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Long),
                    DescriptorType::Basic(DescriptorTypeBasic::Long),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JLong, param3: JLong));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Boolean),
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JBoolean, param3: JObject));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Long),
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JLong, param3: JObject));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Array { .. },
                    DescriptorType::Array { .. },
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JObject, param3: JObject));
                }
                _ => todo!("Fully implement three parameter native methods: {first:?}, {second:?}, {third:?}"),
            }
        } else if param_count == 4 {
            let first = method.descriptor().parameters()[0];
            let second = method.descriptor().parameters()[1];
            let third = method.descriptor().parameters()[2];
            let fourth = method.descriptor().parameters()[3];
            match (first, second, third, fourth) {
                (
                    DescriptorType::Array { .. }
                    | DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                    DescriptorType::Basic(DescriptorTypeBasic::Boolean),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JInt, param3: JInt, param4: JBoolean));
                }
                (
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                    DescriptorType::Array { .. }
                    | DescriptorType::Basic(DescriptorTypeBasic::Class(_)),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                    DescriptorType::Basic(DescriptorTypeBasic::Int),
                ) => {
                    impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JInt, param2: JObject, param3: JInt, param4: JInt));
                }
                _ => todo!("Fully implement four parameter native methods"),
            }
        } else if param_count == 5
            && matches!(
                method.descriptor().parameters()[0],
                DescriptorType::Array { .. } | DescriptorType::Basic(DescriptorTypeBasic::Class(_))
            )
            && matches!(
                method.descriptor().parameters()[1],
                DescriptorType::Basic(DescriptorTypeBasic::Int)
            )
            && matches!(
                method.descriptor().parameters()[2],
                DescriptorType::Array { .. } | DescriptorType::Basic(DescriptorTypeBasic::Class(_))
            )
            && matches!(
                method.descriptor().parameters()[3],
                DescriptorType::Basic(DescriptorTypeBasic::Int)
            )
            && matches!(
                method.descriptor().parameters()[4],
                DescriptorType::Basic(DescriptorTypeBasic::Int)
            )
        {
            // Specifically for arraycopy
            impl_call_native_method!(env, frame, class_id, method, native_func; (param1: JObject, param2: JInt, param3: JObject, param4: JInt, param5: JInt));
        }
        tracing::info!("Method: {:?}", method.descriptor());
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

            let should_log = if env.state.conf.log_only_control_flow_insts {
                matches!(
                    inst,
                    Inst::InvokeDynamic(_)
                        | Inst::InvokeStatic(_)
                        | Inst::InvokeInterface(_)
                        | Inst::InvokeVirtual(_)
                        | Inst::InvokeSpecial(_)
                )
            } else {
                true
            };

            if should_log {
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
        }

        let args = RunInstArgs {
            env,
            method_id,
            frame: &mut frame,
            inst_index: pc,
        };

        // TODO: Some of the returned errors should maybe be exceptions
        let res = map_inst!(inst; x; RunInst::run(x, args))?;

        match res {
            // TODO: Should we throw an exception if we return a value of the wrong type?
            RunInstValue::Continue => pc.0 += size,
            RunInstValue::ContinueAt(i) => pc = i,
            RunInstValue::ReturnVoid => return Ok(EvalMethodValue::ReturnVoid),
            RunInstValue::Return(x) => return Ok(EvalMethodValue::Return(x)),
            RunInstValue::Exception(exc) => {
                let exception_id = env
                    .state
                    .gc
                    .deref(exc)
                    .ok_or(EvalError::InvalidGcRef(exc.into_generic()))?
                    .instanceof;

                let method = env
                    .methods
                    .get(&method_id)
                    .ok_or(EvalError::MissingMethod(method_id))?;

                let exception_tables = method.code().unwrap().exception_table();
                let exception_tables = exception_tables
                    .iter()
                    .filter(|entry| pc >= entry.start_pc && pc < entry.end_pc);

                let mut jump_to = None;
                for exception in exception_tables {
                    let catch_type = exception.catch_type;
                    if catch_type.is_zero() {
                        // It is for all exceptions
                        jump_to = Some(exception.handler_pc);
                        break;
                    }

                    let class_file = env
                        .class_files
                        .get(&class_id)
                        .ok_or(EvalError::MissingMethodClassFile(class_id))?;
                    let catch_type = class_file
                        .get_t(catch_type)
                        .ok_or(GeneralError::BadClassFileIndex(catch_type.into_generic()))?;
                    let catch_type = class_file.get_text_b(catch_type.name_index).ok_or(
                        GeneralError::BadClassFileIndex(catch_type.name_index.into_generic()),
                    )?;
                    let catch_type_id = env.class_names.gcid_from_bytes(catch_type);

                    let is_castable = exception_id == catch_type_id
                        || env.classes.is_super_class(
                            &mut env.class_names,
                            &mut env.class_files,
                            &mut env.packages,
                            exception_id,
                            catch_type_id,
                        )?
                        || env.classes.implements_interface(
                            &mut env.class_names,
                            &mut env.class_files,
                            exception_id,
                            catch_type_id,
                        )?;

                    if is_castable {
                        jump_to = Some(exception.handler_pc);
                        break;
                    }
                }

                if let Some(jump_to) = jump_to {
                    // We have a location to jump to
                    frame
                        .stack
                        .push(RuntimeValue::Reference(exc.into_generic()))?;
                    pc = jump_to;
                } else {
                    // Otherwise, we bubble the exception up
                    return Ok(EvalMethodValue::Exception(exc));
                }
            }
        }
    }
}

fn eval_array_clone(env: &mut Env, frame: Frame) -> Result<EvalMethodValue, GeneralError> {
    // Array clones are pretty simple, they just do a shallow clone

    let array = frame
        .locals
        .get(0)
        .ok_or(EvalError::ExpectedLocalVariable(0))?;
    let array = array
        .as_value()
        .ok_or(EvalError::ExpectedLocalVariableWithValue(0))?;
    let array = array
        .into_reference()
        .ok_or(EvalError::ExpectedLocalVariableReference(0))?;
    if let Some(array) = array {
        let array_dupe = env
            .state
            .gc
            .shallow_clone(array)
            .ok_or(EvalError::InvalidGcRef(array.into_generic()))?;
        Ok(EvalMethodValue::Return(RuntimeValue::Reference(array_dupe)))
    } else {
        todo!("NPE");
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

#[derive(Debug, Clone)]
pub enum RunInstContinueValue {
    Exception(GcRef<ClassInstance>),
    Continue,
}

pub struct RunInstArgs<'e, 'i, 'f> {
    pub env: &'e mut Env<'i>,
    pub method_id: ExactMethodId,
    pub frame: &'f mut Frame,
    /// Index into 'bytes' of instructions, which is more commonly used in code
    pub inst_index: InstructionIndex,
}
/// [`RunInstArgs`] but with a potentially nonexistent instruction index
/// because instructions that implement [`RunInstContinue`] shouldn't rely on it.
pub struct RunInstArgsC<'e, 'i, 'f> {
    pub env: &'e mut Env<'i>,
    pub method_id: ExactMethodId,
    pub frame: &'f mut Frame,
    pub inst_index: Option<InstructionIndex>,
}

pub trait RunInst: Instruction {
    fn run(self, args: RunInstArgs) -> Result<RunInstValue, GeneralError>;
}

/// RunInst but it merely continues to the next instruction or returns an error
/// This helps certain code behave better
pub trait RunInstContinue: Instruction {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError>;
}

impl<T: RunInstContinue> RunInst for T {
    fn run(self, args: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        match <T as RunInstContinue>::run(
            self,
            RunInstArgsC {
                env: args.env,
                method_id: args.method_id,
                frame: args.frame,
                inst_index: Some(args.inst_index),
            },
        )? {
            RunInstContinueValue::Exception(exc) => Ok(RunInstValue::Exception(exc)),
            RunInstContinueValue::Continue => Ok(RunInstValue::Continue),
        }
    }
}

impl RunInst for Wide {
    fn run(self, args: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        // TODO: The passed in inst index is wrong, but that doesn't matter for the current set of
        // instructions
        match self.0 {
            WideInst::WideIntLoad(x) => RunInst::run(x, args),
            WideInst::WideIntIncrement(x) => RunInst::run(x, args),
        }
    }
}
