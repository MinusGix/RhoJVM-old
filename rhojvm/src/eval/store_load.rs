//! Implementations for functions which store/load data

use classfile_parser::{
    constant_info::{ConstantInfo, FieldRefConstant},
    constant_pool::ConstantPoolIndexRaw,
};

use rhojvm_base::{
    code::{
        op::{
            AALoad, AAStore, AConstNull, ALoad, ALoad0, ALoad1, ALoad2, ALoad3, AStore, AStore0,
            AStore1, AStore2, AStore3, ArrayLength, ByteArrayLoad, ByteArrayStore, CharArrayLoad,
            CharArrayStore, DoubleArrayLoad, DoubleArrayStore, DoubleConst0, DoubleConst1,
            DoubleLoad, DoubleLoad0, DoubleLoad1, DoubleLoad2, DoubleLoad3, DoubleStore,
            DoubleStore0, DoubleStore1, DoubleStore2, DoubleStore3, Dup, Dup2, Dup2X1, Dup2X2,
            DupX1, DupX2, FloatArrayLoad, FloatArrayStore, FloatConst0, FloatConst1, FloatConst2,
            FloatLoad, FloatLoad0, FloatLoad1, FloatLoad2, FloatLoad3, FloatStore, FloatStore0,
            FloatStore1, FloatStore2, FloatStore3, GetField, GetStatic, IConstNeg1, IntALoad,
            IntArrayStore, IntConst0, IntConst1, IntConst2, IntConst3, IntConst4, IntConst5,
            IntLoad, IntLoad0, IntLoad1, IntLoad2, IntLoad3, IntStore, IntStore0, IntStore1,
            IntStore2, IntStore3, LoadConstant, LoadConstant2Wide, LoadConstantWide, LongArrayLoad,
            LongArrayStore, LongConst0, LongConst1, LongLoad, LongLoad0, LongLoad1, LongLoad2,
            LongLoad3, LongStore, LongStore0, LongStore1, LongStore2, LongStore3, Pop, Pop2,
            PushByte, PushShort, PutField, PutStaticField, ShortArrayLoad, ShortArrayStore,
            WideIntLoad,
        },
        types::{JavaChar, LocalVariableIndex},
    },
    data::{
        class_files::ClassFiles,
        class_names::ClassNames,
        classes::{load_super_classes_iter, Classes},
    },
    id::ClassId,
    package::Packages,
};
use usize_cast::IntoUsize;

use crate::{
    class_instance::{FieldId, FieldType, ReferenceInstance, StaticClassInstance},
    gc::GcRef,
    initialize_class,
    rv::{RuntimeType, RuntimeTypePrimitive, RuntimeValue, RuntimeValuePrimitive},
    util::{self, find_field_with_name, Env},
    GeneralError, State,
};

use super::{
    EvalError, Frame, RunInstArgsC, RunInstContinue, RunInstContinueValue, ValueException,
};

enum DestRes {
    GcRef((GcRef<StaticClassInstance>, FieldId, FieldRefConstant)),
    RunInstContinue(RunInstContinueValue),
}
fn get_field_dest(
    env: &mut Env,
    index: ConstantPoolIndexRaw<FieldRefConstant>,
    class_id: ClassId,
) -> Result<DestRes, GeneralError> {
    let class_file = env
        .class_files
        .get(&class_id)
        .ok_or(EvalError::MissingMethodClassFile(class_id))?;

    let field = class_file
        .get_t(index)
        .ok_or(EvalError::InvalidConstantPoolIndex(index.into_generic()))?
        .clone();

    let dest_class =
        class_file
            .get_t(field.class_index)
            .ok_or(EvalError::InvalidConstantPoolIndex(
                field.class_index.into_generic(),
            ))?;

    let dest_class_id =
        class_file
            .get_text_b(dest_class.name_index)
            .ok_or(EvalError::InvalidConstantPoolIndex(
                dest_class.name_index.into_generic(),
            ))?;
    let dest_class_id = env.class_names.gcid_from_bytes(dest_class_id);

    // TODO: Check the begun status
    // Initialize the target class, since we're going to need to get a field from it
    // and so it has to be all initialized before we can do that
    let status = initialize_class(env, dest_class_id)?;
    let dest_ref = match status.into_value() {
        ValueException::Value(dest) => dest,
        ValueException::Exception(exc) => {
            return Ok(DestRes::RunInstContinue(RunInstContinueValue::Exception(
                exc,
            )))
        }
    };

    // TODO: Document assumption that fields stay in order
    let mut iter = load_super_classes_iter(dest_class_id);
    while let Some(dest_id) = iter.next_item(
        &mut env.class_names,
        &mut env.class_files,
        &mut env.classes,
        &mut env.packages,
    ) {
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;
        let field_nat = class_file.get_t(field.name_and_type_index).ok_or(
            EvalError::InvalidConstantPoolIndex(field.name_and_type_index.into_generic()),
        )?;
        let field_name = class_file.get_text_b(field_nat.name_index).ok_or(
            EvalError::InvalidConstantPoolIndex(field_nat.name_index.into_generic()),
        )?;

        let dest_id = dest_id?;
        let field_id = find_field_with_name(&env.class_files, dest_id, field_name)?;

        if let Some((field_id, _)) = field_id {
            // TODO: dest_ref isn't accurate! it should probably be the destination that the field
            // is actually on!
            return Ok(DestRes::GcRef((dest_ref, field_id, field)));
        }
    }

    todo!("Field not found exception")
}

