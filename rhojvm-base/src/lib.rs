#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
// This would be nice to re-enable eventually, but not while in active dev
#![allow(clippy::missing_errors_doc)]
// Shadowing is nice.
#![allow(clippy::shadow_unrelated)]
// Not awful, but it highlights entire function.
#![allow(clippy::unnecessary_wraps)]
// Cool idea but highlights entire function and is too aggressive.
#![allow(clippy::option_if_let_else)]
// It would be nice to have this, but: active dev, it highlights entire function, and currently the
// code unwraps in a variety of cases where it knows it is valid.
#![allow(clippy::missing_panics_doc)]
// This is nice to have for cases where we might want to rely on it not returning anything.
#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::unused_self)]
#![allow(clippy::enum_variant_names)]
#![allow(clippy::too_many_lines)]
// The way this library is designed has many arguments. Grouping them together would be nice for
// readability, but it makes it harder to minimize dependnecies which has other knock-on effects..
#![allow(clippy::too_many_arguments)]
// This is nice to have, but it activates on cheaply constructible enumeration variants.
#![allow(clippy::or_fun_call)]

use class::ClassFileIndexError;

use code::{stack_map::StackMapError, types::StackInfoError};
use data::{
    class_file_loader::LoadClassFileError,
    classes::LoadClassError,
    methods::{LoadCodeError, LoadMethodError, VerifyCodeExceptionError, VerifyMethodError},
};
use id::ClassId;

pub mod class;
pub mod code;
pub mod data;
pub mod id;
pub mod package;
pub mod util;

// Note: Currently all of these errors use non_exhaustive, but in the future that may be removed
// on some if there is a belief that they are likely to be stable.

#[derive(Debug, Clone)]
pub struct BadIdError {
    pub id: ClassId,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum StepError {
    Custom(Box<dyn std::error::Error>),
    LoadClassFile(LoadClassFileError),
    LoadClass(LoadClassError),
    LoadMethod(LoadMethodError),
    VerifyMethod(VerifyMethodError),
    LoadCode(LoadCodeError),
    VerifyCodeException(VerifyCodeExceptionError),
    StackMapError(StackMapError),
    StackInfoError(StackInfoError),
    DescriptorTypeError(classfile_parser::descriptor::DescriptorTypeError),
    /// Some code loaded a value and then tried accessing it but it was missing.
    /// This might be a sign that it shouldn't assume that, or a sign of a bug elsewhere
    /// that caused it to not load but also not reporting an error.
    MissingLoadedValue(&'static str),
    /// Expected a class that wasn't an array
    ExpectedNonArrayClass,
    /// There was a problem indexing into the class file
    ClassFileIndex(ClassFileIndexError),
    /// There was a bad class id that didn't have a name stored
    BadId(BadIdError),
    /// The type held by a descriptor was unexpected
    UnexpectedDescriptorType,
}
impl From<LoadClassFileError> for StepError {
    fn from(err: LoadClassFileError) -> Self {
        Self::LoadClassFile(err)
    }
}
impl From<LoadClassError> for StepError {
    fn from(err: LoadClassError) -> Self {
        Self::LoadClass(err)
    }
}
impl From<LoadMethodError> for StepError {
    fn from(err: LoadMethodError) -> Self {
        Self::LoadMethod(err)
    }
}
impl From<VerifyMethodError> for StepError {
    fn from(err: VerifyMethodError) -> Self {
        Self::VerifyMethod(err)
    }
}
impl From<LoadCodeError> for StepError {
    fn from(err: LoadCodeError) -> Self {
        Self::LoadCode(err)
    }
}
impl From<VerifyCodeExceptionError> for StepError {
    fn from(err: VerifyCodeExceptionError) -> Self {
        Self::VerifyCodeException(err)
    }
}
impl From<StackMapError> for StepError {
    fn from(err: StackMapError) -> Self {
        Self::StackMapError(err)
    }
}
impl From<StackInfoError> for StepError {
    fn from(err: StackInfoError) -> Self {
        Self::StackInfoError(err)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Whether access flags should be verified or not.
    /// Note: This doesn't completely disable the feature, it just stops functions
    /// that do multiple verification steps for you (such as verifying the entirty of a method)
    /// from performing verify method access flags
    /// The user can still manually do such.
    pub verify_method_access_flags: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            verify_method_access_flags: true,
        }
    }
}
