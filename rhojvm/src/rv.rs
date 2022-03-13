use rhojvm_base::{
    code::{
        method::{DescriptorType, DescriptorTypeBasic},
        types::{JavaChar, PrimitiveType},
    },
    data::class_names::ClassNames,
    id::ClassId,
    BadIdError,
};

use crate::{class_instance::ReferenceInstance, gc::GcRef};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RuntimeValuePrimitive {
    /// Long
    I64(i64),
    /// Integer
    I32(i32),
    /// Short
    I16(i16),
    /// Byte
    I8(i8),
    /// Float
    F32(f32),
    /// Double
    F64(f64),

    /// Char
    Char(JavaChar),

    /// Considered equivalent to an i8
    Bool(bool),
}
impl RuntimeValuePrimitive {
    #[must_use]
    pub fn is_long(&self) -> bool {
        matches!(self, RuntimeValuePrimitive::I64(_))
    }

    #[must_use]
    pub fn is_float(&self) -> bool {
        matches!(self, RuntimeValuePrimitive::F32(_))
    }

    #[must_use]
    pub fn is_double(&self) -> bool {
        matches!(self, RuntimeValuePrimitive::F64(_))
    }

    #[must_use]
    pub fn is_category_2(&self) -> bool {
        matches!(
            self,
            RuntimeValuePrimitive::I64(_) | RuntimeValuePrimitive::F64(_)
        )
    }

    #[must_use]
    pub fn can_be_int(&self) -> bool {
        matches!(
            self,
            RuntimeValuePrimitive::I32(_)
                | RuntimeValuePrimitive::I16(_)
                | RuntimeValuePrimitive::I8(_)
                | RuntimeValuePrimitive::Char(_)
                | RuntimeValuePrimitive::Bool(_)
        )
    }

    #[must_use]
    pub fn into_byte(self) -> Option<i8> {
        #[allow(clippy::cast_possible_truncation)]
        self.into_int().map(|x| x as i8)
    }

    #[must_use]
    pub fn into_short(self) -> Option<i16> {
        #[allow(clippy::cast_possible_truncation)]
        self.into_int().map(|x| x as i16)
    }

    #[must_use]
    pub fn into_char(self) -> Option<JavaChar> {
        self.into_int().map(JavaChar::from_int)
    }

    #[must_use]
    pub fn into_bool_loose(self) -> Option<u8> {
        #[allow(clippy::cast_possible_truncation)]
        self.into_int().map(|x| x as u8)
    }