/// Theoretically, this shouldn't error since it would've been checked by stack map verifier
/// already.
fn convert_field_type_store(
    class_names: &mut ClassNames,
    class_files: &mut ClassFiles,
    classes: &mut Classes,
    packages: &mut Packages,
    state: &mut State,
    dest: FieldType,
    src: RuntimeValue,
) -> Result<RuntimeValue, GeneralError> {
    Ok(match (dest, src) {
        // TODO: This might be more lenient than it should be?
        // A lot of these automatic shrinking/expanding integer casts are due to them being
        // represented as the same on the stack in typical jvm, but we keep what they are,
        // and so we have to properly narrow/expand them,
        // but that doesn't apply to i64
        #[allow(clippy::cast_possible_truncation, clippy::match_same_arms)]
        (RuntimeType::Primitive(lprim), RuntimeValue::Primitive(rprim)) => match (lprim, rprim) {
            (RuntimeTypePrimitive::I64, RuntimeValuePrimitive::I64(_)) => src,
            // TODO: Do other values get cast automatically into i64? probably not
            // (RuntimeTypePrimitive::I64, RuntimeValuePrimitive::I32(_)) => todo!(),
            // (RuntimeTypePrimitive::I64, RuntimeValuePrimitive::I16(_)) => todo!(),
            // (RuntimeTypePrimitive::I64, RuntimeValuePrimitive::I8(_)) => todo!(),
            // (RuntimeTypePrimitive::I64, RuntimeValuePrimitive::Char(_)) => todo!(),
            // (RuntimeTypePrimitive::I64, RuntimeValuePrimitive::Bool(_)) => todo!(),

            // (RuntimeTypePrimitive::I32, RuntimeValuePrimitive::I64(_)) => todo!(),
            (RuntimeTypePrimitive::I32, RuntimeValuePrimitive::I32(_)) => src,
            (RuntimeTypePrimitive::I32, RuntimeValuePrimitive::I16(x)) => {
                RuntimeValuePrimitive::I32(x.into()).into()
            }
            (RuntimeTypePrimitive::I32, RuntimeValuePrimitive::I8(x)) => {
                RuntimeValuePrimitive::I32(x.into()).into()
            }
            // (RuntimeTypePrimitive::I32, RuntimeValuePrimitive::F32(_)) => todo!(),
            // (RuntimeTypePrimitive::I32, RuntimeValuePrimitive::F64(_)) => todo!(),
            (RuntimeTypePrimitive::I32, RuntimeValuePrimitive::Char(x)) => {
                RuntimeValuePrimitive::I32(x.as_int()).into()
            }
            (RuntimeTypePrimitive::I32, RuntimeValuePrimitive::Bool(x)) => {
                RuntimeValuePrimitive::I32(i32::from(x)).into()
            }
            // (RuntimeTypePrimitive::I16, RuntimeValuePrimitive::I64(_)) => todo!(),
            (RuntimeTypePrimitive::I16, RuntimeValuePrimitive::I32(x)) => {
                RuntimeValuePrimitive::I16(x as i16).into()
            }
            (RuntimeTypePrimitive::I16, RuntimeValuePrimitive::I16(_)) => src,
            (RuntimeTypePrimitive::I16, RuntimeValuePrimitive::I8(x)) => {
                RuntimeValuePrimitive::I16(i16::from(x)).into()
            }
            // (RuntimeTypePrimitive::I16, RuntimeValuePrimitive::F32(_)) => todo!(),
            // (RuntimeTypePrimitive::I16, RuntimeValuePrimitive::F64(_)) => todo!(),
            (RuntimeTypePrimitive::I16, RuntimeValuePrimitive::Char(x)) => {
                RuntimeValuePrimitive::I16(x.as_i16()).into()
            }
            (RuntimeTypePrimitive::I16, RuntimeValuePrimitive::Bool(x)) => {
                RuntimeValuePrimitive::I16(i16::from(x)).into()
            }
            // (RuntimeTypePrimitive::I8, RuntimeValuePrimitive::I64(_)) => todo!(),
            (RuntimeTypePrimitive::I8, RuntimeValuePrimitive::I32(x)) => {
                RuntimeValuePrimitive::I8(x as i8).into()
            }
            (RuntimeTypePrimitive::I8, RuntimeValuePrimitive::I16(x)) => {
                RuntimeValuePrimitive::I8(x as i8).into()
            }
            (RuntimeTypePrimitive::I8, RuntimeValuePrimitive::I8(_)) => src,
            (RuntimeTypePrimitive::I8, RuntimeValuePrimitive::Char(x)) => {
                RuntimeValuePrimitive::I8(x.as_i16() as i8).into()
            }
            (RuntimeTypePrimitive::I8, RuntimeValuePrimitive::Bool(x)) => {
                RuntimeValuePrimitive::I8(x as i8).into()
            }
            (RuntimeTypePrimitive::Bool, RuntimeValuePrimitive::I32(x)) => {
                RuntimeValuePrimitive::Bool((x != 0).into()).into()
            }
            (RuntimeTypePrimitive::Bool, RuntimeValuePrimitive::I16(x)) => {
                RuntimeValuePrimitive::Bool((x != 0).into()).into()
            }
            (RuntimeTypePrimitive::Bool, RuntimeValuePrimitive::I8(x)) => {
                RuntimeValuePrimitive::Bool((x != 0).into()).into()
            }
            (RuntimeTypePrimitive::Bool, RuntimeValuePrimitive::Char(x)) => {
                RuntimeValuePrimitive::Bool((x.as_i16() != 0).into()).into()
            }
            (RuntimeTypePrimitive::Bool, RuntimeValuePrimitive::Bool(_)) => src,
            // TODO: Do floats get any automatic conversion from integers or f64?
            (RuntimeTypePrimitive::F32, RuntimeValuePrimitive::F32(_)) => src,
            // (RuntimeTypePrimitive::F32, RuntimeValuePrimitive::F64(_)) => todo!(),// (RuntimeTypePrimitive::F64, RuntimeValuePrimitive::F32(_)) => todo!(),
            (RuntimeTypePrimitive::F64, RuntimeValuePrimitive::F64(_)) => src,
            (RuntimeTypePrimitive::Char, RuntimeValuePrimitive::I32(x)) => {
                RuntimeValuePrimitive::Char(JavaChar::from_int(x)).into()
            }
            (RuntimeTypePrimitive::Char, RuntimeValuePrimitive::I16(x)) => {
                RuntimeValuePrimitive::Char(JavaChar::from_int(x.into())).into()
            }
            (RuntimeTypePrimitive::Char, RuntimeValuePrimitive::I8(x)) => {
                RuntimeValuePrimitive::Char(JavaChar::from_int(x.into())).into()
            }
            (RuntimeTypePrimitive::Char, RuntimeValuePrimitive::Char(_)) => src,
            (RuntimeTypePrimitive::Char, RuntimeValuePrimitive::Bool(x)) => {
                RuntimeValuePrimitive::Char(JavaChar::from_int(x.into())).into()
            }
            _ => todo!("Error"),
        },
        // TODO: Does the JVM autocast Integer into an int?
        // (RuntimeType::Primitive(_), RuntimeValue::Reference(_)) => todo!(),
        // (RuntimeType::Reference(_), RuntimeValue::Primitive(_)) => todo!(),

        // Storing a null pointer is allowed
        (RuntimeType::Reference(_), RuntimeValue::NullReference) => src,
        (RuntimeType::Reference(id), RuntimeValue::Reference(src_ref)) => {
            // TODO: We could probably do a bit less work if we condition on it being an array or
            // a normal reference
            let src = state
                .gc
                .deref(src_ref)
                .ok_or(EvalError::InvalidGcRef(src_ref.into_generic()))?;
            let instance_id = src.instanceof();

            let is_castable = instance_id == id
                || classes.is_super_class(class_names, class_files, packages, instance_id, id)?
                || classes.implements_interface(class_names, class_files, instance_id, id)?
                || classes.is_castable_array(
                    class_names,
                    class_files,
                    packages,
                    instance_id,
                    id,
                )?;

            if is_castable {
                RuntimeValue::Reference(src_ref)
            } else {
                tracing::warn!(
                    "Failed to cast {:?} to {:?}",
                    class_names.tpath(src.instanceof()),
                    class_names.tpath(id)
                );
                todo!("Type was not castable")
            }
        }
        _ => panic!(
            "TODO: Type failed to be casted: dest: {:?}, src: {:?}",
            dest, src
        ),
    })
}

