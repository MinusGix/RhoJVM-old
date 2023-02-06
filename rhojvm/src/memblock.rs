//! This implements a memory block allocation structure, primarily for java's Unsafe class which
//! allows it to manually allocate memory.
//! This is very unsafe.

use std::alloc::LayoutError;

#[derive(Debug)]
pub enum MemoryAllocError {
    /// The layout was invalid
    Layout(LayoutError),
    /// An unknown error in allocating
    /// This may be out of memory, or the allocator not supporting the given layout
    AllocationFailure,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MemoryBlockPtr(*mut u8);
impl MemoryBlockPtr {
    /// # Safety
    /// The pointer should be valid to dereference
    #[must_use]
    pub unsafe fn new_unchecked(ptr: *mut u8) -> MemoryBlockPtr {
        MemoryBlockPtr(ptr)
    }

    #[must_use]
    pub fn get(self) -> *mut u8 {
        self.0
    }
}

#[derive(Debug, Default)]
pub struct MemoryBlocks {
    blocks: Vec<MemoryBlock>,
}
impl MemoryBlocks {
    pub fn allocate_block(&mut self, size: usize) -> Result<MemoryBlockPtr, MemoryAllocError> {
        let block = MemoryBlock::alloc(size)?;
        let ptr = block.data;

        self.blocks.push(block);

        Ok(ptr)
    }

    /// Returns whether it was able to deallocate the block
    pub fn deallocate_block(&mut self, ptr: MemoryBlockPtr) -> bool {
        if let Some((index, _)) = self.blocks.iter().enumerate().find(|(_, x)| x.data == ptr) {
            let block = self.blocks.remove(index);
            // The allocation should still be valid since it was held within this structure and not // freely given out
            unsafe { block.dealloc() };
            true
        } else {
            false
        }
    }

    // We allow unused mut self to be more explicit that we are only writing to it (at least within
    // our api) when we have unique access.
    #[allow(clippy::unused_self)]
    pub(crate) unsafe fn write_slice(&mut self, ptr: MemoryBlockPtr, slice: &[u8]) {
        let ptr = ptr.get();

        assert!(
            isize::try_from(slice.len()).is_ok(),
            "Slice was too big for proper pointer arithmetic"
        );
        for (offset, byte) in slice.iter().copied().enumerate() {
            #[allow(clippy::cast_possible_wrap)]
            let offset = offset as isize;
            let ptr_at = ptr.offset(offset);
            *ptr_at = byte;
        }
    }

    #[allow(clippy::unused_self)]
    pub unsafe fn write_repeat(&mut self, ptr: MemoryBlockPtr, count: usize, val: u8) {
        let ptr = ptr.get();

        assert!(
            isize::try_from(count).is_ok(),
            "Count was too big for proper pointer arithmetic"
        );
        for offset in 0..count {
            #[allow(clippy::cast_possible_wrap)]
            let offset = offset as isize;
            let ptr_at = ptr.offset(offset);
            *ptr_at = val;
        }
    }

    // TODO: Debug checks that ensure that pointers that we are writing to are inside our memory
    // blocks

