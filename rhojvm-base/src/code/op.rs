use classfile_parser::attribute_info::InstructionIndex;
use classfile_parser::constant_info::{
    ClassConstant, ConstantInfo, FieldRefConstant, InterfaceMethodRefConstant,
    InvokeDynamicConstant,
};
use classfile_parser::constant_pool::ConstantPoolIndexRaw;

use crate::code::op_ex::InstructionParseError;
use crate::code::types::{
    Byte, Char, ComplexType, Double, Float, Int, LocalVariableIndexByte, Long, ParseOutput,
    PopIndex, PrimitiveType, PushIndex, Short, Type, UnsignedByte, UnsignedShort, WithType,
};
use crate::util::{MemorySize, StaticMemorySize};

macro_rules! define_pop {
    ($for:ident, [$(
        $(#[$pop_outer:meta])*
        $pop_name:ident : $pop_ty:expr
    ),* $(,)*]) => {
        impl $for {
            #[must_use]
            pub fn pop_type_at(&self, i: PopIndex) -> Option<Type> {
                let pops = [$(Type::from($pop_ty)),*];
                std::array::IntoIter::new(pops).nth(i)
            }

            #[must_use]
            pub fn pop_type_name_at(&self, i: PopIndex) -> Option<&'static str> {
                let pops = [$(stringify!($pop_name)),*];
                std::array::IntoIter::new(pops).nth(i)
            }
        }
    };
    ($for:ident, [{extern}]) => {};
}
macro_rules! define_push {
    ($for:ident, [$(
        $(#[$push_outer:meta])*
        $push_name:ident : $push_ty:expr
    ),* $(,)*]) => {
        impl $for {
            #[must_use]
            pub fn push_type_at(&self, i: PushIndex) -> Option<Type> {
                let push = [$(Type::from($push_ty)),*];
                std::array::IntoIter::new(push).nth(i)
            }

            #[must_use]
            pub fn push_type_name_at(&self, i: PushIndex) -> Option<&'static str> {
                let push = [$(stringify!($push_name)),*];
                std::array::IntoIter::new(push).nth(i)
            }
        }
    };
    ($for:ident, [{extern}]) => {};
}

macro_rules! define_init {
    // NOTE: We should deliberately not allow an empty init parameters (if we create init params)
    ($for:ident, []) => {};
    ($for:ident, [$($self:ident)?; RequireValidCodeOffset($index:expr)]) => {};
    ($for:ident, [$($self:ident)?; RequireValidLocalVariableIndex($index:expr)]) => {};
    ($for:ident, [{extern}]) => {};
}

/// Basically, if there is data, then don't do perform
/// but if there is, then do this other thing
macro_rules! inverse_do {
    ({$($perform:tt)*} : $($data:tt)+) => {};
    ({$($perform:tt)*}: ) => {
        $($perform)*
    };
}