impl RunInstContinue for GetStatic {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (class_id, _) = method_id.decompose();

        let (dest_ref, dest_field_index, _) = match get_field_dest(env, self.index, class_id)? {
            DestRes::GcRef(v) => v,
            // Probably threw an exception
            DestRes::RunInstContinue(v) => return Ok(v),
        };

        let dest_instance = env
            .state
            .gc
            .deref(dest_ref)
            .ok_or(EvalError::InvalidGcRef(dest_ref.into_generic()))?;
        let field = dest_instance.fields.get(dest_field_index);

        if let Some(field) = field {
            // TODO: JVM says it throws incompatible class change error if the resolved field is not
            // a static class field, but by definition, this field is static
            // does that mean that in the case where there is no such field, we need to check if
            // that field exists on instances of the given class to produce the proper error?

            let field_value = field.value();
            frame.stack.push(field_value)?;
        } else {
            todo!("Return no such field exception")
        }

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for PutStaticField {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (class_id, _) = method_id.decompose();

        // Get the value we are storing to the field
        // Put static field works for any category of type
        let value = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let (dest_ref, dest_field_index, _) = match get_field_dest(env, self.index, class_id)? {
            DestRes::GcRef(v) => v,
            // Probably threw an exception
            DestRes::RunInstContinue(v) => return Ok(v),
        };

        let dest_instance = env
            .state
            .gc
            .deref(dest_ref)
            .ok_or(EvalError::InvalidGcRef(dest_ref.into_generic()))?;
        let field = dest_instance.fields.get(dest_field_index);

        if let Some(field) = field {
            let field_type = field.typ();

            // TODO: Some of the errors should be exceptions
            let field_value = convert_field_type_store(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.classes,
                &mut env.packages,
                &mut env.state,
                field_type,
                value,
            )?;

            // The gcref should still exist, and the field should still exist
            let dest_instance = env.state.gc.deref_mut(dest_ref).unwrap();
            let field = dest_instance.fields.get_mut(dest_field_index).unwrap();

            *field.value_mut() = field_value;
        } else {
            todo!("Return no such field exception")
        }

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for GetField {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (class_id, _) = method_id.decompose();

        let instance_ref = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let instance_ref = instance_ref
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?;
        let instance_ref = match instance_ref {
            Some(instance_ref) => instance_ref,
            None => todo!("Return null pointer exception"),
        };

        let (_, dest_field_id, _) = match get_field_dest(env, self.index, class_id)? {
            DestRes::GcRef(v) => v,
            // Probably an exception
            DestRes::RunInstContinue(v) => return Ok(v),
        };

        let instance = env
            .state
            .gc
            .deref(instance_ref)
            .ok_or(EvalError::InvalidGcRef(instance_ref.into_generic()))?;
        match instance {
            ReferenceInstance::Class(_class) => {
                // TODO: Check that it is the right class instance!
            }
            ReferenceInstance::Thread(_thread) => {
                // TODO: Check that it is correct class instance!
            }
            ReferenceInstance::StaticForm(_class) => {
                // TODO: Check that it is correct class instance!
            }
            ReferenceInstance::MethodHandle(_class) => {
                // TODO: Check that it is correct class instance!
            }
            ReferenceInstance::MethodHandleInfo(_class) => {
                // TODO: Check that it is correct class instance!
            }
            ReferenceInstance::PrimitiveArray(_) => todo!(),
            ReferenceInstance::ReferenceArray(_) => todo!(),
        }

        let instance = env
            .state
            .gc
            .deref(instance_ref)
            .ok_or(EvalError::InvalidGcRef(instance_ref.into_generic()))?;
        let field = instance
            .fields()
            .find(|x| x.0 == dest_field_id)
            .map(|x| x.1);

        if let Some(field) = field {
            let field_value = field.value();
            frame.stack.push(field_value)?;
        } else {
            todo!("Return no such field exception");
        }

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for PutField {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (class_id, _) = method_id.decompose();

        // Get the value we are storing to the field
        // Put static field works for any category of type
        let value = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        let instance_ref = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let instance_ref = instance_ref
            .into_reference()
            .ok_or(EvalError::ExpectedStackValueReference)?;
        let instance_ref = if let Some(instance_ref) = instance_ref {
            instance_ref
        } else {
            todo!("Null Pointer Exception")
        };

        let (_, dest_field_index, _) = match get_field_dest(env, self.index, class_id)? {
            DestRes::GcRef(v) => v,
            // Probably threw an exception
            DestRes::RunInstContinue(v) => return Ok(v),
        };

        let dest_instance = env
            .state
            .gc
            .deref(instance_ref)
            .ok_or(EvalError::InvalidGcRef(instance_ref.into_generic()))?;
        let dest_instance_fields = if let Some(fields) = dest_instance.get_class_fields() {
            fields
        } else {
            // You can't put field on an array, but it doesn't detail an exception for that.
            // Probably illegal access error?
            todo!("Some exception here");
        };
        let field = dest_instance_fields.get(dest_field_index);

        if let Some(field) = field {
            let field_type = field.typ();

            // TODO: Some of the errors should be exceptions
            let field_value = convert_field_type_store(
                &mut env.class_names,
                &mut env.class_files,
                &mut env.classes,
                &mut env.packages,
                &mut env.state,
                field_type,
                value,
            )?;

            // The gcref should still exist, and the field should still exist
            let dest_instance = env.state.gc.deref_mut(instance_ref).unwrap();
            let dest_instance_fields = dest_instance.get_class_fields_mut().unwrap();
            let field = dest_instance_fields.get_mut(dest_field_index).unwrap();

            *field.value_mut() = field_value;
        } else {
            todo!("Return no such field exception")
        }

        Ok(RunInstContinueValue::Continue)
    }
}

fn load_constant(
    RunInstArgsC {
        env,
        method_id,
        frame,
        ..
    }: RunInstArgsC,
    index: ConstantPoolIndexRaw<ConstantInfo>,
) -> Result<RunInstContinueValue, GeneralError> {
    let (class_id, _) = method_id.decompose();
    let class_file = env
        .class_files
        .get(&class_id)
        .ok_or(EvalError::MissingMethodClassFile(class_id))?;

    let info = class_file
        .get_t(index)
        .ok_or(EvalError::InvalidConstantPoolIndex(index.into_generic()))?;

    match info {
        ConstantInfo::Integer(v) => frame.stack.push(RuntimeValuePrimitive::I32(v.value))?,
        ConstantInfo::Float(v) => frame.stack.push(RuntimeValuePrimitive::F32(v.value))?,
        ConstantInfo::Class(class) => {
            let target_class_name = class_file.get_text_b(class.name_index).ok_or(
                EvalError::InvalidConstantPoolIndex(class.name_index.into_generic()),
            )?;
            let target_class_id = env.class_names.gcid_from_bytes(target_class_name);

            let class_form = util::make_class_form_of(env, class_id, target_class_id)?;
            let class_form = match class_form {
                ValueException::Value(class_form) => class_form,
                ValueException::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
            };

            frame
                .stack
                .push(RuntimeValue::Reference(class_form.into_generic()))?;
        }
        ConstantInfo::String(string) => {
            // TODO: This conversion could go directly from cesu8 to utf16
            let string = class_file.get_text_t(string.string_index).ok_or(
                EvalError::InvalidConstantPoolIndex(string.string_index.into_generic()),
            )?;

            let string_ref = {
                let char_arr = string
                    .encode_utf16()
                    .map(|x| RuntimeValuePrimitive::Char(JavaChar(x)))
                    .collect::<Vec<_>>();

                util::construct_string(env, char_arr, true)?
            };
            match string_ref {
                ValueException::Value(string_ref) => frame
                    .stack
                    .push(RuntimeValue::Reference(string_ref.into_generic()))?,
                ValueException::Exception(exc) => return Ok(RunInstContinueValue::Exception(exc)),
            }
        }
        ConstantInfo::MethodHandle(_method_handle) => {
            todo!()
        }
        ConstantInfo::MethodType(_method_type) => {
            todo!()
        }
        _ => return Err(EvalError::InvalidConstantPoolIndex(index.into_generic()).into()),
    };

    Ok(RunInstContinueValue::Continue)
}
impl RunInstContinue for LoadConstant {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        load_constant(args, self.index)
    }
}
impl RunInstContinue for LoadConstantWide {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        load_constant(args, self.index)
    }
}
impl RunInstContinue for LoadConstant2Wide {
    fn run(
        self,
        RunInstArgsC {
            env,
            method_id,
            frame,
            ..
        }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let (class_id, _) = method_id.decompose();
        let class_file = env
            .class_files
            .get(&class_id)
            .ok_or(EvalError::MissingMethodClassFile(class_id))?;

        let info = class_file
            .get_t(self.index)
            .ok_or(EvalError::InvalidConstantPoolIndex(
                self.index.into_generic(),
            ))?;

        match info {
            ConstantInfo::Long(x) => frame.stack.push(RuntimeValuePrimitive::I64(x.value))?,
            ConstantInfo::Double(x) => frame.stack.push(RuntimeValuePrimitive::F64(x.value))?,
            _ => return Err(EvalError::InvalidConstantPoolIndex(self.index.into_generic()).into()),
        }

        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for Pop {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let val = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if val.is_category_2() {
            return Err(EvalError::ExpectedStackValueCategory1.into());
        }

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for Pop2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let val = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if val.is_category_2() {
            return Ok(RunInstContinueValue::Continue);
        }

        let val2 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if val2.is_category_2() {
            return Err(EvalError::ExpectedStackValueCategory1.into());
        }

        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for PushByte {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I8(self.val))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for PushShort {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I16(self.val))?;
        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for AConstNull {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValue::NullReference)?;
        Ok(RunInstContinueValue::Continue)
    }
}

// === Reference Load ===

fn aload_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let local = frame
        .locals
        .get(index)
        .ok_or(EvalError::ExpectedLocalVariable(index))?;
    let local = local
        .as_value()
        .ok_or(EvalError::ExpectedLocalVariableWithValue(index))?;
    let local = *local;
    if local.is_reference() {
        frame.stack.push(local)?;
        Ok(RunInstContinueValue::Continue)
    } else {
        Err(EvalError::ExpectedLocalVariableReference(index).into())
    }
}