    #[must_use]
    pub fn into_f32(self) -> Option<f32> {
        match self {
            RuntimeValuePrimitive::F32(x) => Some(x),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_f64(self) -> Option<f64> {
        match self {
            RuntimeValuePrimitive::F64(x) => Some(x),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_i16(self) -> Option<i16> {
        Some(match self {
            RuntimeValuePrimitive::I16(x) => x,
            _ => return None,
        })
    }

    #[must_use]
    pub fn into_i32(self) -> Option<i32> {
        Some(match self {
            RuntimeValuePrimitive::I32(x) => x,
            _ => return None,
        })
    }

    #[must_use]
    pub fn into_i64(self) -> Option<i64> {
        Some(match self {
            RuntimeValuePrimitive::I64(x) => x,
            _ => return None,
        })
    }

    /// Converts the value into itself as an integer, if it is allowed to be converted
    /// This is intended for use by code that handles, well, code, where types like
    /// bool/byte/short/char/int are all collapsed into int
    #[must_use]
    pub fn into_int(self) -> Option<i32> {
        Some(match self {
            RuntimeValuePrimitive::I32(x) => x,
            RuntimeValuePrimitive::I16(x) => i32::from(x),
            RuntimeValuePrimitive::I8(x) => i32::from(x),
            RuntimeValuePrimitive::Char(x) => x.as_int(),
            RuntimeValuePrimitive::Bool(x) => i32::from(x),
            _ => return None,
        })
    }

    #[must_use]
    pub fn runtime_type(&self) -> RuntimeTypePrimitive {
        match self {
            RuntimeValuePrimitive::I64(_) => RuntimeTypePrimitive::I64,
            RuntimeValuePrimitive::I32(_) => RuntimeTypePrimitive::I32,
            RuntimeValuePrimitive::I16(_) => RuntimeTypePrimitive::I16,
            RuntimeValuePrimitive::I8(_) | RuntimeValuePrimitive::Bool(_) => {
                RuntimeTypePrimitive::I8
            }
            RuntimeValuePrimitive::F32(_) => RuntimeTypePrimitive::F32,
            RuntimeValuePrimitive::F64(_) => RuntimeTypePrimitive::F64,
            RuntimeValuePrimitive::Char(_) => RuntimeTypePrimitive::Char,
        }
    }
}

#[derive(Debug)]
pub enum RuntimeValue<REF = ReferenceInstance> {
    Primitive(RuntimeValuePrimitive),
    NullReference,
    Reference(GcRef<REF>),
}
impl<REF> RuntimeValue<REF> {
    /// Returns whether or not it is a reference type
    /// Note that this includes null reference as well as a known reference
    #[must_use]
    pub fn is_reference(&self) -> bool {
        matches!(
            self,
            RuntimeValue::NullReference | RuntimeValue::Reference(_)
        )
    }

    #[must_use]
    pub fn is_long(&self) -> bool {
        match self {
            Self::Primitive(x) => x.is_long(),
            _ => false,
        }
    }

    #[must_use]
    pub fn is_float(&self) -> bool {
        match self {
            Self::Primitive(x) => x.is_float(),
            _ => false,
        }
    }

    #[must_use]
    pub fn is_double(&self) -> bool {
        match self {
            Self::Primitive(x) => x.is_double(),
            _ => false,
        }
    }

    #[must_use]
    pub fn is_category_2(&self) -> bool {
        match self {
            Self::Primitive(x) => x.is_category_2(),
            _ => false,
        }
    }

    #[must_use]
    pub fn can_be_int(&self) -> bool {
        match self {
            Self::Primitive(x) => x.can_be_int(),
            _ => false,
        }
    }

    /// Convert this into a reference, if it is one.
    /// If the first layer is None, that means it was a non-reference.
    /// If the second layer is None, that means it is a null reference.
    /// If the second layer is Some, that means it is a valid reference.
    #[must_use]
    pub fn into_reference(self) -> Option<Option<GcRef<REF>>> {
        match self {
            RuntimeValue::Reference(x) => Some(Some(x)),
            RuntimeValue::NullReference => Some(None),
            RuntimeValue::Primitive(_) => None,
        }
    }

    #[must_use]
    pub fn into_byte(self) -> Option<i8> {
        match self {
            Self::Primitive(x) => x.into_byte(),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_short(self) -> Option<i16> {
        match self {
            Self::Primitive(x) => x.into_short(),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_char(self) -> Option<JavaChar> {
        match self {
            Self::Primitive(x) => x.into_char(),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_bool_loose(self) -> Option<u8> {
        match self {
            Self::Primitive(x) => x.into_bool_loose(),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_f32(self) -> Option<f32> {
        match self {
            Self::Primitive(x) => x.into_f32(),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_f64(self) -> Option<f64> {
        match self {
            Self::Primitive(x) => x.into_f64(),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_i16(self) -> Option<i16> {
        match self {
            Self::Primitive(x) => x.into_i16(),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_i32(self) -> Option<i32> {
        match self {
            Self::Primitive(x) => x.into_i32(),
            _ => None,
        }
    }

    #[must_use]
    pub fn into_i64(self) -> Option<i64> {
        match self {
            Self::Primitive(x) => x.into_i64(),
            _ => None,
        }
    }

    /// Converts the value into itself as an integer, if it is allowed to be converted
    /// This is intended for use by code that handles, well, code, where types like
    /// bool/byte/short/char/int are all collapsed into int
    #[must_use]
    pub fn into_int(self) -> Option<i32> {
        match self {
            Self::Primitive(x) => x.into_int(),
            _ => None,
        }
    }

    #[must_use]
    pub fn runtime_type(&self) -> RuntimeType {
        match self {
            RuntimeValue::Primitive(p) => p.runtime_type().into(),
            RuntimeValue::Reference(_) | RuntimeValue::NullReference => RuntimeType::Reference(()),
        }
    }
}

impl<REF> Copy for RuntimeValue<REF> {}
impl<REF> Clone for RuntimeValue<REF> {
    fn clone(&self) -> Self {
        match self {
            Self::Primitive(x) => Self::Primitive(*x),
            Self::NullReference => Self::NullReference,
            Self::Reference(x) => Self::Reference(*x),
        }
    }
}
impl<REF> From<RuntimeValuePrimitive> for RuntimeValue<REF> {
    fn from(p: RuntimeValuePrimitive) -> Self {
        RuntimeValue::Primitive(p)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeType<REF = ()> {
    Primitive(RuntimeTypePrimitive),
    Reference(REF),
}
impl RuntimeType<ClassId> {
    pub fn from_descriptor_type(
        class_names: &mut ClassNames,
        desc: DescriptorType,
    ) -> Result<RuntimeType<ClassId>, BadIdError> {
        Ok(match desc {
            DescriptorType::Basic(bdesc) => match bdesc {
                DescriptorTypeBasic::Byte | DescriptorTypeBasic::Boolean => {
                    RuntimeTypePrimitive::I8.into()
                }
                DescriptorTypeBasic::Char => RuntimeTypePrimitive::Char.into(),
                DescriptorTypeBasic::Short => RuntimeTypePrimitive::I16.into(),
                DescriptorTypeBasic::Int => RuntimeTypePrimitive::I32.into(),
                DescriptorTypeBasic::Long => RuntimeTypePrimitive::I64.into(),
                DescriptorTypeBasic::Float => RuntimeTypePrimitive::F32.into(),
                DescriptorTypeBasic::Double => RuntimeTypePrimitive::F64.into(),
                DescriptorTypeBasic::Class(id) => RuntimeType::Reference(id),
            },
            DescriptorType::Array { level, component } => {
                let id = class_names.gcid_from_level_array_of_desc_type_basic(level, component)?;
                RuntimeType::Reference(id)
            }
        })
    }
}
impl<REF> RuntimeType<REF> {
    #[must_use]
    pub fn default_value(self) -> RuntimeValue {
        match self {
            RuntimeType::Primitive(p) => p.default_value().into(),
            RuntimeType::Reference(_) => RuntimeValue::NullReference,
        }
    }

    #[must_use]
    pub fn is_reference(&self) -> bool {
        matches!(self, RuntimeType::Reference(_))
    }

    #[must_use]
    pub fn is_long(&self) -> bool {
        match self {
            Self::Primitive(p) => p.is_long(),
            Self::Reference(_) => false,
        }
    }

    #[must_use]
    pub fn is_float(&self) -> bool {
        match self {
            Self::Primitive(p) => p.is_float(),
            Self::Reference(_) => false,
        }
    }

    #[must_use]
    pub fn is_double(&self) -> bool {
        match self {
            Self::Primitive(p) => p.is_double(),
            Self::Reference(_) => false,
        }
    }

    #[must_use]
    pub fn is_category_2(&self) -> bool {
        match self {
            Self::Primitive(p) => p.is_category_2(),
            Self::Reference(_) => false,
        }
    }

    #[must_use]
    pub fn can_be_int(&self) -> bool {
        match self {
            Self::Primitive(p) => p.can_be_int(),
            Self::Reference(_) => false,
        }
    }
}
impl<REF> From<RuntimeTypePrimitive> for RuntimeType<REF> {
    fn from(v: RuntimeTypePrimitive) -> Self {
        RuntimeType::Primitive(v)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTypePrimitive {
    I64,
    I32,
    I16,
    I8,
    F32,
    F64,
    Char,
}
impl From<PrimitiveType> for RuntimeTypePrimitive {
    fn from(v: PrimitiveType) -> Self {
        match v {
            PrimitiveType::Byte | PrimitiveType::UnsignedByte | PrimitiveType::Boolean => {
                RuntimeTypePrimitive::I8
            }
            PrimitiveType::Short | PrimitiveType::UnsignedShort => RuntimeTypePrimitive::I16,
            PrimitiveType::Int => RuntimeTypePrimitive::I32,
            PrimitiveType::Long => RuntimeTypePrimitive::I64,
            PrimitiveType::Float => RuntimeTypePrimitive::F32,
            PrimitiveType::Double => RuntimeTypePrimitive::F64,
            PrimitiveType::Char => RuntimeTypePrimitive::Char,
        }
    }
}
impl RuntimeTypePrimitive {
    #[must_use]
    pub fn default_value(self) -> RuntimeValuePrimitive {
        match self {
            RuntimeTypePrimitive::I64 => RuntimeValuePrimitive::I64(0),
            RuntimeTypePrimitive::I32 => RuntimeValuePrimitive::I32(0),
            RuntimeTypePrimitive::I16 => RuntimeValuePrimitive::I16(0),
            RuntimeTypePrimitive::I8 => RuntimeValuePrimitive::I8(0),
            RuntimeTypePrimitive::F32 => RuntimeValuePrimitive::F32(0.0),
            RuntimeTypePrimitive::F64 => RuntimeValuePrimitive::F64(0.0),
            RuntimeTypePrimitive::Char => RuntimeValuePrimitive::Char(JavaChar(0)),
        }
    }

    #[must_use]
    pub fn is_long(&self) -> bool {
        matches!(self, RuntimeTypePrimitive::I64)
    }

    #[must_use]
    pub fn is_float(&self) -> bool {
        matches!(self, RuntimeTypePrimitive::F32)
    }

    #[must_use]
    pub fn is_double(&self) -> bool {
        matches!(self, RuntimeTypePrimitive::F64)
    }

    #[must_use]
    pub fn is_category_2(&self) -> bool {
        matches!(self, RuntimeTypePrimitive::I64 | RuntimeTypePrimitive::F64)
    }

    #[must_use]
    pub fn can_be_int(&self) -> bool {
        matches!(
            self,
            RuntimeTypePrimitive::I32
                | RuntimeTypePrimitive::I16
                | RuntimeTypePrimitive::I8
                | RuntimeTypePrimitive::Char
        )
    }
}
