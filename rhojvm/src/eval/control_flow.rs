use std::cmp::Ordering;

use classfile_parser::attribute_info::InstructionIndex;
use rhojvm_base::{
    code::op::{
        AReturn, AThrow, DoubleCmpG, DoubleCmpL, DoubleReturn, FloatCmpG, FloatCmpL, FloatReturn,
        Goto, IfACmpEq, IfACmpNe, IfEqZero, IfGeZero, IfGtZero, IfIntCmpEq, IfIntCmpGe, IfIntCmpGt,
        IfIntCmpLe, IfIntCmpLt, IfIntCmpNe, IfLeZero, IfLtZero, IfNeZero, IfNonNull, IfNull,
        IntReturn, LongCmp, LongReturn, LookupSwitch, MonitorEnter, MonitorExit, Return,
        TableSwitch,
    },
    data::classes::does_extend_class,
};

use crate::{
    class_instance::ReferenceInstance,
    eval::EvalError,
    rv::{RuntimeValue, RuntimeValuePrimitive},
    util::{self},
    GeneralError,
};

use super::{RunInst, RunInstArgs, RunInstValue};

impl RunInst for Return {
    fn run(self, _: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        // TODO: monitor
        Ok(RunInstValue::ReturnVoid)
    }
}

// Note that this implementation of IntReturn keeps the types as much as possible and is more
// accurately 'IntReprReturn', an instruction which returns a type which can be represented as an
// int
impl RunInst for IntReturn {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        // TODO: monitor

        // It is up to the function running this to determine if it makes sense for the method
        // to be returning this
        let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if !object.can_be_int() {
            return Err(EvalError::ExpectedStackValueIntRepr.into());
        }

        Ok(RunInstValue::Return(object))
    }
}

impl RunInst for FloatReturn {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        // TODO: monitor

        // It is up to the function running this to determine if it makes sense for the method
        // to be returning this
        let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if !object.is_float() {
            return Err(EvalError::ExpectedStackValueFloat.into());
        }

        Ok(RunInstValue::Return(object))
    }
}

impl RunInst for LongReturn {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        // TODO: monitor

        // It is up to the function running this to determine if it makes sense for the method
        // to be returning this
        let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if !object.is_long() {
            return Err(EvalError::ExpectedStackValueLong.into());
        }

        Ok(RunInstValue::Return(object))
    }
}

impl RunInst for DoubleReturn {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        // TODO: monitor

        // It is up to the function running this to determine if it makes sense for the method
        // to be returning this
        let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if !object.is_double() {
            return Err(EvalError::ExpectedStackValueDouble.into());
        }

        Ok(RunInstValue::Return(object))
    }
}

impl RunInst for AReturn {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        // TODO: monitor

        // It is up to the function running this to determine if it makes sense for the method
        // to be returning this
        let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if !object.is_reference() {
            return Err(EvalError::ExpectedStackValueReference.into());
        }

        Ok(RunInstValue::Return(object))
    }
}

fn branch_if(
    cond: bool,
    inst_index: InstructionIndex,
    branch_offset: i16,
) -> Result<RunInstValue, GeneralError> {
    if cond {
        let destination = util::signed_offset_16(inst_index.0, branch_offset)
            .ok_or(EvalError::BranchOverflows)?;
        let destination = InstructionIndex(destination);
        Ok(RunInstValue::ContinueAt(destination))
    } else {
        Ok(RunInstValue::Continue)
    }
}

impl RunInst for Goto {
    fn run(
        self,
        RunInstArgs { inst_index, .. }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        branch_if(true, inst_index, self.branch_offset)
    }
}

impl RunInst for AThrow {
    fn run(
        self,
        RunInstArgs { env, frame, .. }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        // TODO: monitor

        let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        match object {
            RuntimeValue::NullReference => todo!("return null pointer exception"),
            RuntimeValue::Reference(gc_ref) => {
                let instance = env
                    .state
                    .gc
                    .deref(gc_ref)
                    .ok_or(EvalError::InvalidGcRef(gc_ref.into_generic()))?;
                if let ReferenceInstance::Class(instance) = instance {
                    let throwable_id = env.class_names.gcid_from_bytes(b"java/lang/Throwable");
                    if does_extend_class(
                        &mut env.class_names,
                        &mut env.class_files,
                        &env.classes,
                        instance.instanceof,
                        throwable_id,
                    )? {
                        // TODO: It would be possible to provide a checked as version
                        Ok(RunInstValue::Exception(gc_ref.unchecked_as()))
                    } else {
                        Err(EvalError::ExpectedThrowable.into())
                    }
                } else {
                    Err(EvalError::ExpectedClassInstance.into())
                }
            }
            RuntimeValue::Primitive(_) => Err(EvalError::ExpectedStackValueReference.into()),
        }
    }
}