impl RunInstContinue for ALoad {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        aload_index(frame, self.index.into())
    }
}
impl RunInstContinue for ALoad0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        aload_index(frame, 0)
    }
}
impl RunInstContinue for ALoad1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        aload_index(frame, 1)
    }
}
impl RunInstContinue for ALoad2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        aload_index(frame, 2)
    }
}
impl RunInstContinue for ALoad3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        aload_index(frame, 3)
    }
}

// === Reference Store ===

fn astore_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
    if !object.is_reference() {
        return Err(EvalError::ExpectedStackValueReference.into());
    }

    frame.locals.set_value_at(index, object);

    Ok(RunInstContinueValue::Continue)
}
impl RunInstContinue for AStore {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        astore_index(frame, self.index.into())
    }
}
impl RunInstContinue for AStore0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        astore_index(frame, 0)
    }
}
impl RunInstContinue for AStore1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        astore_index(frame, 1)
    }
}
impl RunInstContinue for AStore2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        astore_index(frame, 2)
    }
}
impl RunInstContinue for AStore3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        astore_index(frame, 3)
    }
}

// === Dup Instructions ===

impl RunInstContinue for Dup {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let v = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        frame.stack.push(v)?;
        frame.stack.push(v)?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for Dup2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let value1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        if value1.is_category_2() {
            // Only one category 2 type is popped
            frame.stack.push(value1)?;
            frame.stack.push(value1)?;
        } else {
            let value2 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
            assert!(!value2.is_category_2());
            frame.stack.push(value2)?;
            frame.stack.push(value1)?;
            frame.stack.push(value2)?;
            frame.stack.push(value1)?;
        }

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DupX1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let value1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let value2 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        // verifier assures that these aren't category 2 types
        debug_assert!(!value1.is_category_2());
        debug_assert!(!value2.is_category_2());

