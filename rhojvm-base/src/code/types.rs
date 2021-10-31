//! Types specific to operations/instructions
//! These attempt to allow detailed specification of types and bheavior without
//! overly complex logic.

use std::ops::Range;

use classfile_parser::{constant_info::ClassConstant, constant_pool::ConstantPoolIndexRaw};

use crate::util::{MemorySize, StaticMemorySize};

// TODO: The character would be stored in java's modified utf8, so parsing that would be useful
// The classfile lib already does this.
pub struct JavaChar(pub [u8; 4]);

/// Internal
pub trait ParseOutput {
    type Output;
    fn parse(d: &[u8]) -> Self::Output;
}
macro_rules! create_primitive_types {
    ([
        $($name:ident = $mem_size:expr; $d:ident -> $parse_t:ty $parse:block),* $(,)*
    ]) => {
        $(
            /// Internal
            #[derive(Debug, Clone, Copy)]
            pub struct $name;
            impl ParseOutput for $name {
                type Output = $parse_t;
                /// Param Assured to be the same size as [`Self::MEMORY_SIZE`]
                fn parse($d: &[u8]) -> $parse_t {
                    $parse
                }
            }
            impl StaticMemorySize for $name {
                const MEMORY_SIZE: usize = $mem_size;
            }
            impl From<$name> for PrimitiveType {
                fn from(_: $name) -> PrimitiveType {
                    PrimitiveType::$name
                }
            }
            impl From<$name> for Type {
                fn from(v: $name) -> Type {
                    Type::from(PrimitiveType::from(v))
                }
            }
        )*

        #[derive(Debug, Clone, Copy)]
        pub enum PrimitiveTypeM {
            $(
                $name,
            )*
        }
        impl MemorySize for PrimitiveTypeM {
            fn memory_size(&self) -> usize {
                match self {
                    $(
                        Self::$name => $name::MEMORY_SIZE,
                    )*
                }
            }
        }
    };
}
create_primitive_types!([
    // TODO: Is casting like this correct?
    Byte = 1; d -> i8 { i8::from_be_bytes([d[0]]) },
    UnsignedByte = 1; d -> u8 { d[0] },
    Short = 2; d -> i16 { i16::from_be_bytes([d[0], d[1]]) },
    UnsignedShort = 2; d -> u16 { u16::from_be_bytes([d[0], d[1]]) },
    Int = 4; d -> i32 { i32::from_be_bytes([d[0], d[1], d[2], d[3]]) },
    Long = 8; d -> i64 { i64::from_be_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]) },
    Float = 4; d -> f32 { f32::from_be_bytes([d[0], d[1], d[2], d[3]]) },
    Double = 8; d -> f64 { f64::from_be_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]) },
    // TODO: How large is char?
    Char = 1; d -> JavaChar { JavaChar([d[0], d[1], d[2], d[3]]) },
    Boolean = 1; d -> bool { !(d[0] == 0) },
]);
// Redeclaration so RA can consistently see it
pub type PrimitiveType = PrimitiveTypeM;
impl<T> From<ConstantPoolIndexRaw<T>> for PrimitiveType {
    fn from(_: ConstantPoolIndexRaw<T>) -> PrimitiveType {
        PrimitiveType::UnsignedShort
    }
}
impl<T> ParseOutput for ConstantPoolIndexRaw<T> {
    type Output = ConstantPoolIndexRaw<T>;
    fn parse(d: &[u8]) -> Self::Output {
        let v = u16::from_be_bytes([d[0], d[1]]);
        ConstantPoolIndexRaw::new(v)
    }
}

pub type LocalVariableIndex = UnsignedShort;
pub type LocalVariableIndexByte = UnsignedByte;

