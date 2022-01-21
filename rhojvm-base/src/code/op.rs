use classfile_parser::attribute_info::InstructionIndex;
use classfile_parser::constant_info::{
    ClassConstant, ConstantInfo, FieldRefConstant, InterfaceMethodRefConstant,
    InvokeDynamicConstant,
};
use classfile_parser::constant_pool::ConstantPoolIndexRaw;

use crate::class::ClassFileData;
use crate::code::op_ex::InstructionParseError;
use crate::code::types::{
    Byte, Char, ComplexType, Double, Float, HasStackInfo, Instruction, Int, LocalVariableInType,
    LocalVariableIndex, LocalVariableIndexByteType, LocalVariableIndexType, LocalVariableType,
    LocalsIn, LocalsOutAt, Long, ParseOutput, PopComplexType, PopIndex, PopType, PopTypeAt,
    PrimitiveType, PushIndex, PushType, PushTypeAt, Short, StackInfo, StackSizes, UnsignedByte,
    UnsignedShort, WithType,
};
use crate::util::{MemorySizeU16, StaticMemorySizeU16};
use crate::ClassNames;

use super::types::ConstantPoolIndexRawU8;

// TODO: Replace this with a count macro if that is ever added to Rust
macro_rules! replace_expr {
    ($_t:tt $sub:expr) => {
        $sub
    };
}

macro_rules! count_tts {
    ($($tts:tt)*) => {0 $(+ replace_expr!($tts 1))*};
}

macro_rules! define_pop {
    ($for:ident, [$(
        $(#[$pop_outer:meta])*
        $pop_name:ident : $pop_ty:expr
    ),* $(,)*]) => {
        impl PopTypeAt for $for {
            fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
                let pops = [$(PopType::from($pop_ty)),*];
                pops.into_iter().nth(i)
            }

            fn pop_count(&self) -> usize {
                let pops: &[PopType] = &[$(PopType::from($pop_ty)),*];
                pops.len()
            }
        }
        impl $for {
            #[must_use]
            pub fn pop_type_name_at(&self, i: PopIndex) -> Option<&'static str> {
                let pops = [$(stringify!($pop_name)),*];
                pops.into_iter().nth(i)
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
        impl PushTypeAt for $for {
            fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
                let push = [$(PushType::from($push_ty)),*];
                push.into_iter().nth(i)
            }

            fn push_count(&self) -> usize {
                let push: &[PushType] = &[$(PushType::from($push_ty)),*];
                push.len()
            }
        }
        impl $for {
            #[must_use]
            pub fn push_type_name_at(&self, i: PushIndex) -> Option<&'static str> {
                let push = [$(stringify!($push_name)),*];
                push.into_iter().nth(i)
            }
        }
    };
    ($for:ident, [{extern}]) => {};
}
macro_rules! define_locals_out {
    ($for:ident, [$(
        $(#[$local_outer:meta])*
        $local_name:ident ($local_index:expr): $local_ty:expr
    ),* $(,)*]) => {
        impl LocalsOutAt for $for {
            type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableType), {count_tts!($($local_name)*)}>;

            #[must_use]
            fn locals_out_type_iter(&self) -> Self::Iter {
                [$(($local_index, LocalVariableType::from($local_ty)))*].into_iter()
            }
        }
    };
    ($for:ident, [{extern}]) => {};
}
macro_rules! define_locals_in {
    ($for:ident, [$(
        $(#[$local_outer:meta])*
        ($local_index:expr, $local_ty:expr)
    ),* $(,)*]) => {
        impl LocalsIn for $for {
            type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableInType), {count_tts!($($local_ty)*)}>;

            #[must_use]
            fn locals_in_type_iter(&self) -> Self::Iter {
                [$(($local_index, LocalVariableInType::from($local_ty)))*].into_iter()
            }
        }
    };
    ($for:ident, [{extern}]) => {};
}