        frame.stack.push(value1)?;
        frame.stack.push(value2)?;
        frame.stack.push(value1)?;

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DupX2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let value1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let value2 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        if value1.is_category_2() {
            // ..., value1, value2, value1
            frame.stack.push(value1)?;
            frame.stack.push(value2)?;
            frame.stack.push(value1)?;
        } else {
            // ..., value2, value1, value3, value2, value1
            let value3 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
            assert!(!value2.is_category_2());
            assert!(!value3.is_category_2());
            frame.stack.push(value1)?;
            frame.stack.push(value3)?;
            frame.stack.push(value2)?;
            frame.stack.push(value1)?;
        }

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for Dup2X1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let value1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let value2 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        if value1.is_category_2() {
            // ..., value1, value2, value1
            frame.stack.push(value1)?;
            frame.stack.push(value2)?;
            frame.stack.push(value1)?;
        } else {
            // ..., value2, value1, value3, value2, value1
            let value3 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
            assert!(!value2.is_category_2());
            assert!(!value3.is_category_2());
            frame.stack.push(value2)?;
            frame.stack.push(value1)?;
            frame.stack.push(value3)?;
            frame.stack.push(value2)?;
            frame.stack.push(value1)?;
        }

        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for Dup2X2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let value1 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let value2 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;

        if value1.is_category_2() {
            if value2.is_category_2() {
                // ..., value1, value2, value1
                frame.stack.push(value1)?;
                frame.stack.push(value2)?;
                frame.stack.push(value1)?;
            } else {
                // ..., value2, value1, value3, value2, value1
                let value3 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
                assert!(!value3.is_category_2());
                frame.stack.push(value1)?;
                frame.stack.push(value3)?;
                frame.stack.push(value2)?;
                frame.stack.push(value1)?;
            }
        } else {
            let value3 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
            assert!(!value2.is_category_2());
            if value3.is_category_2() {
                // ..., value2, value1, value3, value2, value1
                frame.stack.push(value2)?;
                frame.stack.push(value1)?;
                frame.stack.push(value3)?;
                frame.stack.push(value2)?;
                frame.stack.push(value1)?;
            } else {
                let value4 = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
                assert!(!value4.is_category_2());
                // ..., value2, value1, value4, value3, value2, value1
                frame.stack.push(value2)?;
                frame.stack.push(value1)?;
                frame.stack.push(value4)?;
                frame.stack.push(value3)?;
                frame.stack.push(value2)?;
                frame.stack.push(value1)?;
            }
        }

        Ok(RunInstContinueValue::Continue)
    }
}

// === Int Constants ===

impl RunInstContinue for IConstNeg1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I32(-1))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntConst0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I32(0))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntConst1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I32(1))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntConst2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I32(2))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntConst3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I32(3))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntConst4 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I32(4))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for IntConst5 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I32(5))?;
        Ok(RunInstContinueValue::Continue)
    }
}

// === Int Load ===

fn intload_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let local = frame
        .locals
        .get(index)
        .ok_or(EvalError::ExpectedLocalVariable(index))?;
    let local = local
        .as_value()
        .ok_or(EvalError::ExpectedLocalVariableWithValue(index))?;
    let local = *local;
    if local.can_be_int() {
        frame.stack.push(local)?;
        Ok(RunInstContinueValue::Continue)
    } else {
        tracing::info!("Value: {:?}", local);
        Err(EvalError::ExpectedLocalVariableIntRepr(index).into())
    }
}

impl RunInstContinue for IntLoad {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intload_index(frame, self.index.into())
    }
}
impl RunInstContinue for IntLoad0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intload_index(frame, 0)
    }
}
impl RunInstContinue for IntLoad1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intload_index(frame, 1)
    }
}
impl RunInstContinue for IntLoad2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intload_index(frame, 2)
    }
}
impl RunInstContinue for IntLoad3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intload_index(frame, 3)
    }
}

impl RunInstContinue for WideIntLoad {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intload_index(frame, self.index)
    }
}

// === Int Store ===

fn intstore_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
    if !object.can_be_int() {
        return Err(EvalError::ExpectedStackValueIntRepr.into());
    }

    // We don't convert it to an integer here
    frame.locals.set_value_at(index, object);

    Ok(RunInstContinueValue::Continue)
}
impl RunInstContinue for IntStore {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intstore_index(frame, self.index.into())
    }
}
impl RunInstContinue for IntStore0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intstore_index(frame, 0)
    }
}
impl RunInstContinue for IntStore1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intstore_index(frame, 1)
    }
}
impl RunInstContinue for IntStore2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intstore_index(frame, 2)
    }
}
impl RunInstContinue for IntStore3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        intstore_index(frame, 3)
    }
}

// === Float Load ===

fn floatload_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let local = frame
        .locals
        .get(index)
        .ok_or(EvalError::ExpectedLocalVariable(index))?;
    let local = local
        .as_value()
        .ok_or(EvalError::ExpectedLocalVariableWithValue(index))?;
    let local = *local;
    if local.is_float() {
        frame.stack.push(local)?;
        Ok(RunInstContinueValue::Continue)
    } else {
        Err(EvalError::ExpectedLocalVariableFloat(index).into())
    }
}

impl RunInstContinue for FloatLoad {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatload_index(frame, self.index.into())
    }
}
impl RunInstContinue for FloatLoad0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatload_index(frame, 0)
    }
}
impl RunInstContinue for FloatLoad1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatload_index(frame, 1)
    }
}
impl RunInstContinue for FloatLoad2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatload_index(frame, 2)
    }
}
impl RunInstContinue for FloatLoad3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatload_index(frame, 3)
    }
}

// === Float Store ===

fn floatstore_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
    if !object.is_float() {
        return Err(EvalError::ExpectedStackValueFloat.into());
    }

    // We don't convert it to an integer here
    frame.locals.set_value_at(index, object);

    Ok(RunInstContinueValue::Continue)
}
impl RunInstContinue for FloatStore {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatstore_index(frame, self.index.into())
    }
}
impl RunInstContinue for FloatStore0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatstore_index(frame, 0)
    }
}
impl RunInstContinue for FloatStore1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatstore_index(frame, 1)
    }
}
impl RunInstContinue for FloatStore2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatstore_index(frame, 2)
    }
}
impl RunInstContinue for FloatStore3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        floatstore_index(frame, 3)
    }
}

