//! This file is separate from op.rs, because op.rs is large enough to be unfortunately slow.

use std::num::NonZeroUsize;

use classfile_parser::attribute_info::InstructionIndex;
use classfile_parser::constant_info::{ConstantInfo, FieldRefConstant, NameAndTypeConstant};
use classfile_parser::constant_pool::ConstantPoolIndexRaw;

use classfile_parser::descriptor::method::MethodDescriptorError;
use classfile_parser::descriptor::DescriptorType as DescriptorTypeCF;
use smallvec::SmallVec;

use super::method::DescriptorType;
use super::op::{
    ALoad, ANewArray, AStore, CheckCast, DoubleLoad, DoubleStore, Dup2, Dup2X1, Dup2X2, DupX2,
    FloatLoad, FloatStore, GetField, GetStatic, IntIncrement, IntLoad, IntStore, InvokeDynamic,
    InvokeInterface, InvokeSpecial, InvokeStatic, InvokeVirtual, LoadConstant, LoadConstant2Wide,
    LoadConstantWide, LongLoad, LongStore, MultiANewArray, New, NewArray, Pop2, PutField,
    PutStaticField, WideIntIncrement, WideIntLoad,
};
use super::types::{
    Category, ComplexType, HasStackInfo, LocalVariableInType, LocalVariableIndex,
    LocalVariableType, LocalsIn, LocalsOutAt, PopComplexType, PopType, PopTypeAt, PushIndex,
    PushType, PushTypeAt, StackInfo, StackSizes,
};
use super::{
    op::RawOpcode,
    types::{PopIndex, PrimitiveType, Type, WithType},
};
use crate::class::ClassFileInfo;
use crate::code::method::MethodDescriptor;
use crate::code::types::StackInfoError;
use crate::data::class_names::ClassNames;
use crate::id::{ClassId, ExactMethodId};
use crate::{LoadMethodError, StepError};

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

macro_rules! empty_push {
    ($for:ty) => {
        impl PushTypeAt for $for {
            fn push_type_at(&self, _i: PushIndex) -> Option<PushType> {
                None
            }

            fn push_count(&self) -> usize {
                0
            }
        }
    };
}
macro_rules! empty_locals_out {
    ($for:ty) => {
        impl LocalsOutAt for $for {
            type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableType), 0>;

            fn locals_out_type_iter(&self) -> Self::Iter {
                [].into_iter()
            }
        }
    };
}
macro_rules! empty_locals_in {
    ($for:ty) => {
        impl LocalsIn for $for {
            type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableInType), 0>;

            fn locals_in_type_iter(&self) -> Self::Iter {
                [].into_iter()
            }
        }
    };
}

// === Pop/Push implementations for various opcodes ===
pub struct ANewArrayInfo {
    array_id: ClassId,
}
impl StackInfo for ANewArrayInfo {}
impl PushTypeAt for ANewArrayInfo {
    // TODO: This could be implemented in the macro if we had access to `self`
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            Some(ComplexType::ReferenceClass(self.array_id).into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}
impl PopTypeAt for ANewArrayInfo {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        if i == 0 {
            // Count
            Some(PrimitiveType::Int.into())
        } else {
            None
        }
    }

    fn pop_count(&self) -> usize {
        1
    }
}
empty_locals_in!(ANewArrayInfo);
empty_locals_out!(ANewArrayInfo);
impl HasStackInfo for ANewArray {
    type Output = ANewArrayInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _: ExactMethodId,
        _: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let elem_class =
            class_file
                .get_t(self.index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let elem_name = class_file.get_text_b(elem_class.name_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(elem_class.name_index.into_generic()),
        )?;
        let elem_id = class_names.gcid_from_bytes(elem_name);

        let array_id = class_names
            .gcid_from_level_array_of_class_id(NonZeroUsize::new(1).unwrap(), elem_id)
            .map_err(StepError::BadId)?;

        Ok(ANewArrayInfo { array_id })
    }
}

pub struct MultiANewArrayInfo {
    array_id: ClassId,
    dimensions: u8,
}
impl StackInfo for MultiANewArrayInfo {}
impl PopTypeAt for MultiANewArrayInfo {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        if i < self.dimensions as usize {
            // count
            Some(PrimitiveType::Int.into())
        } else {
            None
        }
    }

    fn pop_count(&self) -> usize {
        self.dimensions as usize
    }
}
impl PushTypeAt for MultiANewArrayInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            Some(ComplexType::ReferenceClass(self.array_id).into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}
empty_locals_in!(MultiANewArrayInfo);
empty_locals_out!(MultiANewArrayInfo);
impl HasStackInfo for MultiANewArray {
    type Output = MultiANewArrayInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _: ExactMethodId,
        _: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let array_class =
            class_file
                .get_t(self.index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let array_name = class_file.get_text_b(array_class.name_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(array_class.name_index.into_generic()),
        )?;
        let array_id = class_names.gcid_from_bytes(array_name);
        Ok(MultiANewArrayInfo {
            array_id,
            dimensions: self.dimensions,
        })
    }
}

