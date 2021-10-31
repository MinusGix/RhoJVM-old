use std::{borrow::Cow, collections::VecDeque};

use crate::{
    code::method::MethodDescriptor,
    command::{ClassCommand, ClassFileCommand, Command, MethodCodeCommand, MethodCommand},
    id::{ClassFileId, ClassId, MethodId},
    util, ClassFiles, ClassNames, LoadClassFileError, ProgramInfo, StepError,
};

pub struct Queue {
    qu: VecDeque<Command>,
    /// A backup queue for when we want to do immediate processing
    back_qu: VecDeque<Command>,
    /// Whether the current queue is the back queue
    is_back: bool,
}
impl Queue {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            qu: VecDeque::with_capacity(capacity),
            back_qu: VecDeque::new(),
            is_back: false,
        }
    }

    pub(crate) fn swap_qu(&mut self) {
        std::mem::swap(&mut self.qu, &mut self.back_qu);
        self.is_back = !self.is_back;
    }

    #[must_use]
    pub(crate) fn is_back(&self) -> bool {
        self.is_back
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.qu.is_empty()
    }

    /// Add a command to the end of the queue
    pub(crate) fn push(&mut self, cmd: impl Into<Command>) {
        self.qu.push_back(cmd.into())
    }

    /// Pop the most recent command
    pub(crate) fn pop(&mut self) -> Option<Command> {
        self.qu.pop_front()
    }

    pub(crate) fn pre_push(&mut self, cmd: impl Into<Command>) {
        self.qu.push_front(cmd.into());
    }

    // == Utility Methods
    pub fn q_load_method_code(
        &mut self,
        method_id: MethodId,
        cb: impl FnOnce(&mut ProgramInfo, MethodId, bool) -> Result<(), StepError> + 'static,
    ) {
        self.push(MethodCodeCommand::LoadMethodCodeCb {
            method_id,
            cb: Box::new(cb),
        });
    }

    pub fn q_load_method_descriptor_types(&mut self, method_id: MethodId) {
        self.push(MethodCommand::LoadMethodDescriptorTypes { method_id });
    }

    pub fn q_verify_method_access_flags(&mut self, method_id: MethodId) {
        self.push(MethodCommand::VerifyMethodAccessFlags { method_id });
    }

    pub fn q_verify_code_exceptions(&mut self, method_id: MethodId) {
        self.push(MethodCodeCommand::VerifyCodeExceptions { method_id });
    }

    pub fn q_do_mut(
        &mut self,
        cb: impl FnOnce(&mut ProgramInfo) -> Result<(), StepError> + 'static,
    ) {
        self.push(Command::DoMut { cb: Box::new(cb) });
    }

    /// Queue the loading of a class file (to some minimum level), returning the id
    /// that will eventually have it
    pub fn q_load_class_by_class_file_id(&mut self, id: ClassFileId) -> ClassId {
        self.push(ClassCommand::LoadClass { class_file_id: id });

        // Class file ids are equivalent to class ids since they are both hashes of the access path
        id
    }

    pub fn q_load_class_by_class_file_id_cb(
        &mut self,
        id: ClassFileId,
        cb: impl FnOnce(&mut ProgramInfo, ClassId) -> Result<(), StepError> + 'static,
    ) -> ClassId {
        self.push(ClassCommand::LoadClassCb {
            class_file_id: id,
            cb: Box::new(cb),
        });

        id
    }

    pub fn q_load_super_classes_cb(
        &mut self,
        id: ClassId,
        entry: impl Fn(&mut ProgramInfo, ClassId) -> Result<(), StepError> + 'static,
        done: impl FnOnce(&mut ProgramInfo) -> Result<(), StepError> + 'static,
    ) {
        self.push(ClassCommand::LoadSuperClassesCb {
            class_id: id,
            entry_cb: Box::new(entry),
            done_cb: Box::new(done),
        })
    }

    pub fn q_for_all_methods(
        &mut self,
        id: ClassId,
        cb: impl Fn(&mut ProgramInfo, MethodId) -> Result<(), StepError> + 'static,
    ) {
        self.push(MethodCommand::ForAllMethods {
            class_id: id,
            cb: Box::new(cb),
        });
    }

    pub fn q_load_method_by_desc_cb(
        &mut self,
        class_id: ClassId,
        name: impl Into<Cow<'static, str>>,
        desc: MethodDescriptor,
        cb: impl FnOnce(&mut ProgramInfo, MethodId) -> Result<(), StepError> + 'static,
    ) {
        self.push(MethodCommand::LoadMethodFromDescCb {
            class_id,
            name: name.into(),
            desc,
            cb: Box::new(cb),
        });
    }

    pub fn q_load_method_by_id(&mut self, method_id: MethodId) {
        self.push(MethodCommand::LoadMethodFromId { method_id });
    }

    pub fn q_init_method_overrides(&mut self, method_id: MethodId) {
        self.push(MethodCommand::InitMethodOverrides { method_id });
    }

    /// Queue the loading of a class file, returning the id that will eventually
    /// have it.
    pub fn q_load_class_file_by_class_path_slice<T: AsRef<str>>(
        &mut self,
        class_names: &mut ClassNames,
        class_files: &ClassFiles,
        class_path: &[T],
    ) -> Result<ClassFileId, LoadClassFileError> {
        // TODO: Should we be doing these checks here? It would make sense to do them elsewhere
        // but they do guard against useless calls to this that would cause a string allocation
        // we could just check if it already exists, and if it does then just return and don't
        // actually insert the command

        if class_path.is_empty() {
            return Err(LoadClassFileError::EmptyPath);
        }

        let class_file_id: ClassFileId = class_names.gcid_from_slice(class_path);
        if class_files.contains_key(&class_file_id) {
            return Err(LoadClassFileError::AlreadyExists);
        }

        // TODO: include current dir? this could be an option.
        let rel_path = util::class_path_slice_to_relative_path(class_path);

        self.push(ClassFileCommand::LoadClassFile {
            id: class_file_id,
            rel_path,
        });
        Ok(class_file_id)
    }
}
