use rhojvm_base::code::{
    op::{
        DoubleAdd, DoubleDivide, DoubleMultiply, DoubleNegate, DoubleRemainder, DoubleSubtract,
        DoubleToFloat, DoubleToInt, DoubleToLong, FloatAdd, FloatDivide, FloatMultiply,
        FloatNegate, FloatRemainder, FloatSub, FloatToDouble, FloatToInt, FloatToLong, IntAdd,
        IntAnd, IntArithmeticShiftRight, IntDivide, IntIncrement, IntLogicalShiftRight,
        IntMultiply, IntNegate, IntOr, IntRemainder, IntShiftLeft, IntSubtract, IntToByte,
        IntToChar, IntToDouble, IntToFloat, IntToLong, IntToShort, IntXor, LongAdd, LongAnd,
        LongArithmeticShiftRight, LongDivide, LongLogicalShiftRight, LongMultiply, LongNegate,
        LongOr, LongRemainder, LongShiftLeft, LongSubtract, LongToDouble, LongToFloat, LongToInt,
        LongXor, WideIntIncrement,
    },
    types::{JavaChar, LocalVariableIndex},
};

use crate::{eval::EvalError, rv::RuntimeValuePrimitive, GeneralError};

use super::{RunInstArgsC, RunInstContinue, RunInstContinueValue};

// === Int ===

impl RunInstContinue for IntIncrement {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let index = LocalVariableIndex::from(self.index);
        let local = frame
            .locals
            .get_mut(index)
            .ok_or(EvalError::ExpectedLocalVariable(index))?;
        let local = local
            .as_value_mut()
            .ok_or(EvalError::ExpectedLocalVariableWithValue(index))?;

        let inc = i32::from(self.increment_amount);

        let value = local
            .into_int()
            .ok_or(EvalError::ExpectedLocalVariableIntRepr(index))?;
        // Java has overflow/underflow
        let value = value.wrapping_add(inc);