impl NewArray {
    #[must_use]
    pub fn get_atype_as_primitive_type(&self) -> Option<PrimitiveType> {
        Some(match self.atype {
            4 => PrimitiveType::Boolean,
            5 => PrimitiveType::Char,
            6 => PrimitiveType::Float,
            7 => PrimitiveType::Double,
            8 => PrimitiveType::Byte,
            9 => PrimitiveType::Short,
            10 => PrimitiveType::Int,
            11 => PrimitiveType::Long,
            _ => return None,
        })
    }
}
impl PushTypeAt for NewArray {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            // TODO: don't panic
            let element_type = self
                .get_atype_as_primitive_type()
                .expect("AType argument for NewArray was invalid");
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

    fn push_count(&self) -> usize {
        1
    }
}

pub struct CheckCastInfo {
    id: ClassId,
}
impl StackInfo for CheckCastInfo {}
impl PushTypeAt for CheckCastInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            Some(ComplexType::ReferenceClass(self.id).into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}
impl PopTypeAt for CheckCastInfo {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        if i == 0 {
            Some(PopComplexType::ReferenceAny.into())
        } else {
            None
        }
    }

    fn pop_count(&self) -> usize {
        1
    }
}
empty_locals_in!(CheckCastInfo);
empty_locals_out!(CheckCastInfo);
impl HasStackInfo for CheckCast {
    type Output = CheckCastInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _: ExactMethodId,
        _: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let class =
            class_file
                .get_t(self.index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let name = class_file.get_text_b(class.name_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(class.name_index.into_generic()),
        )?;
        let id = class_names.gcid_from_bytes(name);
        Ok(CheckCastInfo { id })
    }
}

type SingleInLocal = std::array::IntoIter<(LocalVariableIndex, LocalVariableInType), 1>;

impl LocalsIn for ALoad {
    type Iter = SingleInLocal;

    fn locals_in_type_iter(&self) -> Self::Iter {
        [(self.index.into(), LocalVariableInType::ReferenceAny)].into_iter()
    }
}

impl LocalsIn for IntLoad {
    type Iter = SingleInLocal;

    fn locals_in_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Int.into())].into_iter()
    }
}
impl LocalsIn for IntIncrement {
    type Iter = SingleInLocal;

    fn locals_in_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Int.into())].into_iter()
    }
}
impl LocalsIn for WideIntLoad {
    type Iter = SingleInLocal;

    fn locals_in_type_iter(&self) -> Self::Iter {
        [(self.index, PrimitiveType::Int.into())].into_iter()
    }
}
impl LocalsIn for WideIntIncrement {
    type Iter = SingleInLocal;

    fn locals_in_type_iter(&self) -> Self::Iter {
        [(self.index, PrimitiveType::Int.into())].into_iter()
    }
}

impl LocalsIn for LongLoad {
    type Iter = SingleInLocal;

    fn locals_in_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Long.into())].into_iter()
    }
}
impl LocalsIn for DoubleLoad {
    type Iter = SingleInLocal;

    fn locals_in_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Double.into())].into_iter()
    }
}
impl LocalsIn for FloatLoad {
    type Iter = SingleInLocal;

    fn locals_in_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Float.into())].into_iter()
    }
}

impl LocalsOutAt for AStore {
    type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableType), 1>;

    fn locals_out_type_iter(&self) -> Self::Iter {
        [(self.index.into(), WithType::Type(0).into())].into_iter()
    }
}
impl LocalsOutAt for IntStore {
    type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableType), 1>;

    fn locals_out_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Int.into())].into_iter()
    }
}
impl LocalsOutAt for DoubleStore {
    type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableType), 1>;

    fn locals_out_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Double.into())].into_iter()
    }
}
impl LocalsOutAt for FloatStore {
    type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableType), 1>;

    fn locals_out_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Float.into())].into_iter()
    }
}
impl LocalsOutAt for LongStore {
    type Iter = std::array::IntoIter<(LocalVariableIndex, LocalVariableType), 1>;

    fn locals_out_type_iter(&self) -> Self::Iter {
        [(self.index.into(), PrimitiveType::Long.into())].into_iter()
    }
}

fn descriptor_into_parameters_ret<const N: usize>(
    class_names: &mut ClassNames,
    descriptor: &[u8],
) -> Result<(SmallVec<[Type; N]>, Option<Type>), MethodDescriptorError> {
    let mut desc_iter = MethodDescriptor::from_text_iter(descriptor, class_names)?;

    let mut parameters = SmallVec::new();

    #[allow(clippy::while_let_on_iterator)]
    while let Some(parameter) = desc_iter.next() {
        let parameter = parameter?;
        let parameter = Type::from_descriptor_type(parameter);
        parameters.push(parameter);
    }

    let return_type = desc_iter
        .finish_return_type()?
        .map(Type::from_descriptor_type);
    Ok((parameters, return_type))
}