#[derive(Debug, Clone)]
pub enum ComplexType {
    RefArrayPrimitive(PrimitiveType),
    /// It could be an array of either type, but only one
    RefArrayPrimitiveOr(PrimitiveType, PrimitiveType),
    /// An array of references to a type, this reduces one level of boxing
    RefArrayReferences(Box<ComplexType>),
    RefArrayAnyPrimitive,
    /// An array of any reference type
    ArrayRefAny,
    /// A reference to an array of references to any type
    RefArrayRefAny,
    /// A reference to an array containing any type
    RefArrayAny,
    // TODO: are you able to recursively reference?
    /// This can only hold a complex argument type, because you can't reference a primitive
    Reference(Box<ComplexType>),
    /// A reference to any type
    ReferenceAny,
    ReferenceNull,
    /// 4 byte and lower primtives/ref/returnaddr essentially.
    /// Not a long or double.
    Category1,
    /// A type that is just sized as a category 1, but might be a part of a long/double
    Category1Sized,
    /// 8 byte type, long or double
    Category2,
    Any,
}
impl ComplexType {
    #[must_use]
    pub fn reference(to: ComplexType) -> Self {
        Self::Reference(Box::new(to))
    }
}

pub type PopIndex = usize;
pub type PushIndex = usize;
/// Technically u8 or u16, and thus should be careful
pub type ArgIndex = usize;

#[derive(Debug, Clone)]
pub enum WithType {
    /// &T at pop index -> &T
    /// Inherits null if pop is null
    RefType(PopIndex),
    /// The type that is held in a reference to an array of references
    /// T in &[&T] (so for objects, this would still be a reference?)
    RefArrayRefType(PopIndex),

    RefArrayRefFromIndexLen {
        /// The index of the class that it holds references of
        index: ConstantPoolIndexRaw<ClassConstant>,
        /// The index to a specific pop value that decides the length of the array
        len_idx: PopIndex,
        // TODO: Should this be a separate variant?
        /// Whether they are all null at first
        is_all_null: bool,
    },

    RefArrayPrimitiveLen {
        /// The type of elements that the array holds
        element_type: PrimitiveType,
        /// The index of its length
        len_idx: PopIndex,
        /// Whether or not the values are initialized to their default value
        is_default_init: bool,
    },

    RefMultiDimArrayRefFromIndexLengthsRange {
        /// The index of the class that it holds references of (down the chain)
        index: ConstantPoolIndexRaw<ClassConstant>,
        /// The range of pop indices that decides the length of the dimensions of the array
        /// The amount of values in this is equivalent to the number of dimensions
        len_idxs: Range<PopIndex>,
        /// Whether all the base (at the bottom part of the multiple dimensions array)
        /// are default
        is_all_base_default: bool,
    },

    /// At the index (unsigned byte or unsigned short) at the pop index, there is a local variable
    /// and it is a reference
    LocalVariableRefAt(PopIndex),

    /// At the index (unsigned byte or unsigned short) at the pop index, there is a local variable
    /// and it is a reference, and must be not be a return address
    LocalVariableRefAtNoRetAddr(PopIndex),

    /// A local variable that holds a reference at a very specific index
    LocalVariableRefAtIndex(<LocalVariableIndex as ParseOutput>::Output),

    /// A local variables that holds a reference at a very specific index
    /// and must be not be a return address
    LocalVariableRefAtIndexNoRetAddr(<LocalVariableIndex as ParseOutput>::Output),

    /// A reference to a type that is an instance of the given class name or an instance of a class
    /// that extends class name
    RefClassOf {
        class_name: &'static [&'static str],
        can_be_null: bool,
    },

    /// This is an int that is an index into arrayref
    IntArrayIndexInto(PopIndex),

    LiteralInt(i32),
}

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(PrimitiveType),
    Complex(ComplexType),
    With(WithType),
}
impl From<PrimitiveType> for Type {
    fn from(v: PrimitiveType) -> Self {
        Self::Primitive(v)
    }
}
impl From<ComplexType> for Type {
    fn from(v: ComplexType) -> Self {
        Self::Complex(v)
    }
}
impl From<WithType> for Type {
    fn from(v: WithType) -> Self {
        Self::With(v)
    }
}