        // Store the computed value into the location
        *local = RuntimeValuePrimitive::I32(value).into();

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntAdd {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let value = v2.wrapping_add(v1);

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntSubtract {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let value = v2.wrapping_sub(v1);

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntMultiply {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let value = v2.wrapping_mul(v1);

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntDivide {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        if v1 == 0 {
            todo!("Return ArithmeticException")
        }

        let value = v2.wrapping_div(v1);

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntRemainder {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        if v1 == 0 {
            todo!("Return ArithmeticException")
        }

        let value = v2.wrapping_rem(v1);

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntNegate {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        frame.stack.push(RuntimeValuePrimitive::I32(-v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntAnd {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let value = v2 & v1;

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntOr {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let value = v2 | v1;

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntXor {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let value = v2 ^ v1;

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntShiftLeft {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let value = v2 << (v1 & 0x1F);

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntArithmeticShiftRight {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let value = v2 >> v1;

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntLogicalShiftRight {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        // TODO: There are several places where we use the from_ne_bytesto_ne_bytes) for converting // between integers of different signs, but we may need to implement a special version
        // because the representation as the bytes might not be the same for all platforms
        // and we wish to match java behavior
        let v1 = u32::from_ne_bytes(
            v1.into_int()
                .ok_or(EvalError::ExpectedStackValueIntRepr)?
                .to_ne_bytes(),
        );
        let v2 = u32::from_ne_bytes(
            v2.into_int()
                .ok_or(EvalError::ExpectedStackValueIntRepr)?
                .to_ne_bytes(),
        );
        let value = v2 >> v1;
        #[allow(clippy::cast_possible_wrap)]
        let value = value as i32;

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntToFloat {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        #[allow(clippy::cast_precision_loss)]
        let v1 = v1 as f32;
        frame.stack.push(RuntimeValuePrimitive::F32(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntToDouble {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v1 = f64::from(v1);
        frame.stack.push(RuntimeValuePrimitive::F64(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntToLong {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v1 = i64::from(v1);
        frame.stack.push(RuntimeValuePrimitive::I64(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntToShort {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        #[allow(clippy::cast_possible_truncation)]
        let v1 = v1 as i16;
        frame.stack.push(RuntimeValuePrimitive::I16(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntToByte {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        #[allow(clippy::cast_possible_truncation)]
        let v1 = v1 as i8;
        frame.stack.push(RuntimeValuePrimitive::I8(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntToChar {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v1 = JavaChar::from_int(v1);
        frame.stack.push(RuntimeValuePrimitive::Char(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for WideIntIncrement {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let index = self.index;
        let local = frame
            .locals
            .get_mut(index)
            .ok_or(EvalError::ExpectedLocalVariable(index))?;
        let local = local
            .as_value_mut()
            .ok_or(EvalError::ExpectedLocalVariableWithValue(index))?;

        let inc = i32::from(self.increment_amount);

        let value = local
            .into_int()
            .ok_or(EvalError::ExpectedLocalVariableIntRepr(index))?;
        // Java has overflow/underflow
        let value = value.wrapping_add(inc);

        // Store the computed value into the location
        *local = RuntimeValuePrimitive::I32(value).into();

        Ok(RunInstContinueValue::Continue)
    }
}

// === Long ===

impl RunInstContinue for LongAdd {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let value = v2.wrapping_add(v1);

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongSubtract {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let value = v2.wrapping_sub(v1);

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongMultiply {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let value = v2.wrapping_mul(v1);

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongDivide {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        if v1 == 0 {
            todo!("Return ArithmeticException")
        }

        let value = v2.wrapping_div(v1);

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongRemainder {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        if v1 == 0 {
            todo!("Return ArithmeticException")
        }

        let value = v2.wrapping_rem(v1);

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongNegate {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        frame.stack.push(RuntimeValuePrimitive::I64(-v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongAnd {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let value = v2 & v1;

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongOr {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let value = v2 | v1;

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongXor {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let value = v2 ^ v1;

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongShiftLeft {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let value = v2 << (v1 & 0x3F);

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongArithmeticShiftRight {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?;
        let v2 = v2.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        let value = v2 >> v1;

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongLogicalShiftRight {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = u32::from_ne_bytes(
            v1.into_int()
                .ok_or(EvalError::ExpectedStackValueIntRepr)?
                .to_ne_bytes(),
        );
        let v2 = u64::from_ne_bytes(
            v2.into_i64()
                .ok_or(EvalError::ExpectedStackValueLong)?
                .to_ne_bytes(),
        );
        let value = v2 >> v1;
        #[allow(clippy::cast_possible_wrap)]
        let value = value as i64;

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongToFloat {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        #[allow(clippy::cast_precision_loss)]
        let v1 = v1 as f32;
        frame.stack.push(RuntimeValuePrimitive::F32(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongToDouble {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        #[allow(clippy::cast_precision_loss)]
        let v1 = v1 as f64;
        frame.stack.push(RuntimeValuePrimitive::F64(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongToInt {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let v1 = v1.into_i64().ok_or(EvalError::ExpectedStackValueLong)?;
        #[allow(clippy::cast_possible_truncation)]
        let v1 = v1 as i32;
        frame.stack.push(RuntimeValuePrimitive::I32(v1))?;

        Ok(RunInstContinueValue::Continue)
    }
}

// === Float ===

// TODO: Support value set conversion?
impl RunInstContinue for FloatAdd {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let v2 = v2.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let value = v2 + v1;

        frame.stack.push(RuntimeValuePrimitive::F32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatSub {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let v2 = v2.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let value = v2 - v1;

        frame.stack.push(RuntimeValuePrimitive::F32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatNegate {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let value = -v1;

        frame.stack.push(RuntimeValuePrimitive::F32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatMultiply {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let v2 = v2.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let value = v2 * v1;

        frame.stack.push(RuntimeValuePrimitive::F32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatDivide {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let v2 = v2.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let value = v2 / v1;

        frame.stack.push(RuntimeValuePrimitive::F32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatRemainder {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let v2 = v2.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        // TODO: is this the correct remainder operator for the JVM?
        let value = v2 % v1;

        frame.stack.push(RuntimeValuePrimitive::F32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatToInt {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        #[allow(clippy::cast_possible_truncation)]
        let value = if v1.is_nan() {
            0
        } else if v1.is_infinite() {
            if v1.is_sign_negative() {
                i32::MIN
            } else {
                i32::MAX
            }
        } else {
            v1.round() as i32
        };

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatToLong {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        #[allow(clippy::cast_possible_truncation)]
        let value = if v1.is_nan() {
            0
        } else if v1.is_infinite() {
            if v1.is_sign_negative() {
                i64::MIN
            } else {
                i64::MAX
            }
        } else {
            v1.round() as i64
        };

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatToDouble {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f32().ok_or(EvalError::ExpectedStackValueFloat)?;
        let value = f64::from(v1);

        frame.stack.push(RuntimeValuePrimitive::F64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}

// === Double ===

impl RunInstContinue for DoubleAdd {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let v2 = v2.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let value = v2 + v1;

        frame.stack.push(RuntimeValuePrimitive::F64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleSubtract {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let v2 = v2.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let value = v2 - v1;

        frame.stack.push(RuntimeValuePrimitive::F64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleNegate {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let value = -v1;

        frame.stack.push(RuntimeValuePrimitive::F64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleMultiply {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let v2 = v2.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let value = v2 * v1;

        frame.stack.push(RuntimeValuePrimitive::F64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleDivide {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let v2 = v2.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let value = v2 / v1;

        frame.stack.push(RuntimeValuePrimitive::F64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleRemainder {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (v1, v2) = frame.stack.pop2().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        let v2 = v2.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        // TODO: is this the correct remainder operator for the JVM?
        let value = v2 % v1;

        frame.stack.push(RuntimeValuePrimitive::F64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleToInt {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        #[allow(clippy::cast_possible_truncation)]
        let value = if v1.is_nan() {
            0
        } else if v1.is_infinite() {
            if v1.is_sign_negative() {
                i32::MIN
            } else {
                i32::MAX
            }
        } else {
            v1.round() as i32
        };

        frame.stack.push(RuntimeValuePrimitive::I32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleToLong {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        #[allow(clippy::cast_possible_truncation)]
        let value = if v1.is_nan() {
            0
        } else if v1.is_infinite() {
            if v1.is_sign_negative() {
                i64::MIN
            } else {
                i64::MAX
            }
        } else {
            v1.round() as i64
        };

        frame.stack.push(RuntimeValuePrimitive::I64(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleToFloat {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let v1 = v1.into_f64().ok_or(EvalError::ExpectedStackValueDouble)?;
        // TODO: Is this conversion correct? JVM wants values too small/large from f64 to be
        // represented as infinities in f32
        #[allow(clippy::cast_possible_truncation)]
        let value = v1 as f32;

        frame.stack.push(RuntimeValuePrimitive::F32(value))?;

        Ok(RunInstContinueValue::Continue)
    }
}