/// Not *exactly* static, more for methods that have all their popped args in their descriptor
/// such that this can just use the parameters as the pop type at info
pub struct StaticMethodInfo {
    parameters: SmallVec<[Type; 8]>,
    return_type: Option<Type>,
}
impl StaticMethodInfo {
    fn from_nat_index(
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        nat_index: ConstantPoolIndexRaw<NameAndTypeConstant>,
    ) -> Result<StaticMethodInfo, StepError> {
        let nat = class_file
            .get_t(nat_index)
            .ok_or(StackInfoError::InvalidConstantPoolIndex(
                nat_index.into_generic(),
            ))?;
        let descriptor = class_file.get_text_b(nat.descriptor_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(nat.descriptor_index.into_generic()),
        )?;

        let (parameters, return_type) = descriptor_into_parameters_ret(class_names, descriptor)
            .map_err(LoadMethodError::MethodDescriptorError)?;

        Ok(StaticMethodInfo {
            parameters,
            return_type,
        })
    }
}
impl StackInfo for StaticMethodInfo {}
impl PopTypeAt for StaticMethodInfo {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        // Stack access is in reverse
        self.parameters.iter().rev().nth(i).cloned().map(Into::into)
    }

    fn pop_count(&self) -> usize {
        self.parameters.len()
    }
}
impl PushTypeAt for StaticMethodInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            self.return_type.clone().map(Into::into)
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        if self.return_type.is_some() {
            1
        } else {
            0
        }
    }
}
empty_locals_out!(StaticMethodInfo);
empty_locals_in!(StaticMethodInfo);

/// Method info where the first parameter is of some class
pub struct RefMethodInfo {
    /// The target class id
    class_id: Option<ClassId>,
    parameters: SmallVec<[Type; 8]>,
    return_type: Option<Type>,
}
impl RefMethodInfo {
    fn from_nat_index(
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        rec_class_id: Option<ClassId>,
        nat_index: ConstantPoolIndexRaw<NameAndTypeConstant>,
    ) -> Result<RefMethodInfo, StepError> {
        let nat = class_file
            .get_t(nat_index)
            .ok_or(StackInfoError::InvalidConstantPoolIndex(
                nat_index.into_generic(),
            ))?;
        let descriptor = class_file.get_text_b(nat.descriptor_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(nat.descriptor_index.into_generic()),
        )?;
        let (parameters, return_type) = descriptor_into_parameters_ret(class_names, descriptor)
            .map_err(LoadMethodError::MethodDescriptorError)?;

        Ok(RefMethodInfo {
            class_id: rec_class_id,
            parameters,
            return_type,
        })
    }
}
impl StackInfo for RefMethodInfo {}
impl PopTypeAt for RefMethodInfo {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        [self
            .class_id
            .map(ComplexType::ReferenceClass)
            .map_or_else(|| PopComplexType::ReferenceAny.into(), PopType::from)]
        .into_iter()
        .chain(self.parameters.iter().cloned().map(PopType::from))
        .rev()
        .nth(i)
    }

    fn pop_count(&self) -> usize {
        1 + self.parameters.len()
    }
}
impl PushTypeAt for RefMethodInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            self.return_type.clone().map(Into::into)
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        if self.return_type.is_some() {
            1
        } else {
            0
        }
    }
}
empty_locals_out!(RefMethodInfo);
empty_locals_in!(RefMethodInfo);

impl HasStackInfo for InvokeSpecial {
    type Output = RefMethodInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;

        let target_method =
            class_file
                .get_t(index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    index.into_generic(),
                ))?;

        // We may not be allowed to actually access the method, but that requires loading
        // classes, and so is not checked here.

        let (class_index, nat_index) = match target_method {
            ConstantInfo::MethodRef(method_ref) => {
                (method_ref.class_index, method_ref.name_and_type_index)
            }
            ConstantInfo::InterfaceMethodRef(method_ref) => {
                (method_ref.class_index, method_ref.name_and_type_index)
            }
            _ => return Err(StackInfoError::IncorrectConstantPoolType.into()),
        };

        let rec_class =
            class_file
                .get_t(class_index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    class_index.into_generic(),
                ))?;
        let rec_class_name = class_file.get_text_b(rec_class.name_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(rec_class.name_index.into_generic()),
        )?;
        let rec_class_id = class_names.gcid_from_bytes(rec_class_name);

        RefMethodInfo::from_nat_index(class_names, class_file, Some(rec_class_id), nat_index)
    }
}
impl HasStackInfo for InvokeInterface {
    type Output = RefMethodInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;
        // We can ignore count, since we get that same information from the Descriptor

        let target_method =
            class_file
                .get_t(index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    index.into_generic(),
                ))?;

        let nat_index = target_method.name_and_type_index;
        let rec_class = class_file.get_t(target_method.class_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(target_method.class_index.into_generic()),
        )?;
        let rec_class_name = class_file.get_text_b(rec_class.name_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(rec_class.name_index.into_generic()),
        )?;
        let rec_class_id = class_names.gcid_from_bytes(rec_class_name);

        RefMethodInfo::from_nat_index(class_names, class_file, Some(rec_class_id), nat_index)
    }
}
impl HasStackInfo for InvokeDynamic {
    type Output = StaticMethodInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;