// === Float Const ===

impl RunInstContinue for FloatConst0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::F32(0.0))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatConst1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::F32(1.0))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for FloatConst2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::F32(2.0))?;
        Ok(RunInstContinueValue::Continue)
    }
}

// === Long Load ===

fn longload_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let local = frame
        .locals
        .get(index)
        .ok_or(EvalError::ExpectedLocalVariable(index))?;
    let local = local
        .as_value()
        .ok_or(EvalError::ExpectedLocalVariableWithValue(index))?;
    let local = *local;
    if local.is_long() {
        frame.stack.push(local)?;
        Ok(RunInstContinueValue::Continue)
    } else {
        Err(EvalError::ExpectedLocalVariableLong(index).into())
    }
}

impl RunInstContinue for LongLoad {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longload_index(frame, self.index.into())
    }
}
impl RunInstContinue for LongLoad0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longload_index(frame, 0)
    }
}
impl RunInstContinue for LongLoad1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longload_index(frame, 1)
    }
}
impl RunInstContinue for LongLoad2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longload_index(frame, 2)
    }
}
impl RunInstContinue for LongLoad3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longload_index(frame, 3)
    }
}

// === Long Store ===

fn longstore_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
    if !object.is_long() {
        return Err(EvalError::ExpectedStackValueLong.into());
    }

    // We don't convert it to an integer here
    frame.locals.set_value_at(index, object);

    Ok(RunInstContinueValue::Continue)
}
impl RunInstContinue for LongStore {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longstore_index(frame, self.index.into())
    }
}
impl RunInstContinue for LongStore0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longstore_index(frame, 0)
    }
}
impl RunInstContinue for LongStore1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longstore_index(frame, 1)
    }
}
impl RunInstContinue for LongStore2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longstore_index(frame, 2)
    }
}
impl RunInstContinue for LongStore3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        longstore_index(frame, 3)
    }
}

// === Long Const ===

impl RunInstContinue for LongConst0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I64(0))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for LongConst1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::I64(1))?;
        Ok(RunInstContinueValue::Continue)
    }
}

// === Double Load ===

fn doubleload_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let local = frame
        .locals
        .get(index)
        .ok_or(EvalError::ExpectedLocalVariable(index))?;
    let local = local
        .as_value()
        .ok_or(EvalError::ExpectedLocalVariableWithValue(index))?;
    let local = *local;
    if local.is_double() {
        frame.stack.push(local)?;
        Ok(RunInstContinueValue::Continue)
    } else {
        Err(EvalError::ExpectedLocalVariableDouble(index).into())
    }
}

impl RunInstContinue for DoubleLoad {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doubleload_index(frame, self.index.into())
    }
}
impl RunInstContinue for DoubleLoad0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doubleload_index(frame, 0)
    }
}
impl RunInstContinue for DoubleLoad1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doubleload_index(frame, 1)
    }
}
impl RunInstContinue for DoubleLoad2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doubleload_index(frame, 2)
    }
}
impl RunInstContinue for DoubleLoad3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doubleload_index(frame, 3)
    }
}

// === Double Store ===

fn doublestore_index(
    frame: &mut Frame,
    index: LocalVariableIndex,
) -> Result<RunInstContinueValue, GeneralError> {
    let object = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
    if !object.is_double() {
        return Err(EvalError::ExpectedStackValueDouble.into());
    }

    // We don't convert it to an integer here
    frame.locals.set_value_at(index, object);

    Ok(RunInstContinueValue::Continue)
}
impl RunInstContinue for DoubleStore {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doublestore_index(frame, self.index.into())
    }
}
impl RunInstContinue for DoubleStore0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doublestore_index(frame, 0)
    }
}
impl RunInstContinue for DoubleStore1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doublestore_index(frame, 1)
    }
}
impl RunInstContinue for DoubleStore2 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doublestore_index(frame, 2)
    }
}
impl RunInstContinue for DoubleStore3 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        doublestore_index(frame, 3)
    }
}

// === Double Const ===

impl RunInstContinue for DoubleConst0 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::F64(0.0))?;
        Ok(RunInstContinueValue::Continue)
    }
}
impl RunInstContinue for DoubleConst1 {
    fn run(
        self,
        RunInstArgsC { frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        frame.stack.push(RuntimeValuePrimitive::F64(1.0))?;
        Ok(RunInstContinueValue::Continue)
    }
}

// === Reference[] ===

impl RunInstContinue for ArrayLength {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let array_ref = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let array_ref = match array_ref {
            RuntimeValue::Reference(x) => x,
            RuntimeValue::NullReference => todo!("Return NullPointerException"),
            RuntimeValue::Primitive(_) => return Err(EvalError::ExpectedStackValueReference.into()),
        };
        let array_inst = env
            .state
            .gc
            .deref(array_ref)
            .ok_or(EvalError::InvalidGcRef(array_ref.into_generic()))?;
        let len = match array_inst {
            ReferenceInstance::PrimitiveArray(array) => array.len(),
            ReferenceInstance::ReferenceArray(array) => array.len(),
            _ => return Err(EvalError::ExpectedArrayInstance.into()),
        };
        frame.stack.push(RuntimeValuePrimitive::I32(len))?;
        Ok(RunInstContinueValue::Continue)
    }
}

fn array_load(
    state: &mut State,
    frame: &mut Frame,
    element_type: impl Into<RuntimeType> + Copy,
) -> Result<RunInstContinueValue, GeneralError> {
    let elem: RuntimeType = element_type.into();
    array_load_filter(state, frame, element_type, |x| RuntimeType::from(x) == elem)
}

