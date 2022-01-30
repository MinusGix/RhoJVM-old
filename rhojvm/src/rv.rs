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

    /// Char
    Char(JavaChar),

    NullReference,
    Reference(GcRef<Instance>),
}