        let target_method =
            class_file
                .get_t(index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    index.into_generic(),
                ))?;

        let nat_index = target_method.name_and_type_index;

        // The descriptor has the receiver encoded in it (I think?) and so we don't have to do anything
        // with the boostrap method here.
        // So this is kindof-technically static.
        StaticMethodInfo::from_nat_index(class_names, class_file, nat_index)
    }
}
impl HasStackInfo for InvokeStatic {
    type Output = StaticMethodInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;

        let calling_method = class_file
            .get_t(index)
            .ok_or(StackInfoError::InvalidConstantPoolIndex(index))?;

        let nat_index = match calling_method {
            ConstantInfo::MethodRef(method_ref) => method_ref.name_and_type_index,
            ConstantInfo::InterfaceMethodRef(method_ref) => method_ref.name_and_type_index,
            _ => return Err(StackInfoError::IncorrectConstantPoolType.into()),
        };

        StaticMethodInfo::from_nat_index(class_names, class_file, nat_index)
    }
}
impl HasStackInfo for InvokeVirtual {
    type Output = RefMethodInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;

        let target_method =
            class_file
                .get_t(index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    index.into_generic(),
                ))?;

        // FIXME: Handle signature polymorphic things
        // FIXME: Even beyond that, this may not be correct for invokevirtual

        // We may not be allowed to actually access the method, but that requires loading
        // classes, and so is not checked here.

        let (class_index, nat_index) = match target_method {
            ConstantInfo::MethodRef(method_ref) => {
                (method_ref.class_index, method_ref.name_and_type_index)
            }
            ConstantInfo::InterfaceMethodRef(method_ref) => {
                (method_ref.class_index, method_ref.name_and_type_index)
            }
            _ => return Err(StackInfoError::IncorrectConstantPoolType.into()),
        };

        let rec_class =
            class_file
                .get_t(class_index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    class_index.into_generic(),
                ))?;
        let rec_class_name = class_file.get_text_b(rec_class.name_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(rec_class.name_index.into_generic()),
        )?;
        let rec_class_id = class_names.gcid_from_bytes(rec_class_name);

        RefMethodInfo::from_nat_index(class_names, class_file, Some(rec_class_id), nat_index)
    }
}

pub struct GetFieldStackInfo {
    field_type: Type,
}
impl StackInfo for GetFieldStackInfo {}
impl PopTypeAt for GetFieldStackInfo {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        // TODO: We do get more information about this that can be given
        if i == 0 {
            Some(PopComplexType::ReferenceAny.into())
        } else {
            None
        }
    }

    fn pop_count(&self) -> usize {
        1
    }
}
impl PushTypeAt for GetFieldStackInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            Some(self.field_type.clone().into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}
empty_locals_out!(GetFieldStackInfo);
empty_locals_in!(GetFieldStackInfo);
impl HasStackInfo for GetField {
    type Output = GetFieldStackInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;

        let field_type = get_field_type(class_names, class_file, index)?;
        // Get the type version
        let field_type = Type::from_descriptor_type(field_type);

        Ok(GetFieldStackInfo { field_type })
    }
}

pub struct PutStaticFieldStackInfo {
    field_type: Type,
}
impl StackInfo for PutStaticFieldStackInfo {}
impl PopTypeAt for PutStaticFieldStackInfo {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        if i == 0 {
            Some(self.field_type.clone().into())
        } else {
            None
        }
    }

    fn pop_count(&self) -> usize {
        1
    }
}
empty_push!(PutStaticFieldStackInfo);
empty_locals_out!(PutStaticFieldStackInfo);
empty_locals_in!(PutStaticFieldStackInfo);
impl HasStackInfo for PutStaticField {
    type Output = PutStaticFieldStackInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;

        let field_type = get_field_type(class_names, class_file, index)?;
        // Get the type version
        let field_type = Type::from_descriptor_type(field_type);

        Ok(PutStaticFieldStackInfo { field_type })
    }
}
pub struct PutFieldStackInfo {
    field_type: Type,
}
impl StackInfo for PutFieldStackInfo {}
impl PopTypeAt for PutFieldStackInfo {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        if i == 0 {
            // value
            Some(self.field_type.clone().into())
        } else if i == 1 {
            // TODO: There is more info we could give here
            // objectref
            Some(PopComplexType::ReferenceAny.into())
        } else {
            None
        }
    }

    fn pop_count(&self) -> usize {
        2
    }
}
empty_push!(PutFieldStackInfo);
empty_locals_out!(PutFieldStackInfo);
empty_locals_in!(PutFieldStackInfo);
impl HasStackInfo for PutField {
    type Output = PutFieldStackInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;

        let field_type = get_field_type(class_names, class_file, index)?;
        // Get the type version
        let field_type = Type::from_descriptor_type(field_type);

        // Construct the stack info, which can now provide the proper type information
        Ok(PutFieldStackInfo { field_type })
    }
}

