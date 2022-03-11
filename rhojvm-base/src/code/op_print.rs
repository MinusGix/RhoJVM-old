use classfile_parser::{
    constant_info::{ClassConstant, ConstantInfo, NameAndTypeConstant},
    constant_pool::ConstantPoolIndexRaw,
};

use crate::{class::ClassFileData, code::method::MethodDescriptor, data::class_names::ClassNames};

use super::op::Inst;

struct FormatInst<'a, 'b> {
    class_names: &'a mut ClassNames,
    class_file: &'b ClassFileData,
}
impl<'a, 'b> FormatInst<'a, 'b> {
    fn single<T: TryFrom<ConstantInfo>>(
        &mut self,
        inst: &Inst,
        index: ConstantPoolIndexRaw<T>,
    ) -> String {
        format!(
            "{} @{}",
            inst.name(),
            index_as_pretty_string(self.class_names, self.class_file, index)
        )
    }
}

impl Inst {
    /// This converts the instruction to a nice string for debugging
    /// If there is errors, it returns a representable error since this is
    /// for debugging.
    /// This takes more parameters so that it can provide more detailed information
    /// Otherwise, if the data on them is good enough, just use the debug printing
    pub fn as_pretty_string(
        &self,
        class_names: &mut ClassNames,
        class_file: &ClassFileData,
    ) -> String {
        let mut f = FormatInst {
            class_names,
            class_file,
        };
        match self {
            Inst::InvokeSpecial(x) => f.single(self, x.index),
            Inst::GetStatic(x) => f.single(self, x.index),
            Inst::LoadConstant(x) => f.single(self, x.index),
            Inst::InvokeVirtual(x) => f.single(self, x.index),
            Inst::PutField(x) => f.single(self, x.index),
            Inst::New(x) => f.single(self, x.index),
            Inst::ANewArray(x) => f.single(self, x.index),
            Inst::InvokeStatic(x) => f.single(self, x.index),
            Inst::InvokeInterface(x) => f.single(self, x.index),
            Inst::InvokeDynamic(x) => f.single(self, x.index),
            Inst::CheckCast(x) => f.single(self, x.index),
            Inst::GetField(x) => f.single(self, x.index),
            Inst::LoadConstant2Wide(x) => f.single(self, x.index),
            Inst::AStore(x) => format!("{} to [{}]", self.name(), x.index),
            Inst::ALoad(x) => format!("{} from [{}]", self.name(), x.index),
            Inst::IntStore(x) => format!("{} to [{}]", self.name(), x.index),
            Inst::IntLoad(x) => format!("{} from [{}]", self.name(), x.index),
            Inst::IntIncrement(x) => format!(
                "{} [{}] = [{}] + {}",
                self.name(),
                x.index,
                x.index,
                x.increment_amount
            ),
            Inst::LongStore(x) => format!("{} to [{}]", self.name(), x.index),
            Inst::LongLoad(x) => format!("{} from [{}]", self.name(), x.index),
            Inst::PushByte(x) => format!("{} #{}", self.name(), x.val),
            Inst::PutStaticField(x) => f.single(self, x.index),

            Inst::MultiANewArray(x) => format!("{} [{}]", f.single(self, x.index), x.dimensions),

            // TODO: Include the exact instruction they'd goto. We need to give this func the index
            Inst::Goto(n) => format!("{} {}", self.name(), n.branch_offset),
            _ => format!("{:?}", self),
        }
    }
}

