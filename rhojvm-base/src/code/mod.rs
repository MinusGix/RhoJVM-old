use classfile_parser::{
    attribute_info::{AttributeInfo, ExceptionEntry},
    constant_info::ConstantInfo,
};

use crate::{class::ClassFileData, util::MemorySize, VerifyCodeExceptionError};

use self::{method::Method, op::Inst, op_ex::InstructionParseError};

pub use classfile_parser::attribute_info::InstructionIndex;

pub mod method;
pub mod op;
pub mod op_ex;
mod op_print;
pub mod stack_map;
pub mod types;

/// Inst with location
pub type InstL = (InstructionIndex, Inst);

#[derive(Debug, Clone)]
pub struct CodeInfo {
    pub(crate) instructions: Vec<InstL>,
    pub(crate) max_locals: u16,
    pub(crate) max_stack: u16,
    pub(crate) exception_table: Vec<ExceptionEntry>,
    pub(crate) attributes: Vec<AttributeInfo>,
}
impl CodeInfo {
    #[must_use]
    /// Get an index into the insts vec from an index into the code array
    fn get_instruction_idx(&self, idx: InstructionIndex) -> Option<usize> {
        for (i, (i_idx, _)) in self.instructions.iter().enumerate() {
            match idx.cmp(i_idx) {
                std::cmp::Ordering::Equal => return Some(i),
                // It was less than, and since the indices are increasing
                // that means we've run past the index we were looking for
                // (it was probably inside an instruction)
                std::cmp::Ordering::Less => return None,
                std::cmp::Ordering::Greater => {}
            }
        }

        // We failed to find the instruction
        None
    }

    #[must_use]
    pub fn instructions(&self) -> &[InstL] {
        &self.instructions
    }

    #[must_use]
    pub fn max_locals(&self) -> u16 {
        self.max_locals
    }

    #[must_use]
    pub fn max_stack(&self) -> u16 {
        self.max_stack
    }

    #[must_use]
    pub fn exception_table(&self) -> &[ExceptionEntry] {
        &self.exception_table
    }

    #[must_use]
    pub fn attributes(&self) -> &[AttributeInfo] {
        &self.attributes
    }

    #[must_use]
    /// If the index is not found -> None
    /// If the index would be inside an instruction -> None
    pub fn get_instruction_at(&self, idx: InstructionIndex) -> Option<&Inst> {
        self.get_instruction_idx(idx)
            .and_then(|x| self.instructions.get(x))
            .map(|x| &x.1)
    }

    #[must_use]
    /// If the index is not found -> None
    /// If the index would be inside an instruction -> None
    pub fn get_instruction_mut_at(&mut self, idx: InstructionIndex) -> Option<&mut Inst> {
        self.get_instruction_idx(idx)
            .and_then(move |x| self.instructions.get_mut(x))
            .map(|x| &mut x.1)
    }

    #[must_use]
    pub fn has_instruction_at(&self, idx: InstructionIndex) -> bool {
        self.get_instruction_idx(idx).is_some()
    }

    #[must_use]
    pub fn last(&self) -> Option<&(InstructionIndex, Inst)> {
        self.instructions.last()
    }

    #[must_use]
    pub fn code_length(&self) -> u16 {
        if let Some((idx, inst)) = self.last() {
            let size: u16 = inst
                .memory_size()
                .try_into()
                .expect("Inst memory size to fit within u16");
            idx.0 + size
        } else {
            0
        }
    }