fn get_field_type(
    class_names: &mut ClassNames,
    class_file: &ClassFileInfo,
    index: ConstantPoolIndexRaw<FieldRefConstant>,
) -> Result<DescriptorType, StepError> {
    let field = class_file
        .get_t(index)
        .ok_or(StackInfoError::InvalidConstantPoolIndex(
            index.into_generic(),
        ))?;

    // Get the name and type data, which has the name of the field and a descriptor
    let nat_index = field.name_and_type_index;
    let nat = class_file
        .get_t(nat_index)
        .ok_or(StackInfoError::InvalidConstantPoolIndex(
            nat_index.into_generic(),
        ))?;

    // Get the descriptor text, which describes the type of the field
    let field_descriptor = class_file.get_text_b(nat.descriptor_index).ok_or(
        StackInfoError::InvalidConstantPoolIndex(nat.descriptor_index.into_generic()),
    )?;
    // Parse the type of the field
    let (field_type, rem) =
        DescriptorTypeCF::parse(field_descriptor).map_err(StackInfoError::InvalidDescriptorType)?;
    // There shouldn't be any remaining data.
    if !rem.is_empty() {
        return Err(StackInfoError::UnparsedFieldType.into());
    }
    // Convert to alternative descriptor type
    Ok(DescriptorType::from_class_file_desc(
        class_names,
        field_type,
    ))
}

pub struct GetStaticStackInfo {
    field_type: Type,
}
impl StackInfo for GetStaticStackInfo {}
impl PopTypeAt for GetStaticStackInfo {
    fn pop_type_at(&self, _: PopIndex) -> Option<PopType> {
        None
    }

    fn pop_count(&self) -> usize {
        0
    }
}
impl PushTypeAt for GetStaticStackInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            Some(self.field_type.clone().into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}
empty_locals_out!(GetStaticStackInfo);
empty_locals_in!(GetStaticStackInfo);
impl HasStackInfo for GetStatic {
    type Output = GetStaticStackInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _method_id: ExactMethodId,
        _stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let index = self.index;

        let field_type = get_field_type(class_names, class_file, index)?;
        // Get the type version
        let field_type = Type::from_descriptor_type(field_type);

        Ok(GetStaticStackInfo { field_type })
    }
}

pub struct LoadConstantInfo {
    typ: Type,
}
impl StackInfo for LoadConstantInfo {}
impl PushTypeAt for LoadConstantInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            Some(self.typ.clone().into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}
impl PopTypeAt for LoadConstantInfo {
    fn pop_type_at(&self, _: PopIndex) -> Option<PopType> {
        None
    }

    fn pop_count(&self) -> usize {
        0
    }
}
empty_locals_in!(LoadConstantInfo);
empty_locals_out!(LoadConstantInfo);
fn load_constant_info(
    class_names: &mut ClassNames,
    class_file: &ClassFileInfo,
    index: ConstantPoolIndexRaw<ConstantInfo>,
) -> Result<Type, StepError> {
    let value = class_file
        .get_t(index)
        .ok_or(StackInfoError::InvalidConstantPoolIndex(
            index.into_generic(),
        ))?;
    Ok(match value {
        ConstantInfo::Integer(_) => PrimitiveType::Int.into(),
        ConstantInfo::Float(_) => PrimitiveType::Float.into(),
        ConstantInfo::Class(_) => {
            ComplexType::ReferenceClass(class_names.gcid_from_bytes(b"java/lang/Class")).into()
        }
        ConstantInfo::String(_) => {
            ComplexType::ReferenceClass(class_names.gcid_from_bytes(b"java/lang/String")).into()
        }
        ConstantInfo::MethodHandle(_) => ComplexType::ReferenceClass(
            class_names.gcid_from_bytes(b"java/lang/invoke/MethodHandle"),
        )
        .into(),
        ConstantInfo::MethodType(_) => {
            ComplexType::ReferenceClass(class_names.gcid_from_bytes(b"java/lang/invoke/MethodType"))
                .into()
        }
        _ => return Err(StackInfoError::InvalidConstantPoolIndex(index.into_generic()).into()),
    })
}
impl HasStackInfo for LoadConstant {
    type Output = LoadConstantInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _: ExactMethodId,
        _: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let typ = load_constant_info(class_names, class_file, self.index)?;
        Ok(LoadConstantInfo { typ })
    }
}
impl HasStackInfo for LoadConstantWide {
    type Output = LoadConstantInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _: ExactMethodId,
        _: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let typ = load_constant_info(class_names, class_file, self.index)?;
        Ok(LoadConstantInfo { typ })
    }
}

pub struct LoadConstant2WideInfo {
    typ: Type,
}
impl StackInfo for LoadConstant2WideInfo {}
impl PushTypeAt for LoadConstant2WideInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            Some(self.typ.clone().into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}
impl PopTypeAt for LoadConstant2WideInfo {
    fn pop_type_at(&self, _: PopIndex) -> Option<PopType> {
        None
    }

    fn pop_count(&self) -> usize {
        0
    }
}
empty_locals_in!(LoadConstant2WideInfo);
empty_locals_out!(LoadConstant2WideInfo);
impl HasStackInfo for LoadConstant2Wide {
    type Output = LoadConstant2WideInfo;