fn index_as_pretty_string<T: TryFrom<ConstantInfo>>(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
    index: ConstantPoolIndexRaw<T>,
) -> String {
    let index = index.into_generic();
    if let Some(value) = class_file.get_t(index) {
        match value {
            ConstantInfo::Utf8(v) => format!("utf8\"{}\"", v.as_text(&class_file.class_file_data)),
            ConstantInfo::Integer(v) => format!("{}", v.value),
            ConstantInfo::Float(v) => format!("{}_f", v.value),
            ConstantInfo::Long(v) => format!("{}_l", v.value),
            ConstantInfo::Double(v) => format!("{}_d", v.value),
            ConstantInfo::Class(class) => {
                if let Some(class_name) = class_file.get_text_t(class.name_index) {
                    format!(
                        "Class({} = {})",
                        class_name.as_ref(),
                        class_names
                            .gcid_from_bytes(class_file.get_text_b(class.name_index).unwrap())
                            .get()
                    )
                } else {
                    format!(
                        "Class[BAD POOL INDEX #{}->#{}]",
                        index.0, class.name_index.0
                    )
                }
            }
            ConstantInfo::String(v) => {
                if let Some(text) = class_file.get_text_t(v.string_index) {
                    format!("str\"{}\"", text)
                } else {
                    format!("str[BAD POOL INDEX #{}->#{}]", index.0, v.string_index.0)
                }
            }
            ConstantInfo::FieldRef(field) => {
                let class_name = if let Some(class) = class_file.get_t(field.class_index) {
                    if let Some(text) = class_file.get_text_t(class.name_index) {
                        text.into_owned()
                    } else {
                        format!(
                            "[BadClassNameIndex #{} -> {}]",
                            field.class_index.0, class.name_index.0
                        )
                    }
                } else {
                    format!("[BadClassIndex #{}]", field.class_index.0)
                };

                if let Some(nat) = class_file.get_t(field.name_and_type_index) {
                    let name = if let Some(name) = class_file.get_text_t(nat.name_index) {
                        name.into_owned()
                    } else {
                        format!(
                            "[BadNameIndex #{} -> #{}]",
                            field.name_and_type_index.0, nat.name_index.0
                        )
                    };

                    let typ = if let Some(typ) = class_file.get_text_t(nat.descriptor_index) {
                        // TODO: Parse it?
                        typ.into_owned()
                    } else {
                        format!(
                            "[BadDescriptorIndex #{} -> #{}]",
                            field.name_and_type_index.0, nat.descriptor_index.0
                        )
                    };

                    format!("{}::{}:{}", class_name, name, typ)
                } else {
                    format!(
                        "{}::[BadNatIndex #{}]",
                        class_name, field.name_and_type_index.0
                    )
                }
            }
            ConstantInfo::MethodRef(method) => method_to_string(
                class_names,
                class_file,
                index,
                method.class_index,
                method.name_and_type_index,
            ),
            ConstantInfo::InterfaceMethodRef(method) => method_to_string(
                class_names,
                class_file,
                index,
                method.class_index,
                method.name_and_type_index,
            ),
            ConstantInfo::NameAndType(nat) => {
                let name = if let Some(name) = class_file.get_text_t(nat.name_index) {
                    name.into_owned()
                } else {
                    format!("[BadNameIndex #{} -> #{}]", index.0, nat.name_index.0)
                };

                let typ = if let Some(typ) = class_file.get_text_t(nat.descriptor_index) {
                    // TODO: Parse it?
                    typ.into_owned()
                } else {
                    format!(
                        "[BadDescriptorIndex #{} -> #{}]",
                        index.0, nat.descriptor_index.0
                    )
                };

                format!("{}:{}", name, typ)
            }
            // TODO: Good representation for this
            ConstantInfo::MethodHandle(handle) => format!("{:?}", handle),
            ConstantInfo::MethodType(typ) => {
                if let Some(descriptor) = class_file.get_text_t(typ.descriptor_index) {
                    format!("MT[{}]", descriptor)
                } else {
                    format!(
                        "MethodType[BadDescriptorIndex #{} -> #{}]",
                        index.0, typ.descriptor_index.0
                    )
                }
            }
            ConstantInfo::InvokeDynamic(inv) => {
                let meth = method_to_string(
                    class_names,
                    class_file,
                    index,
                    None,
                    inv.name_and_type_index,
                );
                format!("Bootstrap: {}; {}", inv.bootstrap_method_attr_index, meth)
            }
            ConstantInfo::Unusable => "[Unusable Upper Bits]".to_owned(),
        }
    } else {
        format!("[BAD POOL INDEX #{}]", index.0)
    }
}

fn method_to_string(
    class_names: &mut ClassNames,
    class_file: &ClassFileData,
    _index: ConstantPoolIndexRaw<ConstantInfo>,
    class_index: impl Into<Option<ConstantPoolIndexRaw<ClassConstant>>>,
    nat_index: ConstantPoolIndexRaw<NameAndTypeConstant>,
) -> String {
    let class_index = class_index.into();
    let class_name = if let Some(class_index) = class_index {
        if let Some(class) = class_file.get_t(class_index) {
            if let Some(text) = class_file.get_text_t(class.name_index) {
                text.into_owned()
            } else {
                format!(
                    "[BadClassNameIndex #{} -> {}]",
                    class_index.0, class.name_index.0
                )
            }
        } else {
            format!("[BadClassIndex #{}]", class_index.0)
        }
    } else {
        "".to_owned()
    };

    if let Some(nat) = class_file.get_t(nat_index) {
        let name = if let Some(name) = class_file.get_text_t(nat.name_index) {
            name.into_owned()
        } else {
            format!("[BadNameIndex #{} -> #{}]", nat_index.0, nat.name_index.0)
        };

        let typ = if let Some(typ) = class_file.get_text_b(nat.descriptor_index) {
            if let Ok(method_descriptor) = MethodDescriptor::from_text(typ, class_names) {
                method_descriptor.as_pretty_string(class_names)
            } else {
                format!(
                    "[BadMethodDescriptor {}]",
                    class_file.get_text_t(nat.descriptor_index).unwrap()
                )
            }
        } else {
            format!(
                "[BadDescriptorIndex #{} -> #{}]",
                nat_index.0, nat.descriptor_index.0
            )
        };

        format!("{}::{}:{}", class_name, name, typ)
    } else {
        format!("{}::[BadNatIndex #{}]", class_name, nat_index.0)
    }
}
