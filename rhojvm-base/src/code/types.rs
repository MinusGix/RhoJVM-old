//! Types specific to operations/instructions
//! These attempt to allow detailed specification of types and bheavior without
//! overly complex logic.

use std::{marker::PhantomData, num::NonZeroUsize};

use classfile_parser::{constant_info::ConstantInfo, constant_pool::ConstantPoolIndexRaw};

use crate::{
    class::ClassFileData,
    id::{ClassId, MethodId},
    util::{MemorySizeU16, StaticMemorySizeU16},
    ClassNames, StepError,
};

use super::method::{DescriptorType, DescriptorTypeBasic};

// TODO: The character would be stored in java's modified utf8, so parsing that would be useful
// The classfile lib already does this.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct JavaChar(pub u16);
impl JavaChar {
    #[must_use]
    pub fn from_int(v: i32) -> JavaChar {
        // TODO: Is this correct?
        let b = v.to_be_bytes();
        JavaChar(u16::from_be_bytes([b[0], b[1]]))
    }

    #[must_use]
    pub fn as_int(&self) -> i32 {
        self.0.into()
    }
}

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
            impl StaticMemorySizeU16 for $name {
                const MEMORY_SIZE_U16: u16 = $mem_size;
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
            impl From<$name> for PopType {
                fn from(v: $name) -> PopType {
                    PopType::Type(Type::from(PrimitiveType::from(v)))
                }
            }
            impl From<$name> for PushType {
                fn from(v: $name) -> PushType {
                    PushType::Type(Type::from(PrimitiveType::from(v)))
                }
            }
        )*

        #[derive(Debug, Clone, Copy)]
        pub enum PrimitiveTypeM {
            $(
                $name,
            )*
        }
        impl MemorySizeU16 for PrimitiveTypeM {
            fn memory_size_u16(&self) -> u16 {
                match self {
                    $(
                        Self::$name => $name::MEMORY_SIZE_U16,
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
    Char = 2; d -> JavaChar { JavaChar(u16::from_be_bytes([d[0], d[1]])) },
    Boolean = 1; d -> bool { d[0] != 0 },
]);
impl PrimitiveTypeM {
    #[must_use]
    pub fn is_category_2(&self) -> bool {
        matches!(self, PrimitiveType::Double | PrimitiveType::Long)
    }

    #[must_use]
    pub fn as_desc_prefix(&self) -> &'static [u8] {
        match self {
            PrimitiveTypeM::Byte | PrimitiveTypeM::UnsignedByte => b"B",
            PrimitiveTypeM::Short | PrimitiveTypeM::UnsignedShort => b"S",
            PrimitiveTypeM::Int => b"I",
            PrimitiveTypeM::Long => b"J",
            PrimitiveTypeM::Float => b"F",
            PrimitiveTypeM::Double => b"D",
            PrimitiveTypeM::Char => b"C",
            PrimitiveTypeM::Boolean => b"Z",
        }
    }

    #[must_use]
    pub fn is_same_type_on_stack(&self, right: &PrimitiveType) -> bool {
        #![allow(clippy::match_like_matches_macro)]
        match (self, right) {
            // TODO: is this correct?
            (
                PrimitiveType::Byte
                | PrimitiveType::UnsignedByte
                | PrimitiveType::Short
                | PrimitiveType::UnsignedShort
                | PrimitiveType::Int
                | PrimitiveType::Char
                | PrimitiveType::Boolean,
                PrimitiveType::Byte
                | PrimitiveType::UnsignedByte
                | PrimitiveType::Short
                | PrimitiveType::UnsignedShort
                | PrimitiveType::Int
                | PrimitiveType::Char
                | PrimitiveType::Boolean,
            ) => true,
            (PrimitiveType::Float, PrimitiveType::Float) => true,
            (PrimitiveType::Long, PrimitiveType::Long) => true,
            (PrimitiveType::Double, PrimitiveType::Double) => true,
            _ => false,
        }
    }
}
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

/// Used for [`ConstantPoolIndexRaw`]s that are are stored as a single byte
/// Parses as that, rather than itself
pub struct ConstantPoolIndexRawU8<T>(PhantomData<*const T>);
impl<T> StaticMemorySizeU16 for ConstantPoolIndexRawU8<T> {
    const MEMORY_SIZE_U16: u16 = u8::MEMORY_SIZE_U16;
}
impl<T> ParseOutput for ConstantPoolIndexRawU8<T> {
    type Output = ConstantPoolIndexRaw<T>;
    fn parse(d: &[u8]) -> Self::Output {
        let v = u8::from_be_bytes([d[0]]);
        ConstantPoolIndexRaw::new(u16::from(v))
    }
}

pub type LocalVariableIndexType = UnsignedShort;
pub type LocalVariableIndex = <LocalVariableIndexType as ParseOutput>::Output;
pub type LocalVariableIndexByteType = UnsignedByte;
pub type LocalVariableIndexByte = <LocalVariableIndexByteType as ParseOutput>::Output;

#[derive(Debug, Clone)]
pub enum ComplexType {
    RefArrayPrimitive(PrimitiveType),
    /// An array of known levels that eventually bottoms out as instances of the given primitive
    /// type
    RefArrayPrimitiveLevels {
        level: NonZeroUsize,
        primitive: PrimitiveType,
    },
    /// An array of known levels that eventually bottoms out as instances of the given class id
    RefArrayLevels {
        level: NonZeroUsize,
        class_id: ClassId,
    },
    /// A reference to some specific instance of a class with this id
    ReferenceClass(ClassId),
    ReferenceNull,
}

#[derive(Debug, Clone)]
pub enum PopComplexType {
    /// A reference to an array of references to any type
    RefArrayRefAny,
    /// A reference to an array containing any type
    RefArrayAny,
    /// A reference to any type
    ReferenceAny,
    /// It could be an array of either type, but only one
    RefArrayPrimitiveOr(PrimitiveType, PrimitiveType),
}

pub type PopIndex = usize;
pub type PushIndex = usize;
/// Technically u8 or u16, and thus should be careful
pub type ArgIndex = usize;

#[derive(Debug, Clone)]
pub enum WithType {
    /// The type at the given pop index
    /// Obviously, this should not refer to itself
    Type(PopIndex),
    /// The type that is held in a reference to an array of references
    /// T in &[&T] (so for objects, this would still be a reference?)
    RefArrayRefType(PopIndex),

    RefArrayPrimitiveLen {
        /// The type of elements that the array holds
        element_type: PrimitiveType,
        /// The index of its length
        len_idx: PopIndex,
        /// Whether or not the values are initialized to their default value
        is_default_init: bool,
    },

    /// A local variables that holds a reference at a very specific index
    /// and must be not be a return address
    LocalVariableRefAtIndexNoRetAddr(LocalVariableIndex),

    /// A reference to a type that is an instance of the given class name or an instance of a class
    /// that extends class name
    RefClassOf {
        class_name: &'static [u8],
        can_be_null: bool,
    },

    /// This is an int that is an index into arrayref
    IntArrayIndexInto(PopIndex),

    LiteralInt(i32),
}

/// [`PushType`] and [`PopType`] are separate to provide more guarantees about what
/// info can appear.
/// This has the Category-size types because they can be turned into solid
/// types by glimpsing at the stack.
/// A push type could not manage that with a category sized type but can refer
/// to a pop type.
#[derive(Debug, Clone)]
pub enum PopType {
    /// 4 byte and lower primtives/ref/returnaddr essentially.
    /// Not a long or double.
    Category1,
    /// 8 byte type, long or double
    Category2,
    Type(Type),
    Complex(PopComplexType),
}
impl From<Type> for PopType {
    fn from(typ: Type) -> PopType {
        PopType::Type(typ)
    }
}
impl From<PrimitiveType> for PopType {
    fn from(v: PrimitiveType) -> PopType {
        PopType::Type(Type::Primitive(v))
    }
}
impl From<ComplexType> for PopType {
    fn from(v: ComplexType) -> PopType {
        PopType::Type(Type::Complex(v))
    }
}
impl From<WithType> for PopType {
    fn from(v: WithType) -> PopType {
        PopType::Type(Type::With(v))
    }
}
impl From<PopComplexType> for PopType {
    fn from(v: PopComplexType) -> PopType {
        PopType::Complex(v)
    }
}

#[derive(Debug, Clone)]
pub enum PushType {
    Type(Type),
}
impl From<Type> for PushType {
    fn from(typ: Type) -> PushType {
        PushType::Type(typ)
    }
}
impl From<PrimitiveType> for PushType {
    fn from(v: PrimitiveType) -> PushType {
        PushType::Type(Type::Primitive(v))
    }
}
impl From<ComplexType> for PushType {
    fn from(v: ComplexType) -> Self {
        PushType::Type(Type::Complex(v))
    }
}
impl From<WithType> for PushType {
    fn from(v: WithType) -> PushType {
        PushType::Type(Type::With(v))
    }
}

pub enum LocalVariableType {
    Type(Type),
}
impl From<Type> for LocalVariableType {
    fn from(typ: Type) -> LocalVariableType {
        LocalVariableType::Type(typ)
    }
}
impl From<PrimitiveType> for LocalVariableType {
    fn from(v: PrimitiveType) -> LocalVariableType {
        LocalVariableType::Type(Type::Primitive(v))
    }
}
impl From<ComplexType> for LocalVariableType {
    fn from(v: ComplexType) -> LocalVariableType {
        LocalVariableType::Type(Type::Complex(v))
    }
}
impl From<WithType> for LocalVariableType {
    fn from(v: WithType) -> LocalVariableType {
        LocalVariableType::Type(Type::With(v))
    }
}

#[derive(Debug, Clone)]
pub enum LocalVariableInType {
    Primitive(PrimitiveType),
    ReferenceAny,
}
impl From<PrimitiveType> for LocalVariableInType {
    fn from(v: PrimitiveType) -> LocalVariableInType {
        LocalVariableInType::Primitive(v)
    }
}

/// Helper enum
enum PrimOrId {
    Primitive(PrimitiveType),
    ClassId(ClassId),
}

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(PrimitiveType),
    Complex(ComplexType),
    With(WithType),
}
impl Type {
    fn basic_descriptor_type_as_type(desc: DescriptorTypeBasic) -> PrimOrId {
        PrimOrId::Primitive(match desc {
            DescriptorTypeBasic::Byte => PrimitiveType::Byte,
            DescriptorTypeBasic::Char => PrimitiveType::Char,
            DescriptorTypeBasic::Double => PrimitiveType::Double,
            DescriptorTypeBasic::Float => PrimitiveType::Float,
            DescriptorTypeBasic::Int => PrimitiveType::Int,
            DescriptorTypeBasic::Long => PrimitiveType::Long,
            DescriptorTypeBasic::Class(class_id) => return PrimOrId::ClassId(class_id),
            DescriptorTypeBasic::Short => PrimitiveType::Short,
            DescriptorTypeBasic::Boolean => PrimitiveType::Boolean,
        })
    }