    fn stack_info(
        &self,
        _: &mut ClassNames,
        class_file: &ClassFileInfo,
        _: ExactMethodId,
        _: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let value =
            class_file
                .get_t(self.index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let typ = match value {
            ConstantInfo::Long(_) => PrimitiveType::Long.into(),
            ConstantInfo::Double(_) => PrimitiveType::Double.into(),
            _ => {
                return Err(
                    StackInfoError::InvalidConstantPoolIndex(self.index.into_generic()).into(),
                )
            }
        };
        Ok(LoadConstant2WideInfo { typ })
    }
}

pub struct NewInfo {
    id: ClassId,
}
impl StackInfo for NewInfo {}
impl PopTypeAt for NewInfo {
    fn pop_type_at(&self, _: PopIndex) -> Option<PopType> {
        None
    }

    fn pop_count(&self) -> usize {
        0
    }
}
impl PushTypeAt for NewInfo {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            // TODO: Include the information that it is uninitialized?
            Some(ComplexType::ReferenceClass(self.id).into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}
empty_locals_in!(NewInfo);
empty_locals_out!(NewInfo);
impl HasStackInfo for New {
    type Output = NewInfo;

    fn stack_info(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileInfo,
        _: ExactMethodId,
        _: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let class =
            class_file
                .get_t(self.index)
                .ok_or(StackInfoError::InvalidConstantPoolIndex(
                    self.index.into_generic(),
                ))?;
        let name = class_file.get_text_b(class.name_index).ok_or(
            StackInfoError::InvalidConstantPoolIndex(class.name_index.into_generic()),
        )?;
        let id = class_names.gcid_from_bytes(name);

        Ok(NewInfo { id })
    }
}
impl PushTypeAt for ALoad {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        if i == 0 {
            Some(WithType::LocalVariableRefAtIndexNoRetAddr(self.index.into()).into())
        } else {
            None
        }
    }

    fn push_count(&self) -> usize {
        1
    }
}

pub enum Pop2Info {
    /// Pop two category ones
    Category1,
    /// Pop one category two
    Category2,
}
impl StackInfo for Pop2Info {}
impl PopTypeAt for Pop2Info {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        match (self, i) {
            (Pop2Info::Category1, 0 | 1) => Some(PopType::Category1),
            (Pop2Info::Category2, 0) => Some(PopType::Category2),
            (_, _) => None,
        }
    }

    fn pop_count(&self) -> usize {
        match self {
            Pop2Info::Category1 => 2,
            Pop2Info::Category2 => 1,
        }
    }
}
empty_push!(Pop2Info);
empty_locals_out!(Pop2Info);
empty_locals_in!(Pop2Info);
impl HasStackInfo for Pop2 {
    type Output = Pop2Info;

    fn stack_info(
        &self,
        _: &mut ClassNames,
        _: &ClassFileInfo,
        _method_id: ExactMethodId,
        stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let first = stack_sizes[0].ok_or(StackInfoError::NeededStackSizeAt(0))?;
        if first == Category::Two {
            return Ok(Pop2Info::Category2);
        }

        let second = stack_sizes[1].ok_or(StackInfoError::NeededStackSizeAt(1))?;
        if second == Category::One {
            Ok(Pop2Info::Category1)
        } else {
            Err(StackInfoError::BadStackSizes.into())
        }
    }
}

pub enum Dup2Info {
    TwoCategory1,
    OneCategory2,
}
impl StackInfo for Dup2Info {}
impl PopTypeAt for Dup2Info {
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        match (self, i) {
            (Dup2Info::TwoCategory1, 0 | 1) => Some(PopType::Category1),
            (Dup2Info::OneCategory2, 0) => Some(PopType::Category2),
            (_, _) => None,
        }
    }

    fn pop_count(&self) -> usize {
        match self {
            Dup2Info::TwoCategory1 => 2,
            Dup2Info::OneCategory2 => 1,
        }
    }
}
#[allow(clippy::match_same_arms)]
impl PushTypeAt for Dup2Info {
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        match (self, i) {
            // val1 is at top
            // val2 is next
            // then val1 is at the top of the resulting stack
            // these indices are relative to current stack, so 3 is going to be the topmost value
            (Dup2Info::TwoCategory1, 1 | 3) => Some(WithType::Type(0).into()),
            (Dup2Info::TwoCategory1, 0 | 2) => Some(WithType::Type(1).into()),
            // pop val
            // push val, val
            (Dup2Info::OneCategory2, 0 | 1) => Some(WithType::Type(0).into()),
            (_, _) => None,
        }
    }

    fn push_count(&self) -> usize {
        match self {
            Dup2Info::TwoCategory1 => 4,
            Dup2Info::OneCategory2 => 2,
        }
    }
}
empty_locals_out!(Dup2Info);
empty_locals_in!(Dup2Info);

impl HasStackInfo for Dup2 {
    type Output = Dup2Info;