impl RunInst for IfIntCmpEq {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v2 == v1, inst_index, self.branch_offset)
    }
}
impl RunInst for IfIntCmpNe {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v2 != v1, inst_index, self.branch_offset)
    }
}
impl RunInst for IfIntCmpLt {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v2 < v1, inst_index, self.branch_offset)
    }
}
impl RunInst for IfIntCmpGt {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v2 > v1, inst_index, self.branch_offset)
    }
}
impl RunInst for IfIntCmpLe {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v2 <= v1, inst_index, self.branch_offset)
    }
}
impl RunInst for IfIntCmpGe {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v2 >= v1, inst_index, self.branch_offset)
    }
}
impl RunInst for IfEqZero {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v1 == 0, inst_index, self.branch_offset)
    }
}
impl RunInst for IfNeZero {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v1 != 0, inst_index, self.branch_offset)
    }
}
impl RunInst for IfLtZero {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v1 < 0, inst_index, self.branch_offset)
    }
}
impl RunInst for IfGtZero {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v1 > 0, inst_index, self.branch_offset)
    }
}
impl RunInst for IfLeZero {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v1 <= 0, inst_index, self.branch_offset)
    }
}
impl RunInst for IfGeZero {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        branch_if(v1 >= 0, inst_index, self.branch_offset)
    }
}

impl RunInst for IfACmpEq {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?;
        let v2 = v2
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?;

        branch_if(v2 == v1, inst_index, self.branch_offset)
    }
}
impl RunInst for IfACmpNe {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?;
        let v2 = v2
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?;

        branch_if(v2 != v1, inst_index, self.branch_offset)
    }
}
impl RunInst for IfNonNull {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?;

        branch_if(v1.is_some(), inst_index, self.branch_offset)
    }
}
impl RunInst for IfNull {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?;

        branch_if(v1.is_none(), inst_index, self.branch_offset)
    }
}

impl RunInst for LookupSwitch {
    // TODO: lookup switch has a vec, so cloning is rather expensive
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let key = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let key = key.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;

        let mut offset = None;
        for pair in &self.pairs {
            if key == pair.match_v {
                offset = Some(pair.offset);
            }
        }

        let offset = offset.unwrap_or(self.default);

        let destination =
            util::signed_offset_32_16(inst_index.0, offset).ok_or(EvalError::BranchOverflows)?;
        let destination = InstructionIndex(destination);
        Ok(RunInstValue::ContinueAt(destination))
    }
}
impl RunInst for TableSwitch {
    fn run(
        self,
        RunInstArgs {
            frame, inst_index, ..
        }: RunInstArgs,
    ) -> Result<RunInstValue, GeneralError> {
        let index = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let index = index
            .into_int()
            .ok_or(EvalError::ExpectedStackValueIntRepr)?;

        let offset = if index < self.low || index > self.high {
            self.default
        } else {
            let index = index - self.low;
            let index = usize::try_from(index).unwrap();
            // This should always exist unless someone has been modifying instructions out from
            // under us
            let offset = self.jump_offsets.get(index).unwrap();
            *offset
        };

        let destination =
            util::signed_offset_32_16(inst_index.0, offset).ok_or(EvalError::BranchOverflows)?;
        let destination = InstructionIndex(destination);

        Ok(RunInstValue::ContinueAt(destination))
    }
}

// Not exactly control flow
impl RunInst for LongCmp {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;

        let res = match v2.cmp(&v1) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        };
        frame.stack.push(RuntimeValuePrimitive::I32(res))?;

        Ok(RunInstValue::Continue)
    }
}
impl RunInst for FloatCmpL {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let v2 = v2.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;

        let res = match v2.partial_cmp(&v1) {
            Some(Ordering::Less) | None => -1,
            Some(Ordering::Equal) => 0,
            Some(Ordering::Greater) => 1,
        };
        frame.stack.push(RuntimeValuePrimitive::I32(res))?;

        Ok(RunInstValue::Continue)
    }
}
impl RunInst for FloatCmpG {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let v2 = v2.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;

        let res = match v2.partial_cmp(&v1) {
            Some(Ordering::Less) => -1,
            Some(Ordering::Equal) => 0,
            Some(Ordering::Greater) | None => 1,
        };
        frame.stack.push(RuntimeValuePrimitive::I32(res))?;

        Ok(RunInstValue::Continue)
    }
}
impl RunInst for DoubleCmpL {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let v2 = v2.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;

        let res = match v2.partial_cmp(&v1) {
            Some(Ordering::Less) | None => -1,
            Some(Ordering::Equal) => 0,
            Some(Ordering::Greater) => 1,
        };
        frame.stack.push(RuntimeValuePrimitive::I32(res))?;

        Ok(RunInstValue::Continue)
    }
}
impl RunInst for DoubleCmpG {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let v2 = v2.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;

        let res = match v2.partial_cmp(&v1) {
            Some(Ordering::Less) => -1,
            Some(Ordering::Equal) => 0,
            Some(Ordering::Greater) | None => 1,
        };
        frame.stack.push(RuntimeValuePrimitive::I32(res))?;

        Ok(RunInstValue::Continue)
    }
}

impl RunInst for MonitorEnter {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        let _object_ref = frame
            .stack
            .pop()
            .ok_or(EvalError::ExpectedStackValue)?
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?
            .expect("TODO: null poointer exception");

        tracing::warn!("MonitorEnter Not Implemented!");
        Ok(RunInstValue::Continue)
    }
}
impl RunInst for MonitorExit {
    fn run(self, RunInstArgs { frame, .. }: RunInstArgs) -> Result<RunInstValue, GeneralError> {
        let _object_ref = frame
            .stack
            .pop()
            .ok_or(EvalError::ExpectedStackValue)?
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?
            .expect("TODO: null poointer exception");

        tracing::warn!("MonitorExit Not Implemented!");
        Ok(RunInstValue::Continue)
    }
}