fn array_load_filter(
    state: &mut State,
    frame: &mut Frame,
    element_type: impl Into<RuntimeType>,
    is_valid: impl FnOnce(RuntimeTypePrimitive) -> bool,
) -> Result<RunInstContinueValue, GeneralError> {
    let element_type: RuntimeType = element_type.into();

    let index = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
    let index = index
        .into_int()
        .ok_or(EvalError::ExpectedStackValueIntRepr)?;
    // TODO: Is it correct to treat it as a u32?
    let index = u32::from_ne_bytes(index.to_ne_bytes());
    let index = index.into_usize();

    let array_ref = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
    let array_ref = match array_ref {
        RuntimeValue::Reference(x) => x,
        RuntimeValue::NullReference => todo!("Return NullPointerException"),
        RuntimeValue::Primitive(_) => return Err(EvalError::ExpectedStackValueReference.into()),
    };
    let array_inst = state
        .gc
        .deref(array_ref)
        .ok_or(EvalError::InvalidGcRef(array_ref.into_generic()))?;
    match array_inst {
        ReferenceInstance::ReferenceArray(array) => {
            if !element_type.is_reference() {
                return Err(EvalError::ExpectedArrayInstanceOf(element_type).into());
            }

            if let Some(element) = array.elements.get(index) {
                let val = match *element {
                    Some(v) => RuntimeValue::Reference(v),
                    None => RuntimeValue::NullReference,
                };
                frame.stack.push(val)?;
            } else {
                todo!("Return ArrayIndexOutOfBoundsException")
            }
        }
        ReferenceInstance::PrimitiveArray(array) => {
            if let RuntimeType::Primitive(_) = element_type {
                if !is_valid(array.element_type) {
                    return Err(EvalError::ExpectedArrayInstanceOf(element_type).into());
                }

                if let Some(element) = array.elements.get(index) {
                    frame.stack.push(*element)?;
                } else {
                    todo!("Return ArrayIndexOutOfBoundsException")
                }
            } else {
                return Err(EvalError::ExpectedArrayInstanceOf(element_type).into());
            }
        }
        _ => return Err(EvalError::ExpectedArrayInstance.into()),
    };

    Ok(RunInstContinueValue::Continue)
}

fn array_store(
    mut args: RunInstArgsC,
    is_good_type: impl Fn(RuntimeTypePrimitive) -> bool,
    convert_value: impl Fn(
        &mut RunInstArgsC,
        RuntimeValuePrimitive,
    ) -> Result<RuntimeValuePrimitive, GeneralError>,
    bad_array_value_err: GeneralError,
    bad_stack_value_err: GeneralError,
) -> Result<RunInstContinueValue, GeneralError> {
    let value = args
        .frame
        .stack
        .pop()
        .ok_or(EvalError::ExpectedStackValue)?;
    let value = match value {
        RuntimeValue::Primitive(v) => v,
        _ => return Err(bad_stack_value_err),
    };

    let index = args
        .frame
        .stack
        .pop()
        .ok_or(EvalError::ExpectedStackValue)?;
    let index = index
        .into_int()
        .ok_or(EvalError::ExpectedStackValueIntRepr)?;
    // TODO: Is it correct to treat it as a u32?
    let index = u32::from_ne_bytes(index.to_ne_bytes());

    let array_ref = args
        .frame
        .stack
        .pop()
        .ok_or(EvalError::ExpectedStackValue)?;
    let array_ref = match array_ref {
        RuntimeValue::Reference(x) => x,
        RuntimeValue::NullReference => todo!("Return NullPointerException"),
        RuntimeValue::Primitive(_) => return Err(EvalError::ExpectedStackValueReference.into()),
    };

    if !is_good_type(value.runtime_type()) {
        return Err(bad_stack_value_err);
    }

    let value = convert_value(&mut args, value)?;

    let array_inst = args
        .env
        .state
        .gc
        .deref_mut(array_ref)
        .ok_or(EvalError::InvalidGcRef(array_ref.into_generic()))?;
    let array_inst = if let ReferenceInstance::PrimitiveArray(array_inst) = array_inst {
        array_inst
    } else {
        // TODO: Better err for ReferenceArray
        return Err(EvalError::ExpectedArrayInstance.into());
    };

    if !is_good_type(array_inst.element_type) {
        // TODO: Better error
        return Err(bad_array_value_err);
    }

    let index = index.into_usize();
    if index >= array_inst.elements.len() {
        todo!("Return ArrayIndexOutOfBoundsException")
    }

    array_inst.elements[index] = value;

    Ok(RunInstContinueValue::Continue)
}

impl RunInstContinue for AALoad {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        array_load(&mut env.state, frame, RuntimeType::Reference(()))
    }
}
impl RunInstContinue for AAStore {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        let value = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let value_ref = match value {
            RuntimeValue::NullReference => None,
            RuntimeValue::Reference(v) => Some(v),
            RuntimeValue::Primitive(_) => return Err(EvalError::ExpectedStackValueReference.into()),
        };

        let index = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let index = index
            .into_int()
            .ok_or(EvalError::ExpectedStackValueIntRepr)?;
        // TODO: Is it correct to treat it as a u32?
        let index = u32::from_ne_bytes(index.to_ne_bytes());

        let array_ref = frame.stack.pop().ok_or(EvalError::ExpectedStackValue)?;
        let array_ref = match array_ref {
            RuntimeValue::Reference(x) => x,
            RuntimeValue::NullReference => todo!("Return NullPointerException"),
            RuntimeValue::Primitive(_) => return Err(EvalError::ExpectedStackValueReference.into()),
        };

        // This is calculated before because it needs immutable acess to state.gc but array inst
        // needs mutable access
        let id = if let Some(value_ref) = value_ref {
            let value_inst = env
                .state
                .gc
                .deref(value_ref)
                .ok_or(EvalError::InvalidGcRef(value_ref.into_generic()))?;
            let id = value_inst.instanceof();
            Some(id)
        } else {
            None
        };

        let array_inst = env
            .state
            .gc
            .deref_mut(array_ref)
            .ok_or(EvalError::InvalidGcRef(array_ref.into_generic()))?;
        let array_inst = if let ReferenceInstance::ReferenceArray(array_inst) = array_inst {
            array_inst
        } else {
            // TODO: better error for PrimitiveArray
            return Err(EvalError::ExpectedArrayInstance.into());
        };

        if let Some(id) = id {
            let is_castable = id == array_inst.element_type
                || env.classes.is_super_class(
                    &mut env.class_names,
                    &mut env.class_files,
                    &mut env.packages,
                    id,
                    array_inst.element_type,
                )?
                || env.classes.implements_interface(
                    &mut env.class_names,
                    &mut env.class_files,
                    id,
                    array_inst.element_type,
                )?
                || env.classes.is_castable_array(
                    &mut env.class_names,
                    &mut env.class_files,
                    &mut env.packages,
                    id,
                    array_inst.element_type,
                )?;

            if !is_castable {
                return Err(EvalError::ExpectedArrayInstanceOfClass {
                    element: array_inst.element_type,
                    got: id,
                }
                .into());
            }
        }
        // otherwise, if it was null, we can store it just fine

        let index = index.into_usize();
        if index >= array_inst.elements.len() {
            todo!("Return ArrayIndexOutOfBoundsException")
        }