macro_rules! define_stack_info {
    ($for:ident, [extern]) => {};
    ($for:ident, []) => {
        impl StackInfo for $for {}
        impl HasStackInfo for $for {
            type Output = Self;
            fn stack_info(
                &self,
                _: &mut ClassNames,
                _: &ClassFileData,
                _: $crate::id::MethodId,
                _: StackSizes,
            ) -> Result<Self::Output, $crate::StepError> {
                Ok(self.clone())
            }
        }
    };
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
        $(stack_info: $stack_info_data:ident,)?
        $(locals_out: [$($locals_out_data:tt)*],)?
        $(locals_in: [$($locals_in_data:tt)*],)?
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
                    let needed_size: usize = $name::MEMORY_SIZE_U16 as usize;
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
                        let size = <$arg_ty>::MEMORY_SIZE_U16 as usize;
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
            impl Instruction for $name {
                fn name(&self) -> &'static str {
                    stringify!($name)
                }
            }
            impl $name {
                pub const OPCODE: RawOpcode = $opcode;
            }
            // The size of themselves in the code
            impl StaticMemorySizeU16 for $name {
                const MEMORY_SIZE_U16: u16 = 1 + $(<$arg_ty>::MEMORY_SIZE_U16 +)* 0;
            }
            define_pop!($name, [$($pop_data)*]);
            define_push!($name, [$($push_data)*]);
            define_locals_out!($name, [$($($locals_out_data)*)?]);
            define_locals_in!($name, [$($($locals_in_data)*)?]);
            define_stack_info!($name, [$($stack_info_data)?]);
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

                    assert!(!(rs == ls), "Duplicate opcode name!: '{}'", ls);

                    assert!(!(lo == ro), "Duplicate opcode!: '{}' and '{}' with {}", ls, rs, lo);
                }
            }
        }

        // pub enum StackInfosM {
        //     $(
        //         $name(<$name as HasStackInfo>::Output)
        //     ),+
        // }
        // impl StackInfo for StackInfosM {}
        // impl PopTypeAt for StackInfosM {
        //     fn pop_type_at(&self, i: usize) -> Option<PopType> {
        //         match self {
        //             $(
        //                 StackInfosM::$name(x) => x.pop_type_at(i),
        //             )+
        //             #[allow(unreachable_patterns)]
        //             _ => return None,
        //         }
        //     }

        //     fn pop_count(&self) -> usize {
        //         match self {
        //             $(
        //                 StackInfosM::$name(x) => x.pop_count(),
        //             )+
        //             #[allow(unreachable_patterns)]
        //             _ => unreachable!(),
        //         }
        //     }
        // }
        // impl PushTypeAt for StackInfosM {
        //     fn push_type_at(&self, i: usize) -> Option<PushType> {
        //         match self {
        //             $(
        //                 StackInfosM::$name(x) => x.push_type_at(i),
        //             )+
        //             #[allow(unreachable_patterns)]
        //             _ => return None,
        //         }
        //     }

        //     fn push_count(&self) -> usize {
        //         match self {
        //             $(
        //                 StackInfosM::$name(x) => x.push_count(),
        //             )+
        //             #[allow(unreachable_patterns)]
        //             _ => unreachable!(),
        //         }
        //     }
        // }

        // pub enum LocalsOutIter {
        //     $(
        //         $name(<<$name as HasStackInfo>::Output as LocalsOutAt>::Iter)
        //     ),+
        // }
        // impl Iterator for LocalsOutIter {
        //     type Item = (LocalVariableIndex, LocalVariableType);
        //     fn next(&mut self) -> Option<Self::Item> {
        //         match self {
        //             $(
        //                 LocalsOutIter::$name(x) => x.next(),
        //             )+
        //             #[allow(unreachable_patterns)]
        //             _ => unreachable!(),
        //         }
        //     }
        // }
        // impl LocalsOutAt for StackInfosM {
        //     type Iter = LocalsOutIter;

        //     fn locals_out_type_iter(&self) -> Self::Iter {
        //         match self {
        //             $(
        //                 StackInfosM::$name(x) => LocalsOutIter::$name(x.locals_out_type_iter()),
        //             )+
        //             #[allow(unreachable_patterns)]
        //             _ => unreachable!(),
        //         }
        //     }
        // }

        // pub enum LocalsInIter {
        //     $(
        //         $name(<<$name as HasStackInfo>::Output as LocalsIn>::Iter)
        //     ),+
        // }
        // impl Iterator for LocalsInIter {
        //     type Item = (LocalVariableIndex, LocalVariableInType);
        //     fn next(&mut self) -> Option<Self::Item> {
        //         match self {
        //             $(
        //                 LocalsInIter::$name(x) => x.next(),
        //             )+
        //             #[allow(unreachable_patterns)]
        //             _ => unreachable!(),
        //         }
        //     }
        // }
        // impl LocalsIn for StackInfosM {
        //     type Iter = LocalsInIter;

        //     fn locals_in_type_iter(&self) -> Self::Iter {
        //         match self {
        //             $(
        //                 StackInfosM::$name(x) => LocalsInIter::$name(x.locals_in_type_iter()),
        //             )+
        //             #[allow(unreachable_patterns)]
        //             _ => unreachable!(),
        //         }
        //     }
        // }

        pub trait InstMapFunc<'a> {
            type Output;

            fn call(self, inst: &'a impl Instruction) -> Self::Output;
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

            pub fn map<'a, F: InstMapFunc<'a>>(&'a self, f: F) -> F::Output {
                match self {
                    $(
                        InstM::$name(x) => f.call(x),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => unreachable!()
                }
            }
        // }
        // impl Instruction for InstM {
            #[must_use]
            pub fn name(&self) -> &'static str {
                match self {
                    $(
                        InstM::$name(inst) => inst.name(),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => unreachable!()
                }
            }
        }
        // impl HasStackInfo for InstM {
        //     type Output = StackInfosM;

        //     fn stack_info(&self,
        //         class_names: &mut ClassNames,
        //         class_file: &ClassFileData,
        //         method_id: $crate::id::MethodId,
        //         stack_sizes: StackSizes
        //     ) -> Result<StackInfosM, $crate::StepError> {
        //         Ok(match self {
        //             $(
        //                 InstM::$name(inst) => StackInfosM::$name(inst.stack_info(class_names, class_file, method_id, stack_sizes)?),
        //             )*
        //             #[allow(unreachable_patterns)]
        //             _ => unimplemented!(),
        //         })
        //     }
        // }
        impl MemorySizeU16 for InstM {
            fn memory_size_u16(&self) -> u16 {
                match self {
                    $(
                        InstM::$name(v) => v.memory_size_u16(),
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

        pub enum WideStackInfosM {
            $(
                $wide_name(<$wide_name as HasStackInfo>::Output)
            ),+
        }
        impl StackInfo for WideStackInfosM {}
        impl PopTypeAt for WideStackInfosM {
            fn pop_type_at(&self, i: usize) -> Option<PopType> {
                match self {
                    $(
                        WideStackInfosM::$wide_name(x) => x.pop_type_at(i),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => return None,
                }
            }

            fn pop_count(&self) -> usize {
                match self {
                    $(
                        WideStackInfosM::$wide_name(x) => x.pop_count(),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }
        impl PushTypeAt for WideStackInfosM {
            fn push_type_at(&self, i: usize) -> Option<PushType> {
                match self {
                    $(
                        WideStackInfosM::$wide_name(x) => x.push_type_at(i),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => return None,
                }
            }

            fn push_count(&self) -> usize {
                match self {
                    $(
                        WideStackInfosM::$wide_name(x) => x.push_count(),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }
        pub enum WideLocalsOutIter {
            $(
                $wide_name(<<$wide_name as HasStackInfo>::Output as LocalsOutAt>::Iter)
            ),+
        }
        impl Iterator for WideLocalsOutIter {
            type Item = (LocalVariableIndex, LocalVariableType);
            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $(
                        WideLocalsOutIter::$wide_name(x) => x.next(),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }
        impl LocalsOutAt for WideStackInfosM {
            type Iter = WideLocalsOutIter;

            fn locals_out_type_iter(&self) -> Self::Iter {
                match self {
                    $(
                        WideStackInfosM::$wide_name(x) => WideLocalsOutIter::$wide_name(x.locals_out_type_iter()),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }

        pub enum WideLocalsInIter {
            $(
                $wide_name(<<$wide_name as HasStackInfo>::Output as LocalsIn>::Iter)
            ),+
        }
        impl Iterator for WideLocalsInIter {
            type Item = (LocalVariableIndex, LocalVariableInType);
            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $(
                        WideLocalsInIter::$wide_name(x) => x.next(),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => unreachable!(),
                }
            }
        }
        impl LocalsIn for WideStackInfosM {
            type Iter = WideLocalsInIter;

            fn locals_in_type_iter(&self) -> Self::Iter {
                match self {
                    $(
                        WideStackInfosM::$wide_name(x) => WideLocalsInIter::$wide_name(x.locals_in_type_iter()),
                    )+
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
        }
        impl Instruction for WideInstM {
            fn name(&self) -> &'static str {
                match self {
                    $(
                        WideInstM::$wide_name(inst) => inst.name(),
                    )+
                    #[allow(unreachable_patterns)]
                    _ => unreachable!()
                }
            }
        }
        impl HasStackInfo for WideInstM {
            type Output = WideStackInfosM;

            fn stack_info(&self,
                class_names: &mut ClassNames,
                class_file: &ClassFileData,
                method_id: $crate::id::MethodId,
                stack_sizes: StackSizes
            ) -> Result<WideStackInfosM, $crate::StepError> {
                Ok(match self {
                    $(
                        WideInstM::$wide_name(inst) => WideStackInfosM::$wide_name(inst.stack_info(class_names, class_file, method_id, stack_sizes)?),
                    )*
                    #[allow(unreachable_patterns)]
                    _ => unimplemented!(),
                })
            }
        }
        impl MemorySizeU16 for WideInstM {
            fn memory_size_u16(&self) -> u16 {
                match self {
                    $(
                        WideInstM::$wide_name(v) => v.memory_size_u16(),
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
            index: WithType::IntArrayIndexInto(1),
            arrayref: PopComplexType::RefArrayRefAny,
        ],
        push: [
            res: WithType::RefArrayRefType(1),
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
            value: WithType::RefArrayRefType(2),
            index: WithType::IntArrayIndexInto(2),
            arrayref: PopComplexType::RefArrayRefAny,
            // TODO: there is technically more specific rules for this
            // that may need to be specifically supported
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
            index: LocalVariableIndexByteType,
        ],
        pop: [],
        push: [{extern}],
        exceptions: [],
        locals_in: [{extern}],
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
        locals_in: [(0, LocalVariableInType::ReferenceAny)],
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
        locals_in: [(1, LocalVariableInType::ReferenceAny)],
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
        locals_in: [(2, LocalVariableInType::ReferenceAny)],
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
        locals_in: [(3, LocalVariableInType::ReferenceAny)],
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
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // There's also the usual allocation failures
            /// If the class is not accessible
            IllegalAccessError,
            /// If count < 0
            NegativeArraySizeException,
        ],
        stack_info: extern,
        init: [{extern}],
    },
    /// Create a new multidimensional array
    MultiANewArray: {
        opcode: 0xC5,
        args: [
            index: ConstantPoolIndexRaw<ClassConstant>,
            dimensions: UnsignedByte,
        ],
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO:

            /// Current class does not have permission to access the type
            IllegalAccessError,

            /// If any of the count values are < 0
            NegativeArraySizeException,
        ],
        stack_info: extern,
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
            objectref: PopComplexType::ReferenceAny
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
        pop: [arrayref: PopComplexType::RefArrayAny],
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
            index: LocalVariableIndexByteType
        ],
        pop: [
            // TODO: extern type for returnAddress?
            /// Must be of type returnAddress | reference
            objectref: PopComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        locals_out: [{extern}],
        init: [inst; RequireValidLocalVariableIndex(inst.index as u16)],
    },
    /// Store reference into local variable 0
    AStore0: {
        opcode: 0x4B,
        args: [],
        pop: [
            /// Must be of type returnAddress | reference
            objectef: PopComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        locals_out: [
            loc (0): WithType::Type(0)
        ],
        init: [; RequireValidLocalVariableIndex(0)],
    },
    /// Store reference into local variable 1
    AStore1: {
        opcode: 0x4C,
        args: [],
        pop: [
            /// Must be of type returnAddress | reference
            objectef: PopComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        locals_out: [
            loc (1): WithType::Type(0)
        ],
        init: [; RequireValidLocalVariableIndex(1)],
    },
    /// Store reference into local variable 2
    AStore2: {
        opcode: 0x4D,
        args: [],
        pop: [
            /// Must be of type returnAddress | reference
            objectef: PopComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        locals_out: [
            loc (2): WithType::Type(0)
        ],
        init: [; RequireValidLocalVariableIndex(2)],
    },
    /// Store reference into local variable 3
    AStore3: {
        opcode: 0x4E,
        args: [],
        pop: [
            /// Must be of type returnAddress | reference
            objectef: PopComplexType::ReferenceAny,
        ],
        push: [],
        exceptions: [],
        locals_out: [
            loc (3): WithType::Type(0)
        ],
        init: [; RequireValidLocalVariableIndex(3)],
    },

    // Note: Invoke methods don't have a push value in the documentation, but the return
    // instructions push a value onto the callers operand stack, and so they *do* have some
    // pushed type

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
        push: [{extern}],
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
        stack_info: extern,
        init: [{extern}],
    },
    InvokeInterface: {
        opcode: 0xB9,
        args: [
            index: ConstantPoolIndexRaw<InterfaceMethodRefConstant>,
            count: UnsignedByte,
            zero: UnsignedByte,
        ],
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO
        ],
        stack_info: extern,
        init: [{extern}],
    },
    InvokeDynamic: {
        opcode: 0xBA,
        args: [
            index: ConstantPoolIndexRaw<InvokeDynamicConstant>,
            zero1: Byte,
            zero2: Byte,
        ],
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO: exceptions
        ],
        stack_info: extern,
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
        push: [{extern}],
        exceptions: [
            /// If field lookup fails
            NoSuchFieldError,
            /// Field lookup succeeds but the referenced field is not accessible
            IllegalMonitorStateException,
            /// If the resolved field is a not a static class field or an interface field
            IncompatibleClassChangeError,
            // TODO:
        ],
        stack_info: extern,
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
            index: ConstantPoolIndexRawU8<ConstantInfo>,
        ],
        pop: [],
        push: [{extern}],
        exceptions: [
            // TODO: This is incomplete
            /// If class is not accessible
            IllegalAccessError,
        ],
        stack_info: extern,
        init: [{extern}],
    },
    LoadConstantWide: {
        opcode: 0x13,
        args: [
            /// Index into constant pool of current class
            /// Must be an int | float | reference to string literal | symref to class |
            /// method type | method handle
            index: ConstantPoolIndexRaw<ConstantInfo>,
        ],
        pop: [],
        push: [{extern}],
        exceptions: [],
        stack_info: extern,
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
        push: [{extern}],
        exceptions: [],
        stack_info: extern,
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
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO: resolution errors
            /// Resolved to a interface or abstract class
            InstantiationError,
        ],
        stack_info: extern,
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
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO
        ],
        stack_info: extern,
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
        pop: [{extern}],
        push: [{extern}],
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
        stack_info: extern,
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
    IfIntCmpEq: {
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
    IfIntCmpNe: {
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
    IfIntCmpLt: {
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
    IfIntCmpGe: {
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
    IfIntCmpGt: {
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
    IfIntCmpLe: {
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
            value1: PopComplexType::ReferenceAny,
            value2: PopComplexType::ReferenceAny,
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
            value1: PopComplexType::ReferenceAny,
            value2: PopComplexType::ReferenceAny,
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
        pop: [val: PopComplexType::ReferenceAny],
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
        pop: [val: PopComplexType::ReferenceAny],
        push: [],
        exceptions: [],
        init: [inst; RequireValidCodeOffset(inst.branch_offset)],
    },

    /// Duplicate the top stack value
    Dup: {
        opcode: 0x59,
        args: [],
        pop: [val: PopType::Category1],
        push: [val: WithType::Type(0), val_dup: WithType::Type(0)],
        exceptions: [],
    },
    /// Duplicate the top two or one stack values
    /// Two category 1
    /// or one category two
    Dup2: {
        opcode: 0x5C,
        args: [],
        pop: [{extern}],
        push:[{extern}],
        exceptions: [],
        stack_info: extern,
    },
    /// Duplicate top stack value and inset the duplicate two values down
    /// pop v2
    /// pop v1
    /// push v1
    /// push v2
    /// push v1
    DupX1: {
        opcode: 0x5A,
        args: [],
        pop: [val1: PopType::Category1, val2: PopType::Category1],
        push: [val1_dup: WithType::Type(0), val2_n: WithType::Type(1), val1_n: WithType::Type(0)],
        exceptions: [],
    },
    DupX2: {
        opcode: 0x5B,
        args: [],
        pop: [{extern}],
        push: [{extern}],
        exceptions: [],
        stack_info: extern,
    },
    Dup2X1: {
        opcode: 0x5D,
        args: [],
        // TODO: There is a second form of this that does operates on less stack args
        pop: [{extern}],
        push: [{extern}],
        exceptions: [],
        stack_info: extern,
    },
    Dup2X2: {
        opcode: 0x5E,
        args: [],
        pop: [{extern}],
        push: [{extern}],
        exceptions: [],
        stack_info: extern,
    },

    FloatArrayStore: {
        opcode: 0x51,
        args: [],
        pop: [
            value: Float,
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Float),
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
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Float),
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
            value: Double,
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Double),
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
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Double),
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
            value: Short,
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Short),
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
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Short),
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
            index: LocalVariableIndexByteType
        ],
        pop: [],
        push: [val: Int],
        exceptions: [],
        locals_in: [{extern}],
    },
    IntStore: {
        opcode: 0x36,
        args: [
            /// Index into local variable array
            index: LocalVariableIndexByteType,
        ],
        pop: [value: Int],
        push: [],
        exceptions: [],
        locals_out: [{extern}],
    },
    /// Store val to local variable at index 0
    IntStore0: {
        opcode: 0x3B,
        args: [],
        pop: [val: Int],
        push: [],
        exceptions: [],
        locals_out: [
            loc (0): PrimitiveType::Int,
        ],
    },
    /// Store val to local variable at index 1
    IntStore1: {
        opcode: 0x3C,
        args: [],
        pop: [val: Int],
        push: [],
        exceptions: [],
        locals_out: [
            loc (1): PrimitiveType::Int,
        ],
    },
    /// Store val to local variable at index 2
    IntStore2: {
        opcode: 0x3D,
        args: [],
        pop: [val: Int],
        push: [],
        exceptions: [],
        locals_out: [
            loc (2): PrimitiveType::Int,
        ],
    },
    /// Store val to local variable at index 3
    IntStore3: {
        opcode: 0x3E,
        args: [],
        pop: [val: Int],
        push: [],
        exceptions: [],
        locals_out: [
            loc (3): PrimitiveType::Int,
        ],
    },

    IntIncrement: {
        opcode: 0x84,
        args: [
            /// Index into local variable array
            index: LocalVariableIndexByteType,
            /// The amount to increment by
            increment_amount: Byte,
        ],
        pop: [],
        push: [],
        exceptions: [],
        locals_in: [{extern}],
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
        pop: [
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Int),
        ],
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
        locals_in: [(0, PrimitiveType::Int)],
    },
    /// Loads local-var at index 1 as int
    IntLoad1: {
        opcode: 0x1B,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
        locals_in: [(1, PrimitiveType::Int)],
    },
    /// Loads local-var at index 2 as int
    IntLoad2: {
        opcode: 0x1C,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
        locals_in: [(2, PrimitiveType::Int)],
    },
    /// Loads local-var at index 3 as int
    IntLoad3: {
        opcode: 0x1D,
        args: [],
        pop: [],
        push: [val: Int],
        exceptions: [],
        locals_in: [(3, PrimitiveType::Int)],
    },

    /// Load long from local variable at index, index+1
    LongLoad: {
        opcode: 0x16,
        args: [index: LocalVariableIndexByteType],
        pop: [],
        push: [val: Long],
        exceptions: [],
        locals_in: [{extern}],
    },
    /// Loads local-var at index 0 and 0+1 as a long
    LongLoad0: {
        opcode: 0x1E,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
        locals_in: [(0, PrimitiveType::Long)],
    },
    /// Loads local-var at index 1 and 1+1 as a long
    LongLoad1: {
        opcode: 0x1F,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
        locals_in: [(1, PrimitiveType::Long)],
    },
    /// Loads local-var at index 2 and 2+1 as a long
    LongLoad2: {
        opcode: 0x20,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
        locals_in: [(2, PrimitiveType::Long)],
    },
    /// Loads local-var at index 3 and 3+1 as a long
    LongLoad3: {
        opcode: 0x21,
        args: [],
        pop: [],
        push: [val: Long],
        exceptions: [],
        locals_in: [(3, PrimitiveType::Long)],
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
        locals_in: [{extern}],
    },
    /// Load double from local variable at 0
    DoubleLoad0: {
        opcode: 0x26,
        args: [],
        pop: [],
        push: [val: Double],
        exceptions: [],
        locals_in: [(0, PrimitiveType::Double)],
    },
    /// Load double from local variable at 1
    DoubleLoad1: {
        opcode: 0x27,
        args: [],
        pop: [],
        push: [val: Double],
        exceptions: [],
        locals_in: [(1, PrimitiveType::Double)],
    },
    /// Load double from local variable at 2
    DoubleLoad2: {
        opcode: 0x28,
        args: [],
        pop: [],
        push: [val: Double],
        exceptions: [],
        locals_in: [(2, PrimitiveType::Double)],
    },
    /// Load double from local variable at 3
    DoubleLoad3: {
        opcode: 0x29,
        args: [],
        pop: [],
        push: [val: Double],
        exceptions: [],
        locals_in: [(3, PrimitiveType::Double)],
    },
    /// Store double into local variable at index
    DoubleStore: {
        opcode: 0x39,
        args: [index: UnsignedByte],
        pop: [val: Double],
        push: [],
        exceptions: [],
        locals_out: [{extern}],
    },
    /// Store double into local variable at 0
    DoubleStore0: {
        opcode: 0x47,
        args: [],
        pop: [val: Double],
        push: [],
        exceptions: [],
        locals_out: [
            loc (0): PrimitiveType::Double,
        ],
    },
    /// Store double into local variable at 1
    DoubleStore1: {
        opcode: 0x48,
        args: [],
        pop: [val: Double],
        push: [],
        exceptions: [],
        locals_out: [
            loc (1): PrimitiveType::Double,
        ],
    },
    /// Store double into local variable at 2
    DoubleStore2: {
        opcode: 0x49,
        args: [],
        pop: [val: Double],
        push: [],
        exceptions: [],
        locals_out: [
            loc (2): PrimitiveType::Double,
        ],
    },
    /// Store double into local variable at 3
    DoubleStore3: {
        opcode: 0x4A,
        args: [],
        pop: [val: Double],
        push: [],
        exceptions: [],
        locals_out: [
            loc (3): PrimitiveType::Double,
        ],
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
        pop: [val2: Double, val1: Double],
        push: [res: Double],
        exceptions: [],
    },
    DoubleAdd: {
        opcode: 0x63,
        args: [],
        pop: [val2: Double, val1: Double],
        push: [res: Double],
        exceptions: [],
    },
    DoubleSubtract: {
        opcode: 0x67,
        args: [],
        pop: [val2: Double, val1: Double],
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
        pop: [val2: Double, val1: Double],
        push: [res: Double],
        exceptions: [],
    },
    DoubleRemainder: {
        opcode: 0x73,
        args: [],
        pop: [val2: Double, val1: Double],
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
        pop: [val2: Long, val1: Long],
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
        pop: [val2: Float, val1: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatSub: {
        opcode: 0x66,
        args: [],
        pop: [val2: Float, val1: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatDivide: {
        opcode: 0x6E,
        args: [],
        pop: [val2: Float, val1: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatRemainder: {
        opcode: 0x72,
        args: [],
        pop: [val2: Float, val1: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatMultiply: {
        opcode: 0x6A,
        args: [],
        pop: [val2: Float, val1: Float],
        push: [res: Float],
        exceptions: [],
    },
    FloatLoad: {
        opcode: 0x17,
        args: [index: UnsignedByte],
        pop: [],
        push: [val: Float],
        exceptions: [],
        locals_in: [{extern}],
    },
    /// Load float from local variable at 0
    FloatLoad0: {
        opcode: 0x22,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
        locals_in: [(0, PrimitiveType::Float)],
    },
    /// Load float from local variable at 1
    FloatLoad1: {
        opcode: 0x23,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
        locals_in: [(1, PrimitiveType::Float)],
    },
    /// Load float from local variable at 2
    FloatLoad2: {
        opcode: 0x24,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
        locals_in: [(2, PrimitiveType::Float)],
    },
    /// Load float from local variable at 3
    FloatLoad3: {
        opcode: 0x25,
        args: [],
        pop: [],
        push: [res: Float],
        exceptions: [],
        locals_in: [(3, PrimitiveType::Float)],
    },
    /// Store float into local variable at index
    FloatStore: {
        opcode: 0x38,
        args: [index: UnsignedByte],
        pop: [val: Float],
        push: [],
        exceptions: [],
        locals_out: [{extern}],
    },
    /// Store float into local variable at 0
    FloatStore0: {
        opcode: 0x43,
        args: [],
        pop: [val: Float],
        push: [],
        exceptions: [],
        locals_out: [
            loc (0): PrimitiveType::Float,
        ],
    },
    /// Store float into local variable at 1
    FloatStore1: {
        opcode: 0x44,
        args: [],
        pop: [val: Float],
        push: [],
        exceptions: [],
        locals_out: [
            loc (1): PrimitiveType::Float,
        ],
    },
    /// Store float into local variable at 2
    FloatStore2: {
        opcode: 0x45,
        args: [],
        pop: [val: Float],
        push: [],
        exceptions: [],
        locals_out: [
            loc (2): PrimitiveType::Float,
        ],
    },
    /// Store float into local variable at 3
    FloatStore3: {
        opcode: 0x46,
        args: [],
        pop: [val: Float],
        push: [],
        exceptions: [],
        locals_out: [
            loc (3): PrimitiveType::Float,
        ],
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
        pop: [val2: Float, val1: Float],
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
        pop: [val2: Float, val1: Float],
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
        pop: [val2: Double, val1: Double],
        push: [res: Int],
        exceptions: [],
    },
    DoubleCmpG: {
        opcode: 0x98,
        args: [],
        pop: [val2: Double, val1: Double],
        push: [res: Int],
        exceptions: [],
    },

    LongAdd: {
        opcode: 0x61,
        args: [],
        pop: [val2: Long, val1: Long],
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
        locals_out: [{extern}],
    },
    /// Store a long into local-var at index 0
    LongStore0: {
        opcode: 0x3F,
        args: [],
        pop: [val: Long],
        push: [],
        exceptions: [],
        locals_out: [
            loc (0): PrimitiveType::Long,
        ],
    },
    /// Store a long into local-var at index 1
    LongStore1: {
        opcode: 0x40,
        args: [],
        pop: [val: Long],
        push: [],
        exceptions: [],
        locals_out: [
            loc (1): PrimitiveType::Long,
        ],
    },
    /// Store a long into local-var at index 2
    LongStore2: {
        opcode: 0x41,
        args: [],
        pop: [val: Long],
        push: [],
        exceptions: [],
        locals_out: [
            loc (2): PrimitiveType::Long,
        ],
    },
    /// Store a long into local-var at index 3
    LongStore3: {
        opcode: 0x42,
        args: [],
        pop: [val: Long],
        push: [],
        exceptions: [],
        locals_out: [
            loc (3): PrimitiveType::Long,
        ],
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
        pop: [{extern}],
        push: [{extern}],
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
        stack_info: extern,
    },
    GetField: {
        opcode: 0xB4,
        args: [
            index: ConstantPoolIndexRaw<FieldRefConstant>,
        ],
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO:
            /// If resolved field is static
            IncompatibleClassChangeError,
            /// If objectref is null
            NullPointerException,
        ],
        stack_info: extern,
    },

    PutStaticField: {
        opcode: 0xB3,
        args: [
            index: ConstantPoolIndexRaw<FieldRefConstant>,
        ],
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO
            // Resolved field is not static field
            IncompatibleClassChangeError,
            /// If field is final, it has to be declared in current class and this must be ran in
            /// the clinit method. Otherwise:
            IllegalAccessError,
        ],
        stack_info: extern,
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
            index: Int,
            arrayref: PopComplexType::RefArrayPrimitiveOr(PrimitiveType::Byte, PrimitiveType::Boolean),
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
            value: Byte,
            index: Int,
            arrayref: PopComplexType::RefArrayPrimitiveOr(PrimitiveType::Byte, PrimitiveType::Boolean),
            // TODO: more specific type
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
            value: Char,
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Char),
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
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Char),
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
        pop: [value: PopType::Category1],
        push: [],
        exceptions: [],
    },
    Pop2: {
        opcode: 0x58,
        args: [],
        pop: [{extern}],
        push: [],
        exceptions: [],
        stack_info: extern,
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
        pop: [val2: Int, val1: Int],
        push: [val: Int],
        exceptions: [],
    },
    IntDivide: {
        opcode: 0x6C,
        args: [],
        pop: [val2: Int, val1: Int],
        push: [val: Int],
        exceptions: [],
    },
    IntRemainder: {
        opcode: 0x70,
        args: [],
        pop: [val2: Int, val1: Int],
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
        pop: [val2: Int, val1: Int],
        push: [
            /// val1 & val2
            res: Int,
        ],
        exceptions: [],
    },
    IntOr: {
        opcode: 0x80,
        args: [],
        pop: [val2: Int, val1: Int],
        push: [
            /// val1 | val2
            res: Int,
        ],
        exceptions: [],
    },
    IntXor: {
        opcode: 0x82,
        args: [],
        pop: [val2: Int, val1: Int],
        push: [res: Int],
        exceptions: [],
    },
    IntShiftLeft: {
        opcode: 0x78,
        args: [],
        pop: [val2: Int, val1: Int],
        push: [
            /// val1 << (val2 & 0b11111)
            res: Int,
        ],
        exceptions: [],
    },
    IntArithmeticShiftRight: {
        opcode: 0x7A,
        args: [],
        pop: [val2: Int, val1: Int],
        push: [res: Int],
        exceptions: [],
    },
    IntLogicalShiftRight: {
        opcode: 0x7C,
        args: [],
        pop: [val2: Int, val1: Int],
        push: [res: Int],
        exceptions: [],
    },
    IntArrayStore: {
        opcode: 0x4F,
        args: [],
        pop: [
            value: Int,
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Int),
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
        pop: [val2: Long, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongDivide: {
        opcode: 0x6D,
        args: [],
        pop: [val2: Long, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongMultiply: {
        opcode: 0x69,
        args: [],
        pop: [val2: Long, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongRemainder: {
        opcode: 0x71,
        args: [],
        pop: [val2: Long, val1: Long],
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
        pop: [val2: Long, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongOr: {
        opcode: 0x81,
        args: [],
        pop: [val2: Long, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongLogicalShiftRight: {
        opcode: 0x7D,
        args: [],
        pop: [val2: Int, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongArithmeticShiftRight: {
        opcode: 0x7B,
        args: [],
        pop: [val2: Int, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongShiftLeft: {
        opcode: 0x79,
        args: [],
        pop: [val2: Int, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongXor: {
        opcode: 0x83,
        args: [],
        pop: [val2: Long, val1: Long],
        push: [res: Long],
        exceptions: [],
    },
    LongArrayStore: {
        opcode: 0x50,
        args: [],
        pop: [
            value: Long,
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Long),
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
            index: Int,
            arrayref: ComplexType::RefArrayPrimitive(PrimitiveType::Long),
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
        pop: [objectref: PopComplexType::ReferenceAny],
        push: [],
        exceptions: [
            /// objectref is null
            NullPointerException,
        ],
    },
    MonitorExit: {
        opcode: 0xC3,
        args: [],
        pop: [objectref: PopComplexType::ReferenceAny],
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
        pop: [{extern}],
        push: [{extern}],
        exceptions: [
            // TODO:
            /// If objectref can't cast
            ClassCastException,
        ],
        stack_info: extern,
    },
    /// Check if object is of a type
    InstanceOf: {
        opcode: 0xC1,
        args: [
            index: ConstantPoolIndexRaw<ClassConstant>,
        ],
        pop: [
            objectref: PopComplexType::ReferenceAny,
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
            index: LocalVariableIndexType,
        ],
        pop: [],
        push: [val: Int],
        exceptions: [],
        locals_in: [{extern}],
    },
    WideIntIncrement: {
        opcode: IntIncrement::OPCODE,
        args: [
            /// Index into local variable array
            index: LocalVariableIndexType,
            /// The amount to increment by
            increment_amount: UnsignedShort,
        ],
        pop: [],
        push: [],
        exceptions: [],
        locals_in: [{extern}],
    },
]}

macro_rules! self_sinfo {
    ($for:ty) => {
        impl StackInfo for $for {}
        impl HasStackInfo for $for {
            type Output = Self;
            fn stack_info(
                &self,
                _: &mut ClassNames,
                _: &ClassFileData,
                _: $crate::id::MethodId,
                _: StackSizes,
            ) -> Result<Self::Output, $crate::StepError> {
                Ok(self.clone())
            }
        }
    };
}

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
        let data = &data[Int::MEMORY_SIZE_U16 as usize..];
        let npairs = Int::parse(data);
        let mut data = &data[Int::MEMORY_SIZE_U16 as usize..];
        let mut pairs = Vec::new();
        for _ in 0..npairs {
            let val = LookupSwitchPair::parse(data)?;
            data = &data[LookupSwitchPair::MEMORY_SIZE_U16 as usize..];
            pairs.push(val);
        }
        // TODO: is this correct?
        Ok(Self {
            padding,
            default,
            pairs,
        })
    }
}
impl Instruction for LookupSwitch {
    fn name(&self) -> &'static str {
        "LookupSwitch"
    }
}
impl PopTypeAt for LookupSwitch {
    fn pop_type_at(&self, i: usize) -> Option<PopType> {
        if i == 0 {
            // key
            Some(PrimitiveType::Int.into())
        } else {
            None
        }
    }

    fn pop_count(&self) -> usize {
        1
    }
}
impl PushTypeAt for LookupSwitch {
    fn push_type_at(&self, _: usize) -> Option<PushType> {
        None
    }

    fn push_count(&self) -> usize {
        0
    }
}
define_locals_out!(LookupSwitch, []);
define_locals_in!(LookupSwitch, []);
self_sinfo!(LookupSwitch);
impl MemorySizeU16 for LookupSwitch {
    #[allow(clippy::cast_possible_truncation)]
    fn memory_size_u16(&self) -> u16 {
        1 + u16::from(self.padding)
            // default
            + Int::MEMORY_SIZE_U16
            // npairs
            + Int::MEMORY_SIZE_U16
            + (self.pairs.len() as u16 * LookupSwitchPair::MEMORY_SIZE_U16)
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
        let data = &data[Int::MEMORY_SIZE_U16 as usize..];
        let offset = Int::parse(data);
        Ok(Self { match_v, offset })
    }
}
impl StaticMemorySizeU16 for LookupSwitchPair {
    const MEMORY_SIZE_U16: u16 = Int::MEMORY_SIZE_U16 + Int::MEMORY_SIZE_U16;
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
        let data = &data[Int::MEMORY_SIZE_U16 as usize..];
        let low = Int::parse(data);
        let data = &data[Int::MEMORY_SIZE_U16 as usize..];
        let high = Int::parse(data);
        let mut data = &data[Int::MEMORY_SIZE_U16 as usize..];
        let jump_table_count = high - low + 1;
        let mut jump_offsets = Vec::new();
        for _ in 0..jump_table_count {
            jump_offsets.push(Int::parse(data));
            data = &data[Int::MEMORY_SIZE_U16 as usize..];
        }

        Ok(Self {
            padding,
            default,
            low,
            high,
            jump_offsets,
        })
    }
}
impl Instruction for TableSwitch {
    fn name(&self) -> &'static str {
        "TableSwitch"
    }
}
impl PopTypeAt for TableSwitch {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        if i == 0 {
            Some(PrimitiveType::Int.into())
        } else {
            None
        }
    }

    fn pop_count(&self) -> usize {
        1
    }
}
impl PushTypeAt for TableSwitch {
    fn push_type_at(&self, _i: PushIndex) -> Option<PushType> {
        None
    }

    fn push_count(&self) -> usize {
        0
    }
}
define_locals_out!(TableSwitch, []);
define_locals_in!(TableSwitch, []);
self_sinfo!(TableSwitch);
impl MemorySizeU16 for TableSwitch {
    #[allow(clippy::cast_possible_truncation)]
    fn memory_size_u16(&self) -> u16 {
        1 + u16::from(self.padding)
            + Int::MEMORY_SIZE_U16
            + Int::MEMORY_SIZE_U16
            + Int::MEMORY_SIZE_U16
            + (self.jump_offsets.len() as u16 * Int::MEMORY_SIZE_U16)
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
}
impl Instruction for Wide {
    fn name(&self) -> &'static str {
        self.0.name()
    }
}
impl HasStackInfo for Wide {
    type Output = WideStackInfosM;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
        method_id: crate::id::MethodId,
        stack_sizes: StackSizes,
    ) -> Result<WideStackInfosM, crate::StepError> {
        self.0
            .stack_info(class_names, class_file, method_id, stack_sizes)
    }
}
impl MemorySizeU16 for Wide {
    fn memory_size_u16(&self) -> u16 {
        1 + self.0.memory_size_u16()
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