    pub(crate) fn invokes_init_methods(
        &self,
        class_file: &ClassFileData,
    ) -> Result<bool, VerifyCodeExceptionError> {
        for (_, inst) in &self.instructions {
            if let Inst::InvokeSpecial(inv) = inst {
                let method = class_file
                    .get_t(inv.index)
                    .ok_or(VerifyCodeExceptionError::InvalidInvokeSpecialMethodIndex)?;
                let name_and_type_idx = match method {
                    ConstantInfo::MethodRef(r) => r.name_and_type_index,
                    ConstantInfo::InterfaceMethodRef(r) => r.name_and_type_index,
                    _ => return Err(VerifyCodeExceptionError::InvalidInvokeSpecialInfo),
                };
                let nat = class_file
                    .get_t(name_and_type_idx)
                    .ok_or(VerifyCodeExceptionError::InvalidInvokeSpecialMethodNameTypeIndex)?;
                let name = class_file
                    .get_text_t(nat.name_index)
                    .ok_or(VerifyCodeExceptionError::InvalidInvokeSpecialMethodNameIndex)?;
                if name == "<init>" {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Checks an exception.
    /// Note that this only checks validity for applying it to the function, and doesn't
    /// care if it is actually from this function or not
    pub(crate) fn check_exception_basic(
        &self,
        exc: &ExceptionEntry,
    ) -> Result<(), VerifyCodeExceptionError> {
        // the start must be before the end
        // start..end
        if exc.start_pc >= exc.end_pc {
            return Err(VerifyCodeExceptionError::InverseOrder);
        }

        if !self.has_instruction_at(exc.start_pc) {
            return Err(VerifyCodeExceptionError::InvalidStartIndex);
        }

        // end must either be a valid instruction,
        // or it must be the last index of the code
        if exc.end_pc.0 != self.code_length() && !self.has_instruction_at(exc.end_pc) {
            return Err(VerifyCodeExceptionError::InvalidEndIndex);
        }

        // Ensure that there is code at the handler
        if !self.has_instruction_at(exc.handler_pc) {
            return Err(VerifyCodeExceptionError::InvalidHandlerIndex);
        }
        Ok(())
    }

    pub(crate) fn check_exception(
        &self,
        class_file: &ClassFileData,
        method: &Method,
        exc: &ExceptionEntry,
    ) -> Result<(), VerifyCodeExceptionError> {
        self.check_exception_basic(exc)?;

        // initHandlerIsLegal(1)
        if method.name == "<init>" {
            // notInitHandler (2)
            let has_init_calls = self.invokes_init_methods(class_file)?;
            if has_init_calls {
                // initHandlerIsLegal (2)
                // sublist of handler instructions
                // TODO: It seems like using exc.end_pc is required, even thought it is not
                // mentioned in the docs.
                let has_returns = self
                    .instructions
                    .iter()
                    .filter(|(idx, _)| *idx >= exc.start_pc && *idx < exc.end_pc)
                    .any(|(_, inst)| matches!(inst, Inst::Return(_)));
                if has_returns {
                    let has_athrow = self
                        .instructions
                        .iter()
                        .filter(|(idx, _)| *idx >= exc.start_pc && *idx < exc.end_pc)
                        .any(|(_, inst)| matches!(inst, Inst::AThrow(_)));
                    if !has_athrow {
                        return Err(VerifyCodeExceptionError::IllegalInstructions);
                    }
                }
                // otherwise no returns and it is legal
            }
            // otherwise no init calls and so it is legal
        }
        // otherwise, if it isn't an init method, this doesn't apply

        Ok(())
    }
}

pub(crate) fn parse_code(
    mut code_attr: classfile_parser::attribute_info::CodeAttribute,
) -> Result<CodeInfo, InstructionParseError> {
    // TODO: if the class file version number is >=51.0 then neither JSR or JSR_W can appear
    let code = code_attr.code.as_slice();
    let mut instructions = Vec::new();

    let mut idx: u16 = 0;
    while (idx as usize) < code.len() {
        // We don't need to give the entirety of the code to the instructions but it does not
        // harm anything.
        let inst = Inst::parse(code, InstructionIndex(idx))?;
        let size: u16 = inst
            .memory_size()
            .try_into()
            .expect("All inst memory sizes should fit into a u16");
        instructions.push((InstructionIndex(idx), inst));
        idx += size;
    }

    Ok(CodeInfo {
        instructions,
        max_locals: code_attr.max_locals,
        max_stack: code_attr.max_stack,
        exception_table: std::mem::take(&mut code_attr.exception_table),
        attributes: std::mem::take(&mut code_attr.attributes),
    })
}