    pub(crate) fn from_basic_descriptor_type(desc: DescriptorTypeBasic) -> Type {
        match Type::basic_descriptor_type_as_type(desc) {
            PrimOrId::Primitive(prim) => prim.into(),
            PrimOrId::ClassId(class_id) => ComplexType::ReferenceClass(class_id).into(),
        }
    }

    pub(crate) fn from_descriptor_type(desc: DescriptorType) -> Type {
        match desc {
            DescriptorType::Basic(basic) => Type::from_basic_descriptor_type(basic),
            DescriptorType::Array { level, component } => {
                match Type::basic_descriptor_type_as_type(component) {
                    PrimOrId::Primitive(primitive) => {
                        { ComplexType::RefArrayPrimitiveLevels { level, primitive } }.into()
                    }
                    PrimOrId::ClassId(class_id) => {
                        ComplexType::RefArrayLevels { level, class_id }.into()
                    }
                }
            }
        }
    }
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

#[derive(Debug, Clone)]
pub enum StackInfoError {
    /// The class file id was incorrect, it really shouldn't have been.
    InvalidClassId,
    /// The method id was incorrect, it really shouldn't have been
    InvalidMethodId,
    /// Tried to index into the constant pool, but failed to get it
    InvalidConstantPoolIndex(ConstantPoolIndexRaw<ConstantInfo>),
    // TODO: provide the expected type, and what it should have been?
    /// Got a value from the constant pool but it was the wrong kind
    IncorrectConstantPoolType,
    /// There was an error parsing a descriptor type
    InvalidDescriptorType(classfile_parser::descriptor::DescriptorTypeError),
    /// When parsing the type of a field, it had remaining data.
    /// This indicates either a bug in the parsing or a problem with the class file
    UnparsedFieldType,
    /// It needed a stack size at the given index to make a decision about the stack infop
    NeededStackSizeAt(usize),
    /// The stack sizes were bad for what it needed.
    /// Ex: dup2 only makes sense as either a category 1 then a category 1
    /// or a category 2
    /// but not as a category 1 then a category 2
    BadStackSizes,
}

pub trait PushTypeAt {
    /// Must be contiguous
    #[must_use]
    fn push_type_at(&self, i: PushIndex) -> Option<PushType>;