        array_inst.elements[index] = value_ref;

        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for FloatArrayLoad {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        array_load(&mut env.state, frame, RuntimeTypePrimitive::F32)
    }
}
impl RunInstContinue for FloatArrayStore {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        array_store(
            args,
            |v| matches!(v, RuntimeTypePrimitive::F32),
            |_, v| Ok(v),
            EvalError::ExpectedArrayInstanceOf(RuntimeTypePrimitive::F32.into()).into(),
            EvalError::ExpectedStackValueFloat.into(),
        )
    }
}

impl RunInstContinue for DoubleArrayLoad {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        array_load(&mut env.state, frame, RuntimeTypePrimitive::F64)
    }
}
impl RunInstContinue for DoubleArrayStore {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        array_store(
            args,
            |v| matches!(v, RuntimeTypePrimitive::F64),
            |_, v| Ok(v),
            EvalError::ExpectedArrayInstanceOf(RuntimeTypePrimitive::F64.into()).into(),
            EvalError::ExpectedStackValueDouble.into(),
        )
    }
}

impl RunInstContinue for LongArrayLoad {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        array_load(&mut env.state, frame, RuntimeTypePrimitive::I64)
    }
}
impl RunInstContinue for LongArrayStore {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        array_store(
            args,
            |v| matches!(v, RuntimeTypePrimitive::I64),
            |_, v| Ok(v),
            EvalError::ExpectedArrayInstanceOf(RuntimeTypePrimitive::I64.into()).into(),
            EvalError::ExpectedStackValueDouble.into(),
        )
    }
}

impl RunInstContinue for ShortArrayLoad {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        array_load(&mut env.state, frame, RuntimeTypePrimitive::I16)
    }
}
impl RunInstContinue for ShortArrayStore {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        array_store(
            args,
            |v| v.can_be_int(),
            // int-repr values are accepted, then narrowed
            |_, v| {
                #[allow(clippy::cast_possible_truncation)]
                Ok(RuntimeValuePrimitive::I16(
                    v.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)? as i16,
                ))
            },
            EvalError::ExpectedArrayInstanceOf(RuntimeTypePrimitive::I16.into()).into(),
            EvalError::ExpectedStackValueIntRepr.into(),
        )
    }
}

impl RunInstContinue for IntALoad {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        array_load(&mut env.state, frame, RuntimeTypePrimitive::I32)
    }
}
impl RunInstContinue for IntArrayStore {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        array_store(
            args,
            |v| v.can_be_int(),
            // int-repr values are accepted, then narrowed
            |_, v| {
                Ok(RuntimeValuePrimitive::I32(
                    v.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?,
                ))
            },
            EvalError::ExpectedArrayInstanceOf(RuntimeTypePrimitive::I32.into()).into(),
            EvalError::ExpectedStackValueIntRepr.into(),
        )
    }
}

impl RunInstContinue for ByteArrayLoad {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        array_load_filter(&mut env.state, frame, RuntimeTypePrimitive::I8, |x| {
            x == RuntimeTypePrimitive::I8 || x == RuntimeTypePrimitive::Bool
        })
    }
}
impl RunInstContinue for ByteArrayStore {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        // This is an inlined version of array_store because bytearraystore needs to work with both
        // byte and boolean arrays
        let value = args
            .frame
            .stack
            .pop()
            .ok_or(EvalError::ExpectedStackValue)?;
        let value = match value {
            RuntimeValue::Primitive(v) => v,
            _ => return Err(EvalError::ExpectedStackValueIntRepr.into()),
        };

        let index = args
            .frame
            .stack
            .pop()
            .ok_or(EvalError::ExpectedStackValue)?;
        let index = index
            .into_int()
            .ok_or(EvalError::ExpectedStackValueIntRepr)?;
        // TODO: Is it correct to treat it as a u32?
        let index = u32::from_ne_bytes(index.to_ne_bytes());

        let array_ref = args
            .frame
            .stack
            .pop()
            .ok_or(EvalError::ExpectedStackValue)?;
        let array_ref = match array_ref {
            RuntimeValue::Reference(x) => x,
            RuntimeValue::NullReference => todo!("Return NullPointerException"),
            RuntimeValue::Primitive(_) => return Err(EvalError::ExpectedStackValueReference.into()),
        };

        if !value.runtime_type().can_be_int() {
            return Err(EvalError::ExpectedStackValueIntRepr.into());
        }

        let array_inst = args
            .env
            .state
            .gc
            .deref_mut(array_ref)
            .ok_or(EvalError::InvalidGcRef(array_ref.into_generic()))?;

        let array_inst = if let ReferenceInstance::PrimitiveArray(array_inst) = array_inst {
            array_inst
        } else {
            // TODO: Better err for ReferenceArray
            return Err(EvalError::ExpectedArrayInstance.into());
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = if array_inst.element_type == RuntimeTypePrimitive::I8 {
            RuntimeValuePrimitive::I8(
                value
                    .into_int()
                    .ok_or(EvalError::ExpectedStackValueIntRepr)? as i8,
            )
        } else if array_inst.element_type == RuntimeTypePrimitive::Bool {
            RuntimeValuePrimitive::Bool(
                value
                    .into_int()
                    .ok_or(EvalError::ExpectedStackValueIntRepr)? as u8,
            )
        } else {
            return Err(EvalError::ExpectedArrayInstanceOf(RuntimeTypePrimitive::I8.into()).into());
        };

        let index = index.into_usize();
        if index >= array_inst.elements.len() {
            todo!("Return ArrayIndexOutOfBoundsException")
        }

        array_inst.elements[index] = value;

        Ok(RunInstContinueValue::Continue)
    }
}

impl RunInstContinue for CharArrayLoad {
    fn run(
        self,
        RunInstArgsC { env, frame, .. }: RunInstArgsC,
    ) -> Result<RunInstContinueValue, GeneralError> {
        array_load(&mut env.state, frame, RuntimeTypePrimitive::Char)
    }
}
impl RunInstContinue for CharArrayStore {
    fn run(self, args: RunInstArgsC) -> Result<RunInstContinueValue, GeneralError> {
        array_store(
            args,
            |v| v.can_be_int(),
            // int-repr values are accepted, then narrowed
            |_, v| {
                Ok(RuntimeValuePrimitive::Char(JavaChar::from_int(
                    v.into_int().ok_or(EvalError::ExpectedStackValueIntRepr)?,
                )))
            },
            EvalError::ExpectedArrayInstanceOf(RuntimeTypePrimitive::Char.into()).into(),
            EvalError::ExpectedStackValueIntRepr.into(),
        )
    }
}