    fn stack_info(
        &self,
        _: &mut ClassNames,
        _: &ClassFileInfo,
        _method_id: ExactMethodId,
        stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let first = stack_sizes[0].ok_or(StackInfoError::NeededStackSizeAt(0))?;
        if first == Category::Two {
            return Ok(Dup2Info::OneCategory2);
        }

        let second = stack_sizes[1].ok_or(StackInfoError::NeededStackSizeAt(1))?;
        if second == Category::Two {
            return Err(StackInfoError::BadStackSizes.into());
        }

        Ok(Dup2Info::TwoCategory1)
    }
}

pub enum DupX2Info {
    /// Cat1, then Cat1, then Cat1
    Category1,
    /// Cat1 value then Cat2 value
    Category2,
}
impl StackInfo for DupX2Info {}
impl PopTypeAt for DupX2Info {
    #[allow(clippy::match_same_arms)]
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        match (self, i) {
            (DupX2Info::Category1, 0 | 1 | 2) => Some(PopType::Category1),
            (DupX2Info::Category2, 0) => Some(PopType::Category1),
            (DupX2Info::Category2, 1) => Some(PopType::Category2),
            (_, _) => None,
        }
    }

    fn pop_count(&self) -> usize {
        match self {
            DupX2Info::Category1 => 3,
            DupX2Info::Category2 => 2,
        }
    }
}
impl PushTypeAt for DupX2Info {
    #[allow(clippy::match_same_arms)]
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        match (self, i) {
            (DupX2Info::Category1, 0 | 3) => Some(WithType::Type(0).into()),
            (DupX2Info::Category1, 1) => Some(WithType::Type(2).into()),
            (DupX2Info::Category1, 2) => Some(WithType::Type(1).into()),

            (DupX2Info::Category2, 0 | 2) => Some(WithType::Type(0).into()),
            (DupX2Info::Category2, 1) => Some(WithType::Type(1).into()),
            (_, _) => None,
        }
    }

    fn push_count(&self) -> usize {
        match self {
            DupX2Info::Category1 => 4,
            DupX2Info::Category2 => 3,
        }
    }
}
empty_locals_out!(DupX2Info);
empty_locals_in!(DupX2Info);

impl HasStackInfo for DupX2 {
    type Output = DupX2Info;

    fn stack_info(
        &self,
        _: &mut ClassNames,
        _: &ClassFileInfo,
        _method_id: ExactMethodId,
        stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let first = stack_sizes[0].ok_or(StackInfoError::NeededStackSizeAt(0))?;
        // DupX2 only supports the first entry being category 1
        if first != Category::One {
            return Err(StackInfoError::BadStackSizes.into());
        }

        let second = stack_sizes[1].ok_or(StackInfoError::NeededStackSizeAt(1))?;
        if second == Category::Two {
            return Ok(DupX2Info::Category2);
        }

        let third = stack_sizes[2].ok_or(StackInfoError::NeededStackSizeAt(2))?;
        if third == Category::One {
            Ok(DupX2Info::Category1)
        } else {
            // if there is a third entry then it must be category two
            Err(StackInfoError::BadStackSizes.into())
        }
    }
}

pub enum Dup2X1Info {
    /// Three category ones
    Category1,
    /// Category 2 then category 1
    Category2,
}
impl StackInfo for Dup2X1Info {}
impl PopTypeAt for Dup2X1Info {
    #[allow(clippy::match_same_arms)]
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        match (self, i) {
            (Dup2X1Info::Category1, 0 | 1 | 2) => Some(PopType::Category1),
            (Dup2X1Info::Category2, 0) => Some(PopType::Category2),
            (Dup2X1Info::Category2, 1) => Some(PopType::Category1),
            (_, _) => None,
        }
    }

    fn pop_count(&self) -> usize {
        match self {
            Dup2X1Info::Category1 => 3,
            Dup2X1Info::Category2 => 2,
        }
    }
}
impl PushTypeAt for Dup2X1Info {
    #[allow(clippy::match_same_arms)]
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        Some(
            match (self, i) {
                (Dup2X1Info::Category1, 0) => WithType::Type(1),
                (Dup2X1Info::Category1, 1) => WithType::Type(0),
                (Dup2X1Info::Category1, 2) => WithType::Type(2),
                (Dup2X1Info::Category1, 3) => WithType::Type(1),
                (Dup2X1Info::Category1, 4) => WithType::Type(0),

                (Dup2X1Info::Category2, 0) => WithType::Type(0),
                (Dup2X1Info::Category2, 1) => WithType::Type(1),
                (Dup2X1Info::Category2, 2) => WithType::Type(0),

                (_, _) => return None,
            }
            .into(),
        )
    }

    fn push_count(&self) -> usize {
        match self {
            Dup2X1Info::Category1 => 5,
            Dup2X1Info::Category2 => 3,
        }
    }
}
empty_locals_out!(Dup2X1Info);
empty_locals_in!(Dup2X1Info);

impl HasStackInfo for Dup2X1 {
    type Output = Dup2X1Info;

