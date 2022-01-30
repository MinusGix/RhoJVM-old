use rhojvm_base::code::types::JavaChar;

use crate::{class_instance::Instance, gc::GcRef};

#[derive(Debug, Clone, Copy)]
pub enum RuntimeValue {
    /// Long
    I64(i64),
    U64(u64),
    /// Integer
    I32(i32),
    U32(u32),
    /// Short
    I16(i16),
    U16(u16),
    /// Byte
    I8(i8),
    U8(u8),
    /// Float
    F32(f32),
    /// Double
    F64(f64),

    /// Char
    Char(JavaChar),

    Bool(bool),

    NullReference,
    Reference(GcRef<Instance>),
}
impl RuntimeValue {
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
        matches!(self, RuntimeValue::I64(_) | RuntimeValue::U64(_))
    }

    #[must_use]
    pub fn is_float(&self) -> bool {
        matches!(self, RuntimeValue::F32(_))
    }

    #[must_use]
    pub fn is_double(&self) -> bool {
        matches!(self, RuntimeValue::F64(_))
    }

    #[must_use]
    pub fn is_category_2(&self) -> bool {
        matches!(
            self,
            RuntimeValue::I64(_) | RuntimeValue::U64(_) | RuntimeValue::F64(_)
        )
    }

    pub fn can_be_int(&self) -> bool {
        matches!(
            self,
            RuntimeValue::I32(_)
                | RuntimeValue::U32(_)
                | RuntimeValue::I16(_)
                | RuntimeValue::U16(_)
                | RuntimeValue::I8(_)
                | RuntimeValue::U8(_)
                | RuntimeValue::Char(_)
        )
    }

    // /// Converts the value into itself as an integer, if it is allowed to be converted
    // /// This is intended for use by code that handles, well, code, where types like
    // /// bool/byte/short/char/int are all collapsed into int
    // #[must_use]
    // pub fn into_int(self) -> Option<RuntimeValue> {
    //     Some(match self {
    //         RuntimeValue::I64(_) => todo!(),
    //         RuntimeValue::U64(_) => todo!(),
    //         RuntimeValue::I32(_) => todo!(),
    //         RuntimeValue::U32(_) => todo!(),
    //         RuntimeValue::I16(_) => todo!(),
    //         RuntimeValue::U16(_) => todo!(),
    //         RuntimeValue::I8(_) => todo!(),
    //         RuntimeValue::U8(_) => todo!(),
    //         RuntimeValue::Char(_) => todo!(),
    //         RuntimeValue::NullReference | RuntimeValue::Reference(_) => return None,
    //     })
    // }
}

#[derive(Debug, Clone, Copy)]
pub enum RuntimeType {
    I64,
    U64,
    I32,
    U32,
    I16,
    U16,
    I8,
    U8,
    F32,
    F64,
    Char,
    Bool,
    NullReference,
    Reference,
}