macro_rules! define_instruction {
    (
        $(#[$name_outer:meta])*
        $name:ident : {extern}
    ) => {

    };
    ($(#[$name_outer:meta])*
    $name: ident: {
        opcode: $opcode:expr,
        args: [$(
            $(#[$arg_outer:meta])*
            $arg:ident : $arg_ty:ty
        ),* $(,)*],
        pop: [$($pop_data:tt)*],
        push: [$($push_data:tt)*],
        exceptions: [$(
            $(#[$exception_outer:meta])*
            $exception_name:ident
        ),* $(,)*],
        $(init: [$($init_data:tt)*],)?
    }) => {
        $(#[$name_outer])*
            #[derive(Debug, Clone)]
            pub struct $name {
                $(
                    pub $arg : <$arg_ty as ParseOutput>::Output,
                )*
            }
            impl $name {
                /// Assured that data has enough bytes for it
                #[allow(unused_variables, unused_mut, unused_assignments)]
                pub(crate) fn parse(data: &[u8], idx: InstructionIndex) -> Result<$name, InstructionParseError> {
                    let data = &data[idx.0 as usize..];
                    let needed_size: usize = $name::MEMORY_SIZE;
                    if data.len() < needed_size {
                        return Err(InstructionParseError::NotEnoughData {
                            opcode: Self::OPCODE,
                            needed: needed_size,
                            had: data.len(),
                        });
                    }

                    // Skip over the
                    let mut idx = 1;
                    $(
                        let size = <$arg_ty>::MEMORY_SIZE;
                        let $arg = <$arg_ty>::parse(&data[idx..(idx + size)]);
                        idx += size;
                    )*
                    Ok(Self {
                        $(
                            $arg,
                        )*
                    })
                }
            }
            impl $name {
                pub const OPCODE: RawOpcode = $opcode;
            }
            // The size of themselves in the code
            impl StaticMemorySize for $name {
                const MEMORY_SIZE: usize = 1 + $(<$arg_ty>::MEMORY_SIZE +)* 0;
            }
            define_pop!($name, [$($pop_data)*]);
            define_push!($name, [$($push_data)*]);
            $(define_init!($name, [$($init_data)*]);)?
            // TODO: is there a simpler way of generating code for the case where there is no init?
            inverse_do!({define_init!($name, []);} : $($($init_data)*)?);
    };
}

/// Define the instructions (opcodes)
/// Note: the opcode expr should be a simple number that can be used in a match expression.
macro_rules! define_instructions {
    ([$(
        $(#[$name_outer:meta])*
        $name:ident : {$($data:tt)*},
    )+],
    WIDE_INSTR: [$(
        $(#[$wide_name_outer:meta])*
        $wide_name:ident : {$($wide_data:tt)*},
    )+]) => {
        $(
            define_instruction!(
                // $(#[name_outer])*
                $name : {$($data)*}
            );
        )+

        $(
            define_instruction!(
                $wide_name : {$($wide_data)*}
            );
        )+

        #[allow(dead_code)]
        fn check_instruction_duplicates() {
            let info: &[(&str, RawOpcode)] = &[
                $(
                    (stringify!($name), $name::OPCODE)
                ),+
            ];

            for (li, (ls, lo)) in info.iter().enumerate() {
                for (ri, (rs, ro)) in info.iter().enumerate() {
                    if li == ri {
                        continue;
                    }

                    if rs == ls {
                        // This should already be a compile error since they're declared as structs
                        panic!("Duplicate opcode name!: '{}'", ls);
                    }

                    if lo == ro {
                        panic!("Duplicate opcode!: '{}' and '{}' with {}", ls, rs, lo);
                    }
                }
            }
        }

        #[derive(Clone)]
        pub enum InstM {
            $(
                $(#[$name_outer])*
                $name ($name)
            ),+
        }
        impl InstM {
            #[allow(unused_variables)]
            pub fn parse(code: &[u8], idx: InstructionIndex) -> Result<InstM, InstructionParseError> {
                let opcode: RawOpcode = code
                    .get(idx.0 as usize).copied()
                    .ok_or(InstructionParseError::ExpectedOpCodeAt(idx))?;
                match opcode {
                    $(
                        $name::OPCODE => {
                            Ok(InstM::$name($name::parse(code, idx)?))
                        }
                    )+
                    _ => return Err(InstructionParseError::UnknownOpcode {
                        idx,
                        opcode,
                    })
                }
            }

            #[must_use]
            pub fn pop_type_at(&self, i: usize) -> Option<Type> {
                match self {
                    $(
                        InstM::$name(x) => x.pop_type_at(i),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => return None,
                }
            }

            #[must_use]
            pub fn push_type_at(&self, i: usize) -> Option<Type> {
                match self {
                    $(
                        InstM::$name(x) => x.push_type_at(i),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => return None,
                }
            }
        }
        impl MemorySize for InstM {
            fn memory_size(&self) -> usize {
                match self {
                    $(
                        InstM::$name(v) => v.memory_size(),
                    )*
                    // The macro enjoys erroring
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }
        // Custom formatting to only print the struct, which is nicer to see
        impl std::fmt::Debug for InstM {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        InstM::$name(v) => std::fmt::Debug::fmt(v, f),
                    )*
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }

        #[derive(Clone)]
        pub enum WideInstM {
            $(
                $(#[$wide_name_outer])*
                $wide_name ($wide_name)
            ),+
        }
        impl WideInstM {
            #[allow(unused_variables)]
            pub fn parse(code: &[u8], idx: InstructionIndex) -> Result<WideInstM, InstructionParseError> {
                let opcode: RawOpcode = code
                    .get(idx.0 as usize).copied()
                    .ok_or(InstructionParseError::ExpectedOpCodeAt(idx))?;
                match opcode {
                    $(
                        $wide_name::OPCODE => {
                            Ok(WideInstM::$wide_name($wide_name::parse(code, idx)?))
                        }
                    )+
                    _ => return Err(InstructionParseError::UnknownWideOpcode {
                        idx,
                        opcode,
                    })
                }
            }

            #[must_use]
            pub fn pop_type_at(&self, i: usize) -> Option<Type> {
                match self {
                    $(
                        WideInstM::$wide_name(x) => x.pop_type_at(i),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => return None,
                }
            }

            #[must_use]
            pub fn push_type_at(&self, i: usize) -> Option<Type> {
                match self {
                    $(
                        WideInstM::$wide_name(x) => x.push_type_at(i),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => return None,
                }
            }
        }
        impl MemorySize for WideInstM {
            fn memory_size(&self) -> usize {
                match self {
                    $(
                        WideInstM::$wide_name(v) => v.memory_size(),
                    )*
                    // The macro enjoys erroring
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }
        // Custom formatting to only print the struct, which is nicer to see
        impl std::fmt::Debug for WideInstM {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        WideInstM::$wide_name(v) => std::fmt::Debug::fmt(v, f),
                    )*
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }
    };
}

pub type RawOpcode = u8;

define_instructions! {[
    /// Load a reference from an array with an index
    AALoad: {
        opcode: 0x32,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayRefAny,
            index: WithType::IntArrayIndexInto(0),
        ],
        push: [
            res: WithType::RefArrayRefType(0),
        ],
        exceptions: [
            /// If arrayref is null
            NullPointerException,
            /// If index is not within the bounds of arrayref
            ArrayIndexOutOfBoundsException,
        ],
    },
    /// Stores a reference to a value into an array at an index
    AAStore: {
        opcode: 0x53,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayRefAny,
            index: WithType::IntArrayIndexInto(0),
            // TODO: there is technically more specific rules for this
            // that may need to be specifically supported
            value: WithType::RefArrayRefType(0),
        ],
        push: [],
        exceptions: [
            /// If arrayref is null
            NullPointerException,
            /// Index is not within the bounds of arrayref
            ArrayIndexOutOfBoundsException,
            /// if arrayref is not-null, and the actual type of value is not assignment
            /// compatible with the actual type
            ArrayStoreException,
        ],
    },
    /// Pushes null to the stack
    AConstNull: {
        opcode: 0x1,
        args: [],
        pop: [],
        push: [null: ComplexType::ReferenceNull],
        exceptions: [],
    },
    /// Load a reference from a local variable
    /// Cannot load a value of type returnAddress
    ALoad: {
        opcode: 0x19,
        args: [
            /// Index into the local variable array of the current frame
            /// The locvar at index must contain a reference
            index: LocalVariableIndexByte,
        ],
        pop: [],
        push: [
            objectref: WithType::LocalVariableRefAtNoRetAddr(0),
        ],
        exceptions: [],
        init: [inst; RequireValidLocalVariableIndex(inst.index as u16)],
    },
    // TODO: We really need some way to generate these automatically, they are just adding
    // one onto the opcode each time.

    /// Indexes into the local variable array at index 0
    /// The variable at that position must be a reference.
    /// This cannot result a value of returnAddress
    ALoad0: {
        opcode: 0x2A,
        args: [],
        pop: [],
        push: [
            objectref: WithType::LocalVariableRefAtIndexNoRetAddr(0),
        ],
        exceptions: [],
        init: [; RequireValidLocalVariableIndex(0)],
    },
    /// Indexes into the local variable array at index 1
    /// The variable at that position must be a reference.
    /// This cannot result a value of returnAddress
    ALoad1: {
        opcode: 0x2B,
        args: [],
        pop: [],
        push: [
            objectref: WithType::LocalVariableRefAtIndexNoRetAddr(1),
        ],
        exceptions: [],
        init: [; RequireValidLocalVariableIndex(1)],
    },
    /// Indexes into the local variable array at index 2
    /// The variable at that position must be a reference.
    /// This cannot result a value of returnAddress
    ALoad2: {
        opcode: 0x2C,
        args: [],

        pop: [],
        push: [
            objectref: WithType::LocalVariableRefAtIndexNoRetAddr(2),
        ],
        exceptions: [],
        init: [; RequireValidLocalVariableIndex(2)],
    },
    /// Indexes into the local variable array at index 0
    /// The variable at that position must be a reference.
    /// This cannot result a value of returnAddress
    ALoad3: {
        opcode: 0x2D,
        args: [],
        pop: [],
        push: [
            objectref: WithType::LocalVariableRefAtIndexNoRetAddr(3),
        ],
        exceptions: [],
        init: [; RequireValidLocalVariableIndex(3)],
    },
    /// Create a new array of reference
    /// index is into the constant pool of the current class.
    /// It must be an index to a symbolic-ref to a class, an array, or an interface type.
    /// It is then resolved, and a new array with components of that type with length `count`
    /// is allocated from the gc-heap and a reference `arrayref` to this array is returned.
    /// All components are initialized to `null`.
    ANewArray: {
        opcode: 0xBD,
        args: [
            index: ConstantPoolIndexRaw<ClassConstant>,
        ],
        pop: [
            /// The number of entries in the array
            count: Int,
        ],
        push: [{extern}],
        exceptions: [
            // There's also the usual allocation failures
            /// If the class is not accessible
            IllegalAccessError,
            /// If count < 0
            NegativeArraySizeException,
        ],
        init: [{extern}],
    },
    /// Create a new multidimensional array
    MultiANewArray: {
        opcode: 0xC5,
        args: [
            index: ConstantPoolIndexRaw<ClassConstant>,
            dimensions: UnsignedByte,
        ],
        // TODO: #[dimensions] count variable
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO:

            /// Current class does not have permission to access the type
            IllegalAccessError,

            /// If any of the count values are < 0
            NegativeArraySizeException,
        ],
        init: [{extern}],
    },
    // TODO: Type that signifies tag of primitive values
    /// Creates new array of primitive values
    NewArray: {
        opcode: 0xBC,
        // TODO: is atype a byte?
        args: [atype: UnsignedByte],
        pop: [count: Int],
        push: [{extern}],
        exceptions: [
            /// count < 0
            NegativeArraySizeException,
        ],
        init: [{extern}],
    },
    /// Return reference from method
    /// If the current method is `synchronized`, the monitor {re,}-entered is updated and
    /// possibly exited as if by `monitorexit` in the current thread.
    /// If no exception is thrown, objectref is popped from this stack and pushed onto the
    /// stack of the frame of the invoker. Any other values in the stack are discarded.
    AReturn: {
        opcode: 0xB0,
        args: [],
        pop: [
            /// Must be assignment compatible with the type in the return descriptor
            objectref: ComplexType::ReferenceAny
        ],
        push: [],
        // TODO: Could we have a return field?
        exceptions: [
            /// The current method is synchronized and the current thread is *not* the owner of
            /// the monitor
            IllegalMonitorStateException,
        ],
    },
    /// Get length of array
    ArrayLength: {
        opcode: 0xBE,
        args: [],
        pop: [arrayref: ComplexType::RefArrayAny],
        // TODO: Array length type?
        push: [length: Int],
        exceptions: [
            /// If arrayref is null
            NullPointerException,
        ],
    },
    /// Store reference into local variable
    AStore: {
        opcode: 0x3A,
        args: [
            /// Index into local variable array of current frame
            index: LocalVariableIndexByte
        ],
        pop: [
            // TODO: custom type for returnAddress?
            /// Must be of type returnAddress | reference
            objectref: ComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        init: [inst; RequireValidLocalVariableIndex(inst.index as u16)],
    },
    /// Store reference into local variable 0
    AStore0: {
        opcode: 0x4B,
        args: [],
        pop: [
            /// Must be of type returnAddress | reference
            objectef: ComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        init: [; RequireValidLocalVariableIndex(0)],
    },
    /// Store reference into local variable 1
    AStore1: {
        opcode: 0x4C,
        args: [],
        pop: [
            /// Must be of type returnAddress | reference
            objectef: ComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        init: [; RequireValidLocalVariableIndex(1)],
    },
    /// Store reference into local variable 2
    AStore2: {
        opcode: 0x4D,
        args: [],
        pop: [
            /// Must be of type returnAddress | reference
            objectef: ComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        init: [; RequireValidLocalVariableIndex(2)],
    },
    /// Store reference into local variable 3
    AStore3: {
        opcode: 0x4E,
        args: [],
        pop: [
            /// Must be of type returnAddress | reference
            objectef: ComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        init: [; RequireValidLocalVariableIndex(3)],
    },

    InvokeSpecial: {
        opcode: 0xB7,
        args: [
            /// Index into constant pool
            /// The value at that idx must be a symbolic reference to a method or an interface
            /// method which gives the name and descriptor of the method as well as a symbolic
            /// ref to the class or interface in which the method is to be found. The named
            /// method is resolved.
            /// If the resolved method is protected, and it is a member of a superclass of the
            /// current class, and the method is not declared in the same package as the current
            /// class, then the class of objectref must be either the current class or a subclass
            /// of the current class.
            index: ConstantPoolIndexRaw<ConstantInfo>,
        ],
        pop: [{extern}],
        push: [],
        exceptions: [
            /// If the resolved method is an instance init method, and the class in which
            /// it is declared is not the class symbolically referenced
            /// or if method lookup fails
            NoSuchMethodError,
            /// If method lookup succeeds and the referenced method is inaccessible
            IllegalAccessError,
            /// If the resolved method is a class static method
            /// If there is multiple max-specific methods
            IncompatibleClassChangeError,
            /// If objectref is null
            NullPointerException,
            /// If it was an abstract method
            AbstractMethodError,
            /// If it was a nativem ethod and the code that implements it can't be bound
            UnsatisfiedLinkError,
        ],
        init: [{extern}],
    },
    InvokeInterface: {
        opcode: 0xB9,
        args: [
            index: ConstantPoolIndexRaw<InterfaceMethodRefConstant>,
            count: UnsignedByte,
            zero: UnsignedByte,
        ],
        pop: [
            objectref: ComplexType::ReferenceAny,
            // TODO: Args
        ],
        push: [],
        exceptions: [
            // TODO
        ],
        init: [{extern}],
    },
    InvokeDynamic: {
        opcode: 0xBA,
        args: [
            index: ConstantPoolIndexRaw<InvokeDynamicConstant>,
            zero1: Byte,
            zero2: Byte,
        ],
        pop: [
            // TODO: args
        ],
        push: [
            // TODO: args?
        ],
        exceptions: [
            // TODO: exceptions
        ],
        init: [{extern}],
    },

    Return: {
        opcode: 0xB1,
        args: [],
        pop: [],
        push: [],
        exceptions: [
            IllegalMonitorStateException,
        ],
    },
    /// ireturn
    /// Return an integer
    IntReturn: {
        opcode: 0xAC,
        args: [],
        pop: [ret_val: Int],
        push: [],
        exceptions: [
            IllegalMonitorStateException,
        ],
    },
    /// lreturn
    /// Return a long
    LongReturn: {
        opcode: 0xAD,
        args: [],
        pop: [ret_val: Long],
        push: [],
        exceptions: [
            // TODO
        ],
    },
    /// dreturn
    /// Return a double
    DoubleReturn: {
        opcode: 0xAF,
        args: [],
        pop: [ret_val: Double],
        push: [],
        exceptions: [
            // TODO
        ],
    },
    /// freturn
    /// Return a float
    FloatReturn: {
        opcode: 0xAE,
        args: [],
        pop: [ret_val: Float],
        push: [],
        exceptions: [
            // TODO
        ],
    },

    GetStatic: {
        opcode: 0xB2,
        args: [
            /// Index into constant pool
            /// Must be a symbolic reference to a field which gives the name and descriptor
            /// of the field as well as a symbol reference to the class or interface in which
            /// the field is to be found. The referenced field is resolved.
            index: ConstantPoolIndexRaw<FieldRefConstant>,
        ],
        pop: [],
        push: [],
        exceptions: [
            /// If field lookup fails
            NoSuchFieldError,
            /// Field lookup succeeds but the referenced field is not accessible
            IllegalMonitorStateException,
            /// If the resolved field is a not a static class field or an interface field
            IncompatibleClassChangeError,
        ],
        init: [{extern}],
    },
    /// ldc
    /// If the value is an int | float, it pushes a int or float
    /// If it is a ref-to-string-lit, then a ref to that inst is pushed
    /// If it is a sym-ref-to-class, then it is resolved and a ref to the Class is pushed
    /// If it is sym-ref-to-method-type|method-handle, it is resolved and a reference to the
    ///   result inst of MethodType or MethodHandle is pushed onto the stack.
    LoadConstant: {
        opcode: 0x12,
        args: [
            /// Index into constant pool of current class
            /// Must be a int, float, reference to a string literal, symbolic-ref to a class,
            /// method type, or method handle.
            // TODO: ConstantPoolIndexRawU8
            index: UnsignedByte,
        ],
        pop: [],
        push: [],
        exceptions: [
            // TODO: This is incomplete
            /// If class is not accessible
            IllegalAccessError,
        ],
        init: [{extern}],
    },
    LoadConstantWide: {
        opcode: 0x13,
        args: [
            index: ConstantPoolIndexRaw<ConstantInfo>,
        ],
        pop: [],
        // TODO: This isn't exactly category 1, more 4 byte?
        push: [val: ComplexType::Category1],
        exceptions: [],
        init: [{extern}],
    },
    /// ldc2_w
    /// Push long or double from run-time constant pool
    LoadConstant2Wide: {
        opcode: 0x14,
        args: [
            /// Category 2 (long/double)
            index: ConstantPoolIndexRaw<ConstantInfo>,
        ],
        pop: [],
        push: [value: ComplexType::Category2],
        exceptions: [],
    },


    /// Create new object
    New: {
        opcode: 0xBB,
        args: [
            /// Index into const pool of current class
            /// Value must be a sym-ref to a class or interface
            /// It is resolved and should result in a class type.
            /// Memory is then alloc'd from the GC-heap
            /// And the variables are initialized.
            index: ConstantPoolIndexRaw<ClassConstant>,
        ],
        pop: [],
        push: [
            // TODO: nonnull?
            /// The newly created object
            objectref: ComplexType::ReferenceAny
        ],
        exceptions: [
            // TODO: resolution errors
            /// Resolved to a interface or abstract class
            InstantiationError,
        ],
        init: [{extern}],
    },
    /// Throw exception or error
    AThrow: {
        opcode: 0xBF,
        args: [],
        pop: [
            /// Must be a reference
            /// Must be an instance of (Throwable | Subclass of Throwable)
            /// Searches the current method for the first exception handler
            /// that matches the objectref
            objectref: WithType::RefClassOf {
                class_name: &["java", "lang", "Throwable"],
                // TODO: it can technically be, but it would throw a NullPointerException
                can_be_null: false,
            },
        ],
        push: [],
        exceptions: [
            // TODO: probably incomplete
            /// If objectref is null
            /// This is then thrown instead
            NullPointerException,
            IllegalMonitorStateException,
        ],
    },
    InvokeStatic: {
        opcode: 0xB8,
        args: [
            /// Index into const pool of current class.
            /// Must be a sym-ref to a method or an interface method
            /// which gives the name and descriptor of the method
            /// as well as a sym-ref to the class/interface which the method is found
            /// The method is resolved
            /// Method must not be an instance init method,
            ///  or the class/interface init method
            /// The method must be static, and thus not abstract.
            /// When the method is resolved, the class/interface is initialized
            /// if it has not already been
            index: ConstantPoolIndexRaw<ConstantInfo>,
        ],
        pop: [
            // args
        ],
        push: [],
        exceptions: [
            // TODO
        ],
        init: [{extern}],
    },
    InvokeVirtual: {
        opcode: 0xB6,
        args: [
            /// Index into constant pool
            /// Must be a sym-ref to a method, which gives the name and descriptor of the method
            /// as well as a sym-ref to the class in which the mthod is to be found. It is resolved.
            /// The method must not be an instance init method or the class/interface init method
            /// If the resolved method is protected and it is a member of a superclass of the
            /// current class, and the method is not declared in the same runtime package as the
            /// current class, then the class of object ref must be either the current class or a
            /// subclass of the current class.
            index: ConstantPoolIndexRaw<ConstantInfo>,
        ],
        pop: [
            objectref: ComplexType::ReferenceAny,
            // TODO: arbitrary args
        ],
        push: [],
        exceptions: [
            // TODO: incomplete
            /// If the resolved method is a class static method
            IncompatibleClassChangeError,
            /// objectref is null
            NullPointerException,
            /// Resolved method is a protected method of a superclass of the current class
            /// declared in a different runtime package and the class of objectref is not the
            /// current class or a subclass of the current class.
            IllegalAccessError,
        ],
        init: [{extern}],
    },
    /// Goto a specific offset from the current position
    Goto: {
        opcode: 0xA7,
        args: [
            branch_offset: Short,
        ],
        pop: [],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branch to an offset if two ints are equal
    /// Continues onwards if the condition fails
    IfICmpEq: {
        opcode: 0x9F,
        args: [
            /// Branches to this offset from current position
            branch_offset: Short,
        ],
        pop: [val1: Int, val2: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branch to an offset if two ints are not equal
    /// Continues onwards if the condition fails
    IfICmpNe: {
        opcode: 0xA0,
        args: [
            /// Branches to this offset from current position
            branch_offset: Short,
        ],
        pop: [val1: Int, val2: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branch to an offset if less than
    /// Continues onwards if the condition fails
    IfICmpLt: {
        opcode: 0xA1,
        args: [
            /// Branches to this offset from current position
            branch_offset: Short,
        ],
        pop: [val1: Int, val2: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branch to an offset if greater than or equal
    /// Continues onwards if the condition fails
    IfICmpGe: {
        opcode: 0xA2,
        args: [
            /// Branches to this offset from current position
            branch_offset: Short,
        ],
        pop: [val1: Int, val2: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branch to an offset if greater than
    /// Continues onwards if the condition fails
    IfICmpGt: {
        opcode: 0xA3,
        args: [
            /// Branches to this offset from current position
            branch_offset: Short,
        ],
        pop: [val1: Int, val2: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branch to an offset if less than or equal
    /// Continues onwards if the condition fails
    IfICmpLe: {
        opcode: 0xA4,
        args: [
            /// Branches to this offset from current position
            branch_offset: Short,
        ],
        pop: [val1: Int, val2: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branch to an offset if two references are equal
    /// Continues onwards if the condition fails
    IfACmpEq: {
        opcode: 0xA5,
        args: [
            /// Branches to this offset from current position
            branch_offset: Short,
        ],
        pop: [
            value1: ComplexType::ReferenceAny,
            value2: ComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branch to an offset if two references are not equal
    /// Continues onwards if the condition fails
    IfACmpNe: {
        opcode: 0xA6,
        args: [
            /// Branches to this offset from current position
            branch_offset: Short,
        ],
        pop: [
            value1: ComplexType::ReferenceAny,
            value2: ComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branches to an offset if the int value is zero
    IfEqZero: {
        opcode: 0x99,
        args: [
            branch_offset: Short,
        ],
        pop: [val: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branches to an offset if the int value is not zero
    IfNeZero: {
        opcode: 0x9A,
        args: [
            branch_offset: Short,
        ],
        pop: [val: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branches to an offset if the int value is less than zero
    IfLtZero: {
        opcode: 0x9B,
        args: [
            branch_offset: Short,
        ],
        pop: [val: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branches to an offset if the int value is greater than or equal to zero
    IfGeZero: {
        opcode: 0x9C,
        args: [
            branch_offset: Short,
        ],
        pop: [val: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branches to an offset if the int value is greater than zero
    IfGtZero: {
        opcode: 0x9D,
        args: [
            branch_offset: Short,
        ],
        pop: [val: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branches to an offset if the int value is less than or equal to zero
    IfLeZero: {
        opcode: 0x9E,
        args: [
            branch_offset: Short,
        ],
        pop: [val: Int],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branches if the reference is not null
    IfNonNull: {
        opcode: 0xC7,
        args: [
            branch_offset: Short,
        ],
        pop: [val: ComplexType::ReferenceAny],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },
    /// Branches if the reference is null
    IfNull: {
        opcode: 0xC6,
        args: [
            branch_offset: Short,
        ],
        pop: [val: ComplexType::ReferenceAny],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },

    /// Duplicate the top stack value
    Dup: {
        opcode: 0x59,
        args: [],
        pop: [val: ComplexType::Category1],
        push: [val: ComplexType::Category1, val_dup: ComplexType::Category1],
        exceptions: [],
    },
    /// Duplicate the top two or one stack values
    /// Two category 1
    /// or one category two
    Dup2: {
        opcode: 0x5C,
        args: [],
        // TODO: being able to better represent this would be nice
        pop: [val1: ComplexType::Category1Sized, val2: ComplexType::Category1Sized],
        push:[r1: ComplexType::Category1Sized, r2: ComplexType::Category1Sized, r3: ComplexType::Category1Sized, r4: ComplexType::Category1Sized],
        exceptions: [],
    },
    /// Duplicate top stack value and inset the duplicate two values down
    /// pop v2
    /// pop v1
    /// push v1
    /// push v2
    /// push 1
    DupX1: {
        opcode: 0x5A,
        args: [],
        pop: [val2: ComplexType::Category1, val1: ComplexType::Category1],
        push: [val1_dup: ComplexType::Category1, val2_n: ComplexType::Category1, val1_n: ComplexType::Category1],
        exceptions: [],
    },
    DupX2: {
        opcode: 0x5B,
        args: [],
        pop: [val3: ComplexType::Category1, val2: ComplexType::Category1, val1: ComplexType::Category1],
        push: [val1_dup: ComplexType::Category1, val3_dup: ComplexType::Category1, val2_dup: ComplexType::Category1, val1_dup2: ComplexType::Category1],
        exceptions: [],
    },
    Dup2X1: {
        opcode: 0x5D,
        args: [],
        // TODO: There is a second form of this that does operates on less stack args
        pop: [val3: ComplexType::Category1, val2: ComplexType::Category1, val1: ComplexType::Category1],
        push: [r1val2: ComplexType::Category1, r1val1: ComplexType::Category1, r1val3: ComplexType::Category1, r2val2: ComplexType::Category1, r2val1: ComplexType::Category1],
        exceptions: [],
    },

    FloatArrayStore: {
        opcode: 0x51,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Float),
            index: Int,
            value: Float,
        ],
        push: [],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index out of bound
            ArrayIndexOutOfBoundsException,
        ],
    },
    FloatArrayLoad: {
        opcode: 0x30,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Float),
            index: Int,
        ],
        push: [val: Float],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index out of bound
            ArrayIndexOutOfBoundsException,
        ],
    },

    DoubleArrayStore: {
        opcode: 0x52,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Double),
            index: Int,
            value: Double,
        ],
        push: [],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index out of bound
            ArrayIndexOutOfBoundsException,
        ],
    },
    DoubleArrayLoad: {
        opcode: 0x31,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Double),
            index: Int,
        ],
        push: [val: Double],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index out of bound
            ArrayIndexOutOfBoundsException,
        ],
    },

    ShortArrayStore: {
        opcode: 0x56,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Short),
            index: Int,
            value: Short,
        ],
        push: [],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index out of bound
            ArrayIndexOutOfBoundsException,
        ],
    },
    ShortArrayLoad: {
        opcode: 0x35,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Short),
            index: Int,
        ],
        push: [value: Short],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index out of bound
            ArrayIndexOutOfBoundsException,
        ],
    },

    /// Push -1
    IConstNeg1: {
        opcode: 0x2,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Push 0
    IntConst0: {
        opcode: 0x3,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Push 1
    IntConst1: {
        opcode: 0x4,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Push 2
    IntConst2: {
        opcode: 0x5,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Push 3
    IntConst3: {
        opcode: 0x6,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Push 4
    IntConst4: {
        opcode: 0x7,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Push 5
    IntConst5: {
        opcode: 0x8,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },

    IntLoad: {
        opcode: 0x15,
        args: [
            /// Index into local variable array
            index: UnsignedByte
        ],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    IntStore: {
        opcode: 0x36,
        args: [
            /// Index into local variable array
            index: UnsignedByte,
        ],
        pop: [value: Int],
        push: [],
        exceptions: [],
    },
    /// Store val to local variable at index 0
    IntStore0: {
        opcode: 0x3B,
        args: [],
        pop: [val: Int],
        push: [],
        exceptions: [],
    },
    /// Store val to local variable at index 1
    IntStore1: {
        opcode: 0x3C,
        args: [],
        pop: [val: Int],
        push: [],
        exceptions: [],
    },
    /// Store val to local variable at index 2
    IntStore2: {
        opcode: 0x3D,
        args: [],
        pop: [val: Int],
        push: [],
        exceptions: [],
    },
    /// Store val to local variable at index 3
    IntStore3: {
        opcode: 0x3E,
        args: [],
        pop: [val: Int],
        push: [],
        exceptions: [],
    },

    IntIncrement: {
        opcode: 0x84,
        args: [
            /// Index into local variable array
            index: UnsignedByte,
            /// The amount to increment by
            increment_amount: Byte,
        ],
        pop: [],
        push: [],
        exceptions: [],
    },

    /// val1 + val2
    IntAdd: {
        opcode: 0x60,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [res: Int],
        exceptions: [],
    },
    /// val1 - val2
    IntSubtract: {
        opcode: 0x64,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [res: Int],
        exceptions: [],
    },

    /// Load Int from array
    IntALoad: {
        opcode: 0x2E,
        args: [],
        pop: [arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Int), index: Int],
        push: [val: Int],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index out of bounds
            ArrayIndexOutOfBoundsException,
        ],
    },

    /// Loads local-var at index 0 as int
    IntLoad0: {
        opcode: 0x1A,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Loads local-var at index 1 as int
    IntLoad1: {
        opcode: 0x1B,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Loads local-var at index 2 as int
    IntLoad2: {
        opcode: 0x1C,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    /// Loads local-var at index 3 as int
    IntLoad3: {
        opcode: 0x1D,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },

    /// Load long from local variable at index, index+1
    LondLoad: {
        opcode: 0x16,
        args: [index: UnsignedByte],
        pop: [],
        push: [val: Long],
        exceptions: [],
    },
    /// Loads local-var at index 0 and 0+1 as a long
    LongLoad0: {
        opcode: 0x1E,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
    },
    /// Loads local-var at index 1 and 1+1 as a long
    LongLoad1: {
        opcode: 0x1F,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
    },
    /// Loads local-var at index 2 and 2+1 as a long
    LongLoad2: {
        opcode: 0x20,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
    },
    /// Loads local-var at index 3 and 3+1 as a long
    LongLoad3: {
        opcode: 0x21,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
    },
    /// Push 0 long
    LongConst0: {
        opcode: 0x9,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
    },
    /// Push 1 long
    LongConst1: {
        opcode: 0xA,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
    },
    /// Load double from local variable
    DoubleLoad: {
        opcode: 0x18,
        args: [index: UnsignedByte],
        pop: [],
        push: [res: Double],
        exceptions: [],
    },
    /// Load double from local variable at 0
    DoubleLoad0: {
        opcode: 0x26,
        args: [],
        pop: [],
        push: [val: Double],
        exceptions: [],
    },
    /// Load double from local variable at 1
    DoubleLoad1: {
        opcode: 0x27,
        args: [],
        pop: [],
        push: [val: Double],
        exceptions: [],
    },
    /// Load double from local variable at 2
    DoubleLoad2: {
        opcode: 0x28,
        args: [],
        pop: [],
        push: [val: Double],
        exceptions: [],
    },
    /// Load double from local variable at 3
    DoubleLoad3: {
        opcode: 0x29,
        args: [],
        pop: [],
        push: [val: Double],
        exceptions: [],
    },
    /// Store double into local variable at index
    DoubleStore: {
        opcode: 0x39,
        args: [index: UnsignedByte],
        pop: [val: Double],
        push: [],
        exceptions: [],
    },
    /// Store double into local variable at 0
    DoubleStore0: {
        opcode: 0x47,
        args: [],
        pop: [val: Double],
        push: [],
        exceptions: [],
    },
    /// Store double into local variable at 1
    DoubleStore1: {
        opcode: 0x48,
        args: [],
        pop: [val: Double],
        push: [],
        exceptions: [],
    },
    /// Store double into local variable at 2
    DoubleStore2: {
        opcode: 0x49,
        args: [],
        pop: [val: Double],
        push: [],
        exceptions: [],
    },
    /// Store double into local variable at 3
    DoubleStore3: {
        opcode: 0x4A,
        args: [],
        pop: [val: Double],
        push: [],
        exceptions: [],
    },

    DoubleConst0: {
        opcode: 0xE,
        args: [],
        pop: [],
        push: [res: Double],
        exceptions: [],
    },
    DoubleConst1: {
        opcode: 0xF,
        args: [],
        pop: [],
        push: [res: Double],
        exceptions: [],
    },

    DoubleMultiply: {
        opcode: 0x6B,
        args: [],
        pop: [val1: Double, val2: Double],
        push: [res: Double],
        exceptions: [],
    },
    DoubleAdd: {
        opcode: 0x63,
        args: [],
        pop: [val1: Double, val2: Double],
        push: [res: Double],
        exceptions: [],
    },
    DoubleSubtract: {
        opcode: 0x67,
        args: [],
        pop: [val1: Double, val2: Double],
        push: [res: Double],
        exceptions: [],
    },
    DoubleNegate: {
        opcode: 0x77,
        args: [],
        pop: [value: Double],
        push: [res: Double],
        exceptions: [],
    },
    DoubleDivide: {
        opcode: 0x6F,
        args: [],
        pop: [val1: Double, val2: Double],
        push: [res: Double],
        exceptions: [],
    },
    DoubleToInt: {
        opcode: 0x8E,
        args: [],
        pop: [val: Double],
        push: [res: Int],
        exceptions: [],
    },
    DoubleToLong: {
        opcode: 0x8F,
        args: [],
        pop: [val: Double],
        push: [res: Long],
        exceptions: [],
    },
    DoubleToFloat: {
        opcode: 0x90,
        args: [],
        pop: [val: Double],
        push: [res: Float],
        exceptions: [],
    },

    /// Compare two longs as signed integers
    /// If val1 > val2  -> push  1 (int)
    /// If val1 == val2 -> push  0 (int)
    /// if val1 < val2  -> push -1 (int)
    LongCmp: {
        opcode: 0x94,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Int],
        exceptions: [],
    },

    FloatNegate: {
        opcode: 0x76,
        args: [],
        pop: [val: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatAdd: {
        opcode: 0x62,
        args: [],
        pop: [val1: Float, val2: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatSub: {
        opcode: 0x66,
        args: [],
        pop: [val1: Float, val2: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatDivide: {
        opcode: 0x6E,
        args: [],
        pop: [val1: Float, val2: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatMultiply: {
        opcode: 0x6A,
        args: [],
        pop: [val1: Float, val2: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatLoad: {
        opcode: 0x17,
        args: [index: UnsignedByte],
        pop: [],
        push: [val: Float],
        exceptions: [],
    },
    /// Load float from local variable at 0
    FloatLoad0: {
        opcode: 0x22,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
    },
    /// Load float from local variable at 1
    FloatLoad1: {
        opcode: 0x23,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
    },
    /// Load float from local variable at 2
    FloatLoad2: {
        opcode: 0x24,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
    },
    /// Load float from local variable at 3
    FloatLoad3: {
        opcode: 0x25,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
    },
    /// Store float into local variable at 0
    FloatStore0: {
        opcode: 0x43,
        args: [],
        pop: [val: Float],
        push: [],
        exceptions: [],
    },
    /// Store float into local variable at index
    FloatStore: {
        opcode: 0x38,
        args: [index: UnsignedByte],
        pop: [val: Float],
        push: [],
        exceptions: [],
    },
    /// Store float into local variable at 1
    FloatStore1: {
        opcode: 0x44,
        args: [],
        pop: [val: Float],
        push: [],
        exceptions: [],
    },
    /// Store float into local variable at 2
    FloatStore2: {
        opcode: 0x45,
        args: [],
        pop: [val: Float],
        push: [],
        exceptions: [],
    },
    /// Store float into local variable at 3
    FloatStore3: {
        opcode: 0x46,
        args: [],
        pop: [val: Float],
        push: [],
        exceptions: [],
    },
    /// Push 0.0f
    FloatConst0: {
        opcode: 0xB,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
    },
    /// Push 1.0f
    FloatConst1: {
        opcode: 0xC,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
    },
    /// Push 2.0f
    FloatConst2: {
        opcode: 0xD,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
    },

    /// Compare two floats
    /// Undergoes value set conversion
    /// if val1 > val2 -> push 1 (int)
    /// if val1 == val2 -> push 0 (int)
    /// if val1 < val2 -> push -1
    /// If val1 is NaN || val2 is NaN -> push -1
    FloatCmpL: {
        opcode: 0x95,
        args: [],
        pop: [val1: Float, val2: Float],
        push: [res: Int],
        exceptions: [],
    },

    /// Compare two floats
    /// Undergoes value set conversion
    /// if val1 > val2 -> push 1 (int)
    /// if val1 == val2 -> push 0 (int)
    /// if val1 < val2 -> push -1
    /// If val1 is NaN || val2 is NaN -> push 1
    FloatCmpG: {
        opcode: 0x96,
        args: [],
        pop: [val1: Float, val2: Float],
        push: [res: Int],
        exceptions: [],
    },
    FloatToInt: {
        opcode: 0x8B,
        args: [],
        pop: [val1: Float],
        push: [res: Int],
        exceptions: [],
    },
    FloatToLong: {
        opcode: 0x8C,
        args: [],
        pop: [val: Float],
        push: [res: Long],
        exceptions: [],
    },
    FloatToDouble: {
        opcode: 0x8D,
        args: [],
        pop: [value: Float],
        push: [res: Double],
        exceptions: [],
    },

    DoubleCmpL: {
        opcode: 0x97,
        args: [],
        pop: [val1: Double, val2: Double],
        push: [res: Int],
        exceptions: [],
    },
    DoubleCmpG: {
        opcode: 0x98,
        args: [],
        pop: [val1: Double, val2: Double],
        push: [res: Int],
        exceptions: [],
    },

    LongAdd: {
        opcode: 0x61,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },

    /// Store a long into local-var at index 0
    LongStore0: {
        opcode: 0x3F,
        args: [],
        pop: [val: Long],
        push: [],
        exceptions: [],
    },
    /// Store a long into local-var at index 1
    LongStore1: {
        opcode: 0x40,
        args: [],
        pop: [val: Long],
        push: [],
        exceptions: [],
    },
    /// Store a long into local-var at index 2
    LongStore2: {
        opcode: 0x41,
        args: [],
        pop: [val: Long],
        push: [],
        exceptions: [],
    },
    /// Store a long into local-var at index 3
    LongStore3: {
        opcode: 0x42,
        args: [],
        pop: [val: Long],
        push: [],
        exceptions: [],
    },

    LongToFloat: {
        opcode: 0x89,
        args: [],
        pop: [val: Long],
        push: [res: Float],
        exceptions: [],
    },

    LongToDouble: {
        opcode: 0x8A,
        args: [],
        pop: [val: Long],
        push: [res: Double],
        exceptions: [],
    },

    PutField: {
        opcode: 0xB5,
        args: [
            index: ConstantPoolIndexRaw<FieldRefConstant>,
        ],
        pop: [
            /// Must not be an array
            objectref: ComplexType::ReferenceAny,
            // TODO: Depends upon method descriptor type
            value: ComplexType::Any,
        ],
        push: [],
        exceptions: [
            // TODO:
            /// If the resolved field is static
            IncompatibleClassChangeError,
            /// If the field is final, it must be declared in the current class
            /// and this must be within an instance init method
            IllegalAccessError,
            /// objectref was null
            NullPointerException,
        ],
    },
    GetField: {
        opcode: 0xB4,
        args: [
            index: ConstantPoolIndexRaw<FieldRefConstant>,
        ],
        pop: [
            objectref: ComplexType::ReferenceAny,
        ],
        push: [value: ComplexType::Any],
        exceptions: [
            // TODO:
            /// If resolved field is static
            IncompatibleClassChangeError,
            /// If objectref is null
            NullPointerException,
        ],
    },

    PutStaticField: {
        opcode: 0xB3,
        args: [
            index: ConstantPoolIndexRaw<FieldRefConstant>,
        ],
        pop: [value: ComplexType::Any],
        push: [],
        exceptions: [
            // TODO
            // Resolved field is not static field
            IncompatibleClassChangeError,
            /// If field is final, it has to be declared in current class and this must be ran in
            /// the clinit method. Otherwise:
            IllegalAccessError,
        ],
    },

    IntToDouble: {
        opcode: 0x87,
        args: [],
        pop: [value: Int],
        push: [res: Double],
        exceptions: [],
    },
    IntToLong: {
        opcode: 0x85,
        args: [],
        pop: [value: Int],
        push: [res: Long],
        exceptions: [],
    },
    IntToFloat: {
        opcode: 0x86,
        args: [],
        pop: [value: Int],
        push: [res: Float],
        exceptions: [],
    },
    IntToByte: {
        opcode: 0x91,
        args: [],
        pop: [val: Int],
        push: [res: Byte],
        exceptions: [],
    },
    IntToChar: {
        opcode: 0x92,
        args: [],
        pop: [value: Int],
        push: [res: Char],
        exceptions: [],
    },
    IntToShort: {
        opcode: 0x93,
        args: [],
        pop: [val: Int],
        push: [res: Short],
        exceptions: [],
    },

    /// Narrowing conversion. May not have the same sign.
    LongToInt: {
        opcode: 0x88,
        args: [],
        pop: [value: Long],
        push: [res: Int],
        exceptions: [],
    },

    /// Load a byte or boolean from an array
    ByteArrayLoad: {
        opcode: 0x33,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitiveOr(PrimitiveType::Byte, PrimitiveType::Boolean),
            index: Int,
        ],
        push: [
            value: Byte,
        ],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// index out of bounds
            ArrayIndexOutOfBoundsException,
        ],
    },
    ByteArrayStore: {
        opcode: 0x54,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayAnyPrimitive,
            index: Int,
            // TODO: more specific type
            value: Byte,
        ],
        push: [],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// index out of bounds
            ArrayIndexOutOfBoundsException,
        ],
    },
    CharArrayStore: {
        opcode: 0x55,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Char),
            index: Int,
            value: Char
        ],
        push: [],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index is out of bounds
            ArrayIndexOutOfBoundsException,
        ],
    },
    CharArrayLoad: {
        opcode: 0x34,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Char),
            index: Int,
        ],
        push: [val: Char],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// Index is out of bounds
            ArrayIndexOutOfBoundsException,
        ],
    },

    Pop: {
        opcode: 0x57,
        args: [],
        pop: [value: ComplexType::Category1],
        push: [],
        exceptions: [],
    },
    Pop2: {
        opcode: 0x58,
        args: [],
        // TODO: It could also just pop a single value if there is only one value
        pop: [val2: ComplexType::Category1, val1: ComplexType::Category1],
        push: [],
        exceptions: [],
    },

    /// Pushes sign extended byte to stack
    PushByte: {
        opcode: 0x10,
        args: [val: Byte],
        pop: [],
        // TODO: We could have some type to represent that it has to be the same as arg[0]
        push: [res: Byte],
        exceptions: [],
    },
    /// Pushes sign extended short to stack
    PushShort: {
        opcode: 0x11,
        args: [val: Short],
        pop: [],
        push: [res: Short],
        exceptions: [],
    },
    IntMultiply: {
        opcode: 0x68,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [val: Int],
        exceptions: [],
    },
    IntDivide: {
        opcode: 0x6C,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [val: Int],
        exceptions: [],
    },
    IntRemainder: {
        opcode: 0x70,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [res: Int],
        exceptions: [],
    },
    IntNegate: {
        opcode: 0x74,
        args: [],
        pop: [val: Int],
        push: [res: Int],
        exceptions: [],
    },
    IntAnd: {
        opcode: 0x7E,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [
            /// val1 & val2
            res: Int,
        ],
        exceptions: [],
    },
    IntOr: {
        opcode: 0x80,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [
            /// val1 | val2
            res: Int,
        ],
        exceptions: [],
    },
    IntXor: {
        opcode: 0x82,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [res: Int],
        exceptions: [],
    },
    IntShiftLeft: {
        opcode: 0x78,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [
            /// val1 << (val2 & 0b11111)
            res: Int,
        ],
        exceptions: [],
    },
    IntArithmeticShiftRight: {
        opcode: 0x7A,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [res: Int],
        exceptions: [],
    },
    IntLogicalShiftRight: {
        opcode: 0x7C,
        args: [],
        pop: [val1: Int, val2: Int],
        push: [res: Int],
        exceptions: [],
    },
    IntArrayStore: {
        opcode: 0x4F,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Int),
            index: Int,
            value: Int,
        ],
        push: [],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// index out of bounds
            ArrayIndexOutOfBoundsException,
        ],
    },

    LongSubtract: {
        opcode: 0x65,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongDivide: {
        opcode: 0x6D,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongMultiply: {
        opcode: 0x69,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongRemainder: {
        opcode: 0x71,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongNegate: {
        opcode: 0x75,
        args: [],
        pop: [val: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongAnd: {
        opcode: 0x7F,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongOr: {
        opcode: 0x81,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongLogicalShiftRight: {
        opcode: 0x7D,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongArithmeticShiftRight: {
        opcode: 0x7B,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongShiftLeft: {
        opcode: 0x79,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongXor: {
        opcode: 0x83,
        args: [],
        pop: [val1: Long, val2: Long],
        push: [res: Long],
        exceptions: [],
    },
    /// Store long into local variable at index, index+1
    LongStore: {
        opcode: 0x37,
        args: [index: UnsignedByte],
        pop: [val: Long],
        push: [],
        exceptions: [],
    },
    LongArrayStore: {
        opcode: 0x50,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Long),
            index: Int,
            value: Long,
        ],
        push: [],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// index out of bounds
            ArrayIndexOutOfBoundsException,
        ],
    },
    LongArrayLoad: {
        opcode: 0x2F,
        args: [],
        pop: [
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Long),
            index: Int,
        ],
        push: [res: Long],
        exceptions: [
            /// arrayref is null
            NullPointerException,
            /// index out of bounds
            ArrayIndexOutOfBoundsException,
        ],
    },

    MonitorEnter: {
        opcode: 0xC2,
        args: [],
        pop: [objectref: ComplexType::ReferenceAny],
        push: [],
        exceptions: [
            /// objectref is null
            NullPointerException,
        ],
    },
    MonitorExit: {
        opcode: 0xC3,
        args: [],
        pop: [Objectref: ComplexType::ReferenceAny],
        push: [],
        exceptions: [
            /// objectref is null
            NullPointerException,
            /// Thread executes monitorexit but is not the owner of the monitor
            IllegalMonitorStateException,
            /// If there is a violation of optional rules (2.11.10)
            IllegalMonitorStateException,
        ],
    },

    /// Check whether an object is a specific type
    CheckCast: {
        opcode: 0xC0,
        args: [
            // TODO: is this correct?
            index: ConstantPoolIndexRaw<ClassConstant>,
        ],
        pop: [
            objectref: ComplexType::ReferenceAny,
        ],
        push: [
            /// The same ref that was popped
            res_objectref: WithType::RefType(0),
        ],
        exceptions: [
            // TODO:
            /// If objectref can't cast
            ClassCastException,
        ],
    },
    /// Check if object is of a type
    InstanceOf: {
        opcode: 0xC1,
        args: [
            index: ConstantPoolIndexRaw<ClassConstant>,
        ],
        pop: [
            objectref: ComplexType::ReferenceAny,
        ],
        push: [
            /// 0 if objectref is null
            /// otherwise, if objecterf is an instance of the given class
            result: Int,
        ],
        exceptions: [],
    },

    Wide: {extern},
    LookupSwitch: {extern},
    TableSwitch: {extern},

],
WIDE_INSTR: [
    WideIntLoad: {
        opcode: IntLoad::OPCODE,
        args: [
            index: UnsignedShort,
        ],
        pop: [],
        push: [val: Int],
        exceptions: [],
    },
    WideIntIncrement: {
        opcode: IntIncrement::OPCODE,
        args: [
            /// Index into local variable array
            index: UnsignedShort,
            /// The amount to increment by
            increment_amount: UnsignedShort,
        ],
        pop: [],
        push: [],
        exceptions: [],
    },
]}

// Redeclaration so Rust analyzer picks up on it properly
pub type Inst = InstM;
pub type WideInst = WideInstM;

/// pop: [key: Int]
/// push: []
/// exceptions: []
#[derive(Debug, Clone)]
pub struct LookupSwitch {
    /// 0-3
    padding: u8,
    default: i32,
    pairs: Vec<LookupSwitchPair>,
}
impl LookupSwitch {
    pub const OPCODE: RawOpcode = 0xAB;

    pub(crate) fn parse(
        data: &[u8],
        mut idx: InstructionIndex,
    ) -> Result<LookupSwitch, InstructionParseError> {
        // skip over opcode
        idx.0 += 1;
        let data = &data[idx.0 as usize..];
        // 0 -> 0
        // 1 -> 4 - 1 -> 3
        // 2 -> 4 - 2 -> 2
        // 3 -> 4 - 3 -> 1
        // 4 -> 4 - 4 -> 0
        #[allow(clippy::cast_possible_truncation)]
        let padding = (idx.0 % 4) as u8;
        let padding = if padding == 0 { padding } else { 4 - padding };
        let data = &data[padding as usize..];
        let default = Int::parse(data);
        let data = &data[Int::MEMORY_SIZE..];
        let npairs = Int::parse(data);
        let mut data = &data[Int::MEMORY_SIZE..];
        let mut pairs = Vec::new();
        for _ in 0..npairs {
            let val = LookupSwitchPair::parse(data)?;
            data = &data[LookupSwitchPair::MEMORY_SIZE..];
            pairs.push(val);
        }
        // TODO: is this correct?
        Ok(Self {
            padding,
            default,
            pairs,
        })
    }

    #[must_use]
    pub fn pop_type_at(&self, i: usize) -> Option<Type> {
        if i == 0 {
            // key
            Some(PrimitiveType::Int.into())
        } else {
            None
        }
    }

    #[must_use]
    pub fn push_type_at(&self, _: usize) -> Option<Type> {
        None
    }
}
impl MemorySize for LookupSwitch {
    fn memory_size(&self) -> usize {
        1 + self.padding as usize
            // default
            + Int::MEMORY_SIZE
            // npairs
            + Int::MEMORY_SIZE
            + (self.pairs.len() * LookupSwitchPair::MEMORY_SIZE)
    }
}

#[derive(Debug, Clone)]
pub struct LookupSwitchPair {
    match_v: i32,
    offset: i32,
}
impl LookupSwitchPair {
    pub(crate) fn parse(data: &[u8]) -> Result<LookupSwitchPair, InstructionParseError> {
        let match_v = Int::parse(data);
        let data = &data[Int::MEMORY_SIZE..];
        let offset = Int::parse(data);
        Ok(Self { match_v, offset })
    }
}
impl StaticMemorySize for LookupSwitchPair {
    const MEMORY_SIZE: usize = Int::MEMORY_SIZE + Int::MEMORY_SIZE;
}

#[derive(Debug, Clone)]
pub struct TableSwitch {
    padding: u8,
    default: i32,
    low: i32,
    high: i32,
    jump_offsets: Vec<i32>,
}
impl TableSwitch {
    pub const OPCODE: RawOpcode = 0xAA;

    pub(crate) fn parse(
        data: &[u8],
        mut idx: InstructionIndex,
    ) -> Result<Self, InstructionParseError> {
        // skip over opcode
        idx.0 += 1;
        let data = &data[idx.0 as usize..];
        // 0 -> 0
        // 1 -> 4 - 1 -> 3
        // 2 -> 4 - 2 -> 2
        // 3 -> 4 - 3 -> 1
        // 4 -> 4 - 4 -> 0
        #[allow(clippy::cast_possible_truncation)]
        let padding = (idx.0 % 4) as u8;
        let padding = if padding == 0 { padding } else { 4 - padding };
        let data = &data[padding as usize..];
        let default = Int::parse(data);
        let data = &data[Int::MEMORY_SIZE..];
        let low = Int::parse(data);
        let data = &data[Int::MEMORY_SIZE..];
        let high = Int::parse(data);
        let mut data = &data[Int::MEMORY_SIZE..];
        let jump_table_count = high - low + 1;
        let mut jump_offsets = Vec::new();
        for _ in 0..jump_table_count {
            jump_offsets.push(Int::parse(data));
            data = &data[Int::MEMORY_SIZE..];
        }

        Ok(Self {
            padding,
            default,
            low,
            high,
            jump_offsets,
        })
    }

    #[must_use]
    pub fn pop_type_at(&self, i: usize) -> Option<Type> {
        if i == 0 {
            Some(PrimitiveType::Int.into())
        } else {
            None
        }
    }

    #[must_use]
    pub fn push_type_at(&self, _i: usize) -> Option<Type> {
        None
    }
}
impl MemorySize for TableSwitch {
    fn memory_size(&self) -> usize {
        1 + self.padding as usize
            + Int::MEMORY_SIZE
            + Int::MEMORY_SIZE
            + Int::MEMORY_SIZE
            + (self.jump_offsets.len() * Int::MEMORY_SIZE)
    }
}

#[derive(Debug, Clone)]
pub struct Wide(pub WideInstM);
impl Wide {
    pub const OPCODE: RawOpcode = 0xC4;

    pub(crate) fn parse(
        data: &[u8],
        mut idx: InstructionIndex,
    ) -> Result<Self, InstructionParseError> {
        // skip over opcode
        idx.0 += 1;
        Ok(Self(WideInstM::parse(data, idx)?))
    }

    // TODO: This is a bit ehh
    #[must_use]
    pub fn pop_type_at(&self, i: usize) -> Option<Type> {
        self.0.pop_type_at(i)
    }

    #[must_use]
    pub fn push_type_at(&self, i: usize) -> Option<Type> {
        self.0.push_type_at(i)
    }
}
impl MemorySize for Wide {
    fn memory_size(&self) -> usize {
        1 + self.0.memory_size()
    }
}

#[cfg(test)]
mod tests {
    use super::check_instruction_duplicates;

    #[test]
    fn test_ops() {
        check_instruction_duplicates();
    }
}
