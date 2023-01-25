use classfile_parser::{
    constant_info::ConstantInfo,
    constant_pool::{ConstantPool, ConstantPoolIndex, ConstantPoolIndexRaw},
};
use indexmap::IndexMap;

use crate::class::InvalidConstantPoolIndex;

pub trait ConstantInfoPool {
    fn get<T>(&self, i: impl TryInto<ConstantPoolIndex<T>>) -> Option<&ConstantInfo>;

    fn get_t<'a, T>(&'a self, i: impl TryInto<ConstantPoolIndex<T>>) -> Option<&'a T>
    where
        &'a T: TryFrom<&'a ConstantInfo>,
    {
        let i: ConstantPoolIndex<T> = i.try_into().ok()?;
        let v: &'a ConstantInfo = self.get(i)?;
        <&'a T>::try_from(v).ok()
    }

    fn getr<'a, T>(&'a self, i: ConstantPoolIndexRaw<T>) -> Result<&'a T, InvalidConstantPoolIndex>
    where
        &'a T: TryFrom<&'a ConstantInfo>,
        T: TryFrom<ConstantInfo>,
    {
        self.get_t(i)
            .ok_or_else(|| InvalidConstantPoolIndex(i.into_generic()))
    }
}

impl ConstantInfoPool for ConstantPool {
    fn get<T>(&self, i: impl TryInto<ConstantPoolIndex<T>>) -> Option<&ConstantInfo> {
        self.get(i)
    }
}

#[derive(Debug, Clone, Default)]
pub struct MapConstantPool {
    pool: IndexMap<usize, ConstantInfo>,
}
impl ConstantInfoPool for MapConstantPool {
    fn get<T>(&self, i: impl TryInto<ConstantPoolIndex<T>>) -> Option<&ConstantInfo> {
        let i: ConstantPoolIndex<T> = i.try_into().ok()?;
        self.pool.get(&(i.0 as usize))
    }
}

#[derive(Debug, Clone)]
pub struct ShadowConstantPool<A: ConstantInfoPool, B: ConstantInfoPool> {
    a: A,
    b: B,
}
impl<A: ConstantInfoPool, B: ConstantInfoPool> ShadowConstantPool<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}
impl<A: ConstantInfoPool, B: ConstantInfoPool> ConstantInfoPool for ShadowConstantPool<A, B> {
    fn get<T>(&self, i: impl TryInto<ConstantPoolIndex<T>>) -> Option<&ConstantInfo> {
        let i: ConstantPoolIndex<T> = i.try_into().ok()?;
        self.a.get(i).or_else(|| self.b.get(i))
    }
}
