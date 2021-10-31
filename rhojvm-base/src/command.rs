use std::{borrow::Cow, path::PathBuf};

use crate::{
    class::ArrayClass,
    code::method::MethodDescriptor,
    id::{ClassFileId, ClassId, MethodId},
    ProgramInfo, StepError,
};

// These Boxes don't matter because:
// - if it is a closure, it needs to be allocated anyway (or some complicated stack scheme)
// - if it is a solid fn, it doesn't have to allocate and only stores the vtable
// - Though, if it a general fn() type, I'm not sure that it gets the vtable, but that is
// probably less common.

// The reason that these callbacks take simply the id is made up of two parts
// - We want to incentivize zst closures, and so even for methods where you have to know
//     the id to create them, you would have to move and so not be a cheap.
// - We want to call the callback even if the class is already loaded.
//     If we passed the the type directly in (before we insert it), we could give a reference
//     but that could make aggressive queue commands behave weirdly
//       ex: aggressively loading a method in your LoadMethod callback, so then it loads
//           and then gets overwritten once your cb is done
// It would be somewhat nice to be able to have a type that says 'this index must exist'
// which only work for the callback, but that isn't possible.
pub type ProgCb = Box<dyn FnOnce(&mut ProgramInfo) -> Result<(), StepError>>;
pub type LoadClassCb = Box<dyn FnOnce(&mut ProgramInfo, ClassId) -> Result<(), StepError>>;
pub type LoadClassMultCb = Box<dyn Fn(&mut ProgramInfo, ClassId) -> Result<(), StepError>>;
pub type LoadMethodFromIndexCb =
    Box<dyn FnOnce(&mut ProgramInfo, MethodId) -> Result<(), StepError>>;
pub type LoadMethodFromDescCb =
    Box<dyn FnOnce(&mut ProgramInfo, MethodId) -> Result<(), StepError>>;
pub type ForAllMethodsCb = Box<dyn Fn(&mut ProgramInfo, MethodId) -> Result<(), StepError>>;
/// (_, _, had code: bool)
/// Note that if it had code and we already parsed it, `had_code` will also be `true`
pub type LoadMethodCodeCb =
    Box<dyn FnOnce(&mut ProgramInfo, MethodId, bool) -> Result<(), StepError>>;

#[non_exhaustive]
pub(crate) enum ClassFileCommand {
    /// Load the class file at the given path if it doesn't already exist
    LoadClassFile { id: ClassFileId, rel_path: PathBuf },
}

#[non_exhaustive]
pub(crate) enum ClassCommand {
    /// Load the class from the class file id if it doesn't already exist
    /// Pre-ordering: LoadClassFile
    LoadClass {
        class_file_id: ClassFileId,
    },
    /// Pre-ordering: LoadClassFile
    LoadClassCb {
        class_file_id: ClassFileId,
        cb: LoadClassCb,
    },
    /// Load all of the super classes of a given class
    /// Does minimal verification
    /// Pre-ordering: LoadClass (class_id), and loads the super classes
    LoadSuperClassesCb {
        class_id: ClassId,
        /// Ran on each entry, not including the class_id
        entry_cb: LoadClassMultCb,
        /// Ran when the search is done
        done_cb: ProgCb,
    },
    RegisterArrayClass {
        array: ArrayClass,
    },
}

#[non_exhaustive]
pub(crate) enum MethodCommand {
    /// Pre-ordering: LoadClass
    LoadMethodFromId {
        method_id: MethodId,
    },
    /// Note: the method is not yet put into the program when it is handed down
    /// Pre-ordering: LoadClass
    LoadMethodFromIdCb {
        method_id: MethodId,
        cb: LoadMethodFromIndexCb,
    },
    /// Note: theDebug method is not yet put into the program when it is handed down
    /// Pre-ordering: LoadClass
    LoadMethodFromDescCb {
        class_id: ClassId,
        name: Cow<'static, str>,
        desc: MethodDescriptor,
        cb: LoadMethodFromDescCb,
    },
    /// If the method has not already gotten the method(s) it overrides, then do that.
    /// Pre-Ordering: Method, and has to load the super classes
    InitMethodOverrides {
        method_id: MethodId,
    },
    /// Pre-ordering: LoadMethod
    LoadMethodDescriptorTypes {
        method_id: MethodId,
    },
    /// Pre-ordering: LoadMethod
    VerifyMethodAccessFlags {
        method_id: MethodId,
    },
    /// Iterate over all the methods that a class has
    /// Note that it doesn't actually load them, just giving the id so that other commands
    /// can be used upon them
    /// This could be done manually by library-users, but is easier with this, and allows
    /// the library-user to avoid manually constructing valid [`MethodId`]s.
    ForAllMethods {
        class_id: ClassId,
        cb: ForAllMethodsCb,
    },
    Code(MethodCodeCommand),
}

#[non_exhaustive]
pub(crate) enum MethodCodeCommand {
    /// Pre-ordering: Method
    LoadMethodCodeCb {
        method_id: MethodId,
        /// Callback to run once it has parsed the code
        /// It is not ran if there was no code
        cb: LoadMethodCodeCb,
    },
    /// Pre-ordering: LoadMethodCode
    VerifyCodeExceptions { method_id: MethodId },
}

#[non_exhaustive]
pub(crate) enum Command {
    ClassFile(ClassFileCommand),
    Class(ClassCommand),
    Method(MethodCommand),

    /// A callback to simply do something
    /// This is useful for queueing a bunch of commands and then doing something
    /// NOTE: Those commands might push commands to run, which would then be ran *after this*.
    /// If you want to wait until the command queue is completely empty, then use DoFinal
    DoMut {
        cb: ProgCb,
    },
    // TODO: A DoFinalMut might be nice, but implementing it so it will work if there is multiple of
    // them is a bit rough.
    // It would essentially require that it be ran last. With multiple, it would hopefully preserve
    // order
}
impl From<ClassFileCommand> for Command {
    fn from(v: ClassFileCommand) -> Self {
        Self::ClassFile(v)
    }
}
impl From<ClassCommand> for Command {
    fn from(v: ClassCommand) -> Self {
        Self::Class(v)
    }
}
impl From<MethodCommand> for Command {
    fn from(v: MethodCommand) -> Self {
        Self::Method(v)
    }
}
impl From<MethodCodeCommand> for Command {
    fn from(v: MethodCodeCommand) -> Self {
        Self::Method(MethodCommand::Code(v))
    }
}