    fn push_count(&self) -> usize;
}

pub trait PopTypeAt {
    /// Must be contiguous
    #[must_use]
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType>;

    #[must_use]
    fn pop_count(&self) -> usize;
}

pub trait LocalsOutAt {
    type Iter: Iterator<Item = (LocalVariableIndex, LocalVariableType)>;

    #[must_use]
    fn locals_out_type_iter(&self) -> Self::Iter;
}

pub trait LocalsIn {
    type Iter: Iterator<Item = (LocalVariableIndex, LocalVariableInType)>;

    #[must_use]
    fn locals_in_type_iter(&self) -> Self::Iter;
}

pub trait StackInfo: PushTypeAt + PopTypeAt + LocalsOutAt + LocalsIn {}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Category {
    /// 4 byte sized types. Int, Byte, Char, Float, etc.
    One,
    /// 8 byte sized types. Double, and Long.
    Two,
}
/// The stack sizes of a few of the topmost stack elements
/// These are unfortunately needed for getting stack info out of Dup
/// elements.
/// This is done rather than giving them the stack to keep the outside more generic.
/// So that it can be used with stackmaptypes or frametypes
pub type StackSizes = [Option<Category>; 4];

/// Indicates that the given type has stack info it can supply
/// We could do a more complicated implementation of this that allows dependence
/// on their reference to `self`, but the majority/all instructions can be cloned
/// cheaply, so allowin returning a reference to it doesn't gain much.
pub trait HasStackInfo {
    type Output: StackInfo;

    /// The class id and method id must exist
    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
        method_id: MethodId,
        stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError>;
}

/// Note that this is for things that behave like an instruction
/// This means it is not a pure marker trait, because it is implemented for the enum of all
/// instructions.
pub trait Instruction: HasStackInfo {
    fn name(&self) -> &'static str;
}
