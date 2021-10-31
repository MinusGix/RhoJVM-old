//! This file is separate from op.rs, because op.rs is large enough to be unfortunately slow.

use classfile_parser::attribute_info::InstructionIndex;

use super::op::{ANewArray, InvokeSpecial, MultiANewArray, NewArray};
use super::types::ComplexType;
use super::{
    op::RawOpcode,
    types::{PopIndex, PrimitiveType, Type, WithType},
};

#[derive(Debug)]
pub enum InstructionParseError {
    NotEnoughData {
        opcode: RawOpcode,
        needed: usize,
        had: usize,
    },
    ExpectedOpCodeAt(InstructionIndex),
    UnknownOpcode {
        idx: InstructionIndex,
        opcode: RawOpcode,
    },
    UnknownWideOpcode {
        idx: InstructionIndex,
        opcode: RawOpcode,
    },
}

// === Pop/Push implementations for various opcodes ===
impl ANewArray {
    // TODO: This could be implemented in the macro if we had access to `self`
    #[must_use]
    pub fn push_type_at(&self, i: usize) -> Option<Type> {
        if i == 0 {
            Some(
                WithType::RefArrayRefFromIndexLen {
                    index: self.index,
                    len_idx: 0,
                    is_all_null: true,
                }
                .into(),
            )
        } else {
            None
        }
    }
}

impl MultiANewArray {
    #[must_use]
    pub fn pop_type_at(&self, i: usize) -> Option<Type> {
        if i < self.dimensions as usize {
            // count
            Some(PrimitiveType::Int.into())
        } else {
            None
        }
    }

    #[must_use]
    pub fn push_type_at(&self, i: usize) -> Option<Type> {
        if i == 0 {
            Some(
                WithType::RefMultiDimArrayRefFromIndexLengthsRange {
                    index: self.index,
                    len_idxs: 0..(self.dimensions as PopIndex),
                    is_all_base_default: true,
                }
                .into(),
            )
        } else {
            None
        }
    }
}
impl NewArray {
    #[must_use]
    pub fn push_type_at(&self, i: usize) -> Option<Type> {
        if i == 0 {
            let element_type = match self.atype {
                4 => PrimitiveType::Boolean,
                5 => PrimitiveType::Char,
                6 => PrimitiveType::Float,
                7 => PrimitiveType::Double,
                8 => PrimitiveType::Byte,
                9 => PrimitiveType::Short,
                10 => PrimitiveType::Int,
                11 => PrimitiveType::Long,
                // TODO: don't panic
                _ => panic!("AType argument for NewArray was invalid"),
            };
            Some(
                WithType::RefArrayPrimitiveLen {
                    element_type,
                    len_idx: 0,
                    is_default_init: true,
                }
                .into(),
            )
        } else {
            None
        }
    }
}
impl InvokeSpecial {
    #[must_use]
    pub fn pop_type_at(&self, i: PopIndex) -> Option<Type> {
        if i == 0 {
            // objectref
            return Some(ComplexType::ReferenceAny.into());
        }

        // TODO: Arguments

        None
    }
}
