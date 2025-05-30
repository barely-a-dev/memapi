use crate::{Alloc, AllocError, layout_or_sz_align};
#[cfg(feature = "metadata")]
use crate::{PtrProps, UnsizedCopy};
#[cfg(feature = "clone_to_uninit")]
use core::clone::CloneToUninit;
#[cfg(feature = "metadata")]
use core::ptr::{self, metadata};
use core::{alloc::Layout, ptr::NonNull};

/// Extension methods for the core [`Alloc`] trait, providing convenient
/// routines to allocate, initialize, clone, copy, and deallocate sized
/// and unsized types.
///
/// These helpers simplify common allocation patterns by combining
/// `alloc`, writes, drops, and deallocations for various data shapes.
pub trait AllocExt: Alloc {
    /// Allocates uninitialized memory for a single `T` and writes `data` into it.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    fn alloc_write<T>(&self, data: T) -> Result<NonNull<T>, AllocError> {
        match self.alloc(Layout::new::<T>()) {
            Ok(ptr) => Ok(unsafe {
                let ptr = ptr.cast();
                ptr.write(data);
                ptr
            }),
            Err(e) => Err(e),
        }
    }

    #[cfg(not(feature = "clone_to_uninit"))]
    /// Allocates uninitialized memory for a single `T` and clones `data` into it.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    fn alloc_clone_to<T: Clone>(&self, data: &T) -> Result<NonNull<T>, AllocError> {
        match self.alloc(Layout::new::<T>()) {
            Ok(ptr) => Ok(unsafe {
                let ptr = ptr.cast();
                ptr.write(data.clone());
                ptr
            }),
            Err(e) => Err(e),
        }
    }

    #[cfg(all(feature = "clone_to_uninit", feature = "metadata"))]
    /// Allocates uninitialized memory for a single `T` and clones `data` into it.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    fn alloc_clone_to<T: CloneToUninit + ?Sized>(
        &self,
        data: &T,
    ) -> Result<NonNull<T>, AllocError> {
        match self.alloc(Layout::for_value::<T>(data)) {
            Ok(ptr) => Ok(unsafe {
                data.clone_to_uninit(ptr.as_ptr());
                NonNull::from_raw_parts(ptr, data.metadata())
            }),
            Err(e) => Err(e),
        }
    }

    #[cfg(all(feature = "clone_to_uninit", not(feature = "metadata")))]
    /// Allocates uninitialized memory for a single `T` and clones `data` into it.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    fn alloc_clone_to<T: CloneToUninit>(&self, data: &T) -> Result<NonNull<T>, AllocError> {
        match self.alloc(Layout::for_value::<T>(data)) {
            Ok(ptr) => Ok(unsafe {
                data.clone_to_uninit(ptr.as_ptr());
                ptr.cast()
            }),
            Err(e) => Err(e),
        }
    }

    /// Allocates uninitialized memory for a slice of `T` and clones each element.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    fn alloc_clone_slice_to<T: Clone>(&self, data: &[T]) -> Result<NonNull<[T]>, AllocError> {
        match self.alloc(Layout::for_value(data)) {
            Ok(ptr) => Ok(unsafe {
                let ptr = ptr.cast();
                for (i, elem) in data.iter().enumerate() {
                    ptr.add(i).write(elem.clone());
                }
                NonNull::slice_from_raw_parts(ptr, data.len())
            }),
            Err(e) => Err(e),
        }
    }

    /// Allocates uninitialized memory for a slice of `T` of length `len` and fills each element
    /// with the result of `f(elem_idx)`.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    /// - [`AllocError::LayoutError`] if the computed layout is invalid.
    #[track_caller]
    #[inline]
    fn alloc_slice_with<T, F: Fn(usize) -> T>(
        &self,
        len: usize,
        f: F,
    ) -> Result<NonNull<[T]>, AllocError> {
        match self.alloc(
            layout_or_sz_align::<T>(len).map_err(|(sz, aln)| AllocError::LayoutError(sz, aln))?,
        ) {
            Ok(ptr) => Ok(unsafe {
                let ptr = ptr.cast();
                for i in 0..len {
                    ptr.add(i).write(f(i));
                }
                NonNull::slice_from_raw_parts(ptr, len)
            }),
            Err(e) => Err(e),
        }
    }

    /// Deallocates a previously cloned or written slice of `T`.
    ///
    /// # Safety
    ///
    /// - `slice_ptr` must point to a block of memory allocated using this allocator.
    #[track_caller]
    #[inline]
    unsafe fn dealloc_slice<T>(&self, slice_ptr: NonNull<[T]>) {
        self.dealloc(
            slice_ptr.cast::<u8>(),
            Layout::for_value(&*slice_ptr.as_ptr()),
        );
    }

    /// Drops and deallocates a previously cloned or written slice of `T`.
    ///
    /// # Safety
    ///
    /// - `slice_ptr` must point to a block of memory allocated using this allocator, be valid for
    ///   reads and writes, aligned, and a valid `T`.
    #[track_caller]
    #[inline]
    unsafe fn drop_and_dealloc_slice<T>(&self, slice_ptr: NonNull<[T]>) {
        slice_ptr.drop_in_place();
        self.dealloc_n(slice_ptr);
    }

    #[cfg(feature = "metadata")]
    /// Allocates and copies an unsized `T` by reference, returning a `NonNull<T>`.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    fn alloc_copy_ref_to<T: ?Sized + UnsizedCopy>(
        &self,
        data: &T,
    ) -> Result<NonNull<T>, AllocError> {
        unsafe { self.alloc_copy_ref_to_unchecked(data) }
    }

    #[cfg(feature = "metadata")]
    /// Allocates and copies an unsized `T` by raw pointer, returning a `NonNull<T>`.
    ///
    /// # Safety
    ///
    /// - The caller must ensure `data` is a valid pointer to copy from.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    unsafe fn alloc_copy_ptr_to<T: ?Sized + UnsizedCopy>(
        &self,
        data: *const T,
    ) -> Result<NonNull<T>, AllocError> {
        unsafe { self.alloc_copy_ptr_to_unchecked(data) }
    }

    #[cfg(feature = "metadata")]
    /// Allocates and copies an unsized `T` by reference without requiring
    /// `T: `[`UnsizedCopy`](UnsizedCopy), returning a `NonNull<T>`.
    ///
    /// # Safety
    ///
    /// - The caller must ensure `data` is safe to copy.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    unsafe fn alloc_copy_ref_to_unchecked<T: ?Sized>(
        &self,
        data: &T,
    ) -> Result<NonNull<T>, AllocError> {
        match self.alloc(Layout::for_value(data)) {
            Ok(ptr) => Ok({
                ptr.copy_from_nonoverlapping(*ptr::from_ref(data).cast(), size_of_val::<T>(data));
                NonNull::from_raw_parts(ptr, metadata(&raw const *data))
            }),
            Err(e) => Err(e),
        }
    }

    #[cfg(feature = "metadata")]
    /// Allocates and copies an unsized `T` by raw pointer without requiring
    /// `T: `[`UnsizedCopy`](UnsizedCopy), returning a `NonNull<T>`.
    ///
    /// # Safety
    ///
    /// - The caller must ensure `data` is safe to copy.
    ///
    /// # Errors
    ///
    /// - [`AllocError::AllocFailed`] if allocation fails.
    #[track_caller]
    #[inline]
    unsafe fn alloc_copy_ptr_to_unchecked<T: ?Sized + UnsizedCopy>(
        &self,
        data: *const T,
    ) -> Result<NonNull<T>, AllocError> {
        match self.alloc(Layout::for_value(&*data)) {
            Ok(ptr) => Ok({
                ptr.copy_from_nonoverlapping(*data.cast(), size_of_val::<T>(&*data));
                NonNull::from_raw_parts(ptr, metadata(data))
            }),
            Err(e) => Err(e),
        }
    }
}

impl<A: Alloc> AllocExt for A {}