    #[allow(clippy::unused_self)]
    unsafe fn read_amount<const N: usize>(&mut self, ptr: MemoryBlockPtr) -> [u8; N] {
        let ptr = ptr.get();
        assert!(
            isize::try_from(N).is_ok(),
            "Constant N was too big for proper pointer arithmetic"
        );

        let mut result = [0u8; N];
        for (offset, item) in result.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_wrap)]
            let soffset = offset as isize;
            let ptr_at = ptr.offset(soffset);
            *item = *ptr_at;
        }

        result
    }

    pub unsafe fn read_slice<'a>(&mut self, ptr: MemoryBlockPtr, count: usize) -> &'a [u8] {
        let ptr = ptr.get();
        assert!(
            isize::try_from(count).is_ok(),
            "Count was too big for proper pointer arithmetic"
        );

        std::slice::from_raw_parts(ptr, count)
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to read the bytes of an f64 from it
    pub unsafe fn get_f64_ne(&mut self, ptr: MemoryBlockPtr) -> f64 {
        f64::from_ne_bytes(self.read_amount::<{ std::mem::size_of::<f64>() }>(ptr))
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to read the bytes of an f32 from it
    pub unsafe fn get_f32_ne(&mut self, ptr: MemoryBlockPtr) -> f32 {
        f32::from_ne_bytes(self.read_amount::<{ std::mem::size_of::<f32>() }>(ptr))
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to read the bytes of an i64 from it
    pub unsafe fn get_i64_ne(&mut self, ptr: MemoryBlockPtr) -> i64 {
        i64::from_ne_bytes(self.read_amount::<{ std::mem::size_of::<i64>() }>(ptr))
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to read the bytes of an i32 from it
    pub unsafe fn get_i32_ne(&mut self, ptr: MemoryBlockPtr) -> i32 {
        i32::from_ne_bytes(self.read_amount::<{ std::mem::size_of::<i32>() }>(ptr))
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to read the bytes of an i16 from it
    pub unsafe fn get_i16_ne(&mut self, ptr: MemoryBlockPtr) -> i16 {
        i16::from_ne_bytes(self.read_amount::<{ std::mem::size_of::<i16>() }>(ptr))
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to read the bytes of an u16 from it
    pub unsafe fn get_u16_ne(&mut self, ptr: MemoryBlockPtr) -> u16 {
        u16::from_ne_bytes(self.read_amount::<{ std::mem::size_of::<u16>() }>(ptr))
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to read the bytes of an i8 from it
    pub unsafe fn get_i8_ne(&mut self, ptr: MemoryBlockPtr) -> i8 {
        i8::from_ne_bytes(self.read_amount::<{ std::mem::size_of::<i8>() }>(ptr))
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to write the bytes of an f64 to it
    pub unsafe fn set_f64_ne(&mut self, ptr: MemoryBlockPtr, val: f64) {
        let bytes = val.to_ne_bytes();

        self.write_slice(ptr, &bytes);
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to write the bytes of an f32 to it
    pub unsafe fn set_f32_ne(&mut self, ptr: MemoryBlockPtr, val: f32) {
        let bytes = val.to_ne_bytes();

        self.write_slice(ptr, &bytes);
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to write the bytes of an i64 to it
    pub unsafe fn set_i64_ne(&mut self, ptr: MemoryBlockPtr, val: i64) {
        let bytes = val.to_ne_bytes();

        self.write_slice(ptr, &bytes);
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to write the bytes of an i32 to it
    pub unsafe fn set_i32_ne(&mut self, ptr: MemoryBlockPtr, val: i32) {
        let bytes = val.to_ne_bytes();

        self.write_slice(ptr, &bytes);
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to write the bytes of an i16 to it
    pub unsafe fn set_i16_ne(&mut self, ptr: MemoryBlockPtr, val: i16) {
        let bytes = val.to_ne_bytes();

        self.write_slice(ptr, &bytes);
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to write the bytes of an i16 to it
    pub unsafe fn set_u16_ne(&mut self, ptr: MemoryBlockPtr, val: u16) {
        let bytes = val.to_ne_bytes();

        self.write_slice(ptr, &bytes);
    }

    /// # Safety
    /// Pointer should be valid and should be owned by [`MemoryBlocks`], if it is not then it is
    /// UB both by Rust and Java's Unsafe spec.
    /// It should be valid to write the bytes of an i8 to it
    pub unsafe fn set_i8_ne(&mut self, ptr: MemoryBlockPtr, val: i8) {
        let bytes = val.to_ne_bytes();

        self.write_slice(ptr, &bytes);
    }
}

/// Note: don't derive clone for this or [`MemoryBlocks`] as the reasonable definition of
/// cloning for them is redoing the allocation and copying.
#[derive(Debug)]
pub struct MemoryBlock {
    data: MemoryBlockPtr,
    size: usize,
    // TODO: We can get rid of this small amount of data by initializing it once in a Lazy/OnceCell
    // once those are standardized
    layout: std::alloc::Layout,
}
impl MemoryBlock {
    /// Find the largest alignment of allowed java types that can be written
    /// Since that is a requirement of the Unsafe api for allocating
    fn alignment() -> usize {
        use std::mem::align_of;

        align_of::<i64>()
            .max(align_of::<i32>())
            .max(align_of::<i16>())
            .max(align_of::<i8>())
            .max(align_of::<bool>())
            .max(align_of::<f32>())
            .max(align_of::<f64>())
    }

    pub(crate) fn alloc(size: usize) -> Result<MemoryBlock, MemoryAllocError> {
        let layout = std::alloc::Layout::from_size_align(size, MemoryBlock::alignment())
            .map_err(MemoryAllocError::Layout)?;

        // Safety:
        // We're treating this as an opaque group of bytes for storage of values
        let data = unsafe { std::alloc::alloc_zeroed(layout) };
        if data.is_null() {
            return Err(MemoryAllocError::AllocationFailure);
        }

        let data = MemoryBlockPtr(data);

        Ok(MemoryBlock { data, size, layout })
    }

    /// # Safety
    /// This should be the only code which deallocates the pointer
    pub(crate) unsafe fn dealloc(self) {
        std::alloc::dealloc(self.data.get(), self.layout);
    }
}