    fn stack_info(
        &self,
        _: &mut ClassNames,
        _: &ClassFileInfo,
        _method_id: ExactMethodId,
        stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let first = stack_sizes[0].ok_or(StackInfoError::NeededStackSizeAt(0))?;

        let second = stack_sizes[1].ok_or(StackInfoError::NeededStackSizeAt(1))?;

        // The second entry is category one for both forms
        if second != Category::One {
            return Err(StackInfoError::BadStackSizes.into());
        }

        if first == Category::One {
            let third = stack_sizes[2].ok_or(StackInfoError::NeededStackSizeAt(2))?;
            if third != Category::One {
                return Err(StackInfoError::BadStackSizes.into());
            }

            Ok(Dup2X1Info::Category1)
        } else {
            Ok(Dup2X1Info::Category2)
        }
    }
}

pub enum Dup2X2Info {
    /// 4 category ones
    Form1,
    /// 1 cat-2 then 2 cat-1
    Form2,
    /// 2 cat-1 then 1 cat-2
    Form3,
    /// 2 cat-2
    Form4,
}
impl StackInfo for Dup2X2Info {}
impl PopTypeAt for Dup2X2Info {
    #[allow(clippy::match_same_arms)]
    fn pop_type_at(&self, i: PopIndex) -> Option<PopType> {
        match (self, i) {
            (Dup2X2Info::Form1, 0 | 1 | 2 | 3) => Some(PopType::Category1),

            (Dup2X2Info::Form2, 0) => Some(PopType::Category2),
            (Dup2X2Info::Form2, 1 | 2) => Some(PopType::Category1),

            (Dup2X2Info::Form3, 0 | 1) => Some(PopType::Category1),
            (Dup2X2Info::Form3, 2) => Some(PopType::Category2),

            (Dup2X2Info::Form4, 0 | 1) => Some(PopType::Category2),

            (_, _) => None,
        }
    }

    #[allow(clippy::match_same_arms)]
    fn pop_count(&self) -> usize {
        match self {
            Dup2X2Info::Form1 => 4,
            Dup2X2Info::Form2 => 3,
            Dup2X2Info::Form3 => 3,
            Dup2X2Info::Form4 => 2,
        }
    }
}
impl PushTypeAt for Dup2X2Info {
    #[allow(clippy::match_same_arms)]
    fn push_type_at(&self, i: PushIndex) -> Option<PushType> {
        Some(
            WithType::Type(match (self, i) {
                (Dup2X2Info::Form1, 0) => 1,
                (Dup2X2Info::Form1, 1) => 0,
                (Dup2X2Info::Form1, 2) => 3,
                (Dup2X2Info::Form1, 3) => 2,
                (Dup2X2Info::Form1, 4) => 1,
                (Dup2X2Info::Form1, 5) => 0,

                (Dup2X2Info::Form2, 0) => 0,
                (Dup2X2Info::Form2, 1) => 2,
                (Dup2X2Info::Form2, 2) => 1,
                (Dup2X2Info::Form2, 3) => 0,

                (Dup2X2Info::Form3, 0) => 1,
                (Dup2X2Info::Form3, 1) => 0,
                (Dup2X2Info::Form3, 2) => 2,
                (Dup2X2Info::Form3, 3) => 1,
                (Dup2X2Info::Form3, 4) => 0,

                (Dup2X2Info::Form4, 0) => 0,
                (Dup2X2Info::Form4, 1) => 1,
                (Dup2X2Info::Form4, 2) => 0,

                (_, _) => return None,
            })
            .into(),
        )
    }

    fn push_count(&self) -> usize {
        match self {
            Dup2X2Info::Form1 => 6,
            Dup2X2Info::Form2 => 4,
            Dup2X2Info::Form3 => 5,
            Dup2X2Info::Form4 => 3,
        }
    }
}
empty_locals_out!(Dup2X2Info);
empty_locals_in!(Dup2X2Info);

impl HasStackInfo for Dup2X2 {
    type Output = Dup2X2Info;

    fn stack_info(
        &self,
        _: &mut ClassNames,
        _: &ClassFileInfo,
        _: ExactMethodId,
        stack_sizes: StackSizes,
    ) -> Result<Self::Output, StepError> {
        let first = stack_sizes[0].ok_or(StackInfoError::NeededStackSizeAt(0))?;
        if first == Category::One {
            // Must be Form1 or Form3
            let second = stack_sizes[1].ok_or(StackInfoError::NeededStackSizeAt(1))?;
            if second == Category::Two {
                return Err(StackInfoError::BadStackSizes.into());
            }

            let third = stack_sizes[2].ok_or(StackInfoError::NeededStackSizeAt(2))?;
            if third == Category::Two {
                return Ok(Dup2X2Info::Form3);
            }

            let fourth = stack_sizes[3].ok_or(StackInfoError::NeededStackSizeAt(3))?;
            if fourth == Category::One {
                return Ok(Dup2X2Info::Form1);
            }
        } else {
            // Must be Form2 or Form4
            let second = stack_sizes[1].ok_or(StackInfoError::NeededStackSizeAt(1))?;
            if second == Category::Two {
                return Ok(Dup2X2Info::Form4);
            }

            let third = stack_sizes[2].ok_or(StackInfoError::NeededStackSizeAt(2))?;
            if third == Category::One {
                return Ok(Dup2X2Info::Form2);
            }
        }

        Err(StackInfoError::BadStackSizes.into())
    }
}
