//! ## OptionCell: OnceCell but derivable from Option
//!
//! This library provides an equivalent of [OnceCell](https://doc.rust-lang.org/stable/std/cell/struct.OnceCell.html), but it guarantees layout compatibility with `Option<T>`, providing additional transmute helpers.
//!
//! ## Known use-cases
//!
//! - Implementing the [unification algorithm](https://en.wikipedia.org/wiki/Unification_(computer_science)) without exposing the interior mutability to the user or unnecessarily cloning the value.
//!
//! ## Usage
//!
//! ```txt
//! cargo add option-cell
//! ```
//!
//! ```rust
//! use option_cell::OptionCell;
//!
//! let mut options = vec![None, None];
//! let cells = OptionCell::from_mut_slice(&mut options);
//! cells[0].set(1).unwrap();
//! ```

use std::cell::UnsafeCell;
use std::fmt;

/// An equivalent of [std::cell::OnceCell](https://doc.rust-lang.org/stable/std/cell/struct.OnceCell.html) or [once_cell::unsync::OnceCell](https://docs.rs/once_cell/latest/once_cell/unsync/struct.OnceCell.html)
/// with an additional transmute helper.
/// To guarantee the helper's safety, it is defined as a different type from the original OnceCell.
// Unlike the original OnceCell, we need #[repr(transparent)] to guarantee the layout compatibility
#[repr(transparent)]
pub struct OptionCell<T> {
    // Ownership invariant: same as Option<T>.
    //
    // Shared invariant:
    // It has internally two modes: read and write.
    // - It is in write mode if the value is None
    //   or the control is in a critical section
    //   and the value was None when the critical section started.
    // - It is in read mode if the value is Some(_)
    //   and the write mode is not extended in a critical section in a manner described above.
    //
    // Invariant changes between read and write modes:
    // - In read mode, one has read access to the whole Option<T> (whether or not in a critical section).
    // - In write mode, one has write access to the whole Option<T> when in a critical section.
    inner: UnsafeCell<Option<T>>,
}

impl<T> OptionCell<T> {
    /// Safety requirement: critical sections must not be nested.
    unsafe fn critical_read_section<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&Option<T>) -> R,
    {
        f(&*self.inner.get())
    }

    /// Safety requirement: critical sections must not be nested.
    /// Additionally, the caller must ensure that the value is None before entering the section.
    unsafe fn critical_write_section<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut Option<T>) -> R,
    {
        f(&mut *self.inner.get())
    }

    /// Creates a new empty cell.
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(None),
        }
    }

    /// Gets the reference to the underlying value.
    /// Returns `None` if the cell is empty.
    pub fn get(&self) -> Option<&T> {
        // Safety: critical section can always read. Then,
        // - If it is Some(_), it is in read mode.
        //   It is safe to return references as the caller also has the read access.
        // - If it is None, it returns the None value.
        //   That means no references are exposed to the caller.
        //
        // It does not use the critical section helper to extend the reference's lifetime.
        // Nevertheless it constitutes a critical section.
        unsafe { &*self.inner.get() }.as_ref()
    }

    /// Gets the mutable reference to the underlying Option.
    ///
    /// Unlike the original OnceCell, this method returns a mutable reference to the whole Option<T>,
    /// as the layout is guaranteed.
    pub fn get_mut(&mut self) -> &mut Option<T> {
        // Safety: the ownership invariant is the same as Option<T>
        unsafe { &mut *self.inner.get() }
    }

    /// Sets the contents of this cell to `value`.
    pub fn set(&self, value: T) -> Result<(), T> {
        let is_none = unsafe { self.critical_read_section(|opt| opt.is_none()) };
        if is_none {
            unsafe {
                self.critical_write_section(|opt| *opt = Some(value));
            }
            Ok(())
        } else {
            Err(value)
        }
    }

    /// Gets the contents of the cell, initializing with `f` if the cell was empty.
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if let Some(value) = self.get() {
            value
        } else {
            let value = f();
            if self.set(value).is_err() {
                panic!("Recursive initialization within get_or_init");
            }
            self.get().unwrap()
        }
    }

    /// Consumes the cell, returning the wrapped Option<T>.
    pub fn into_inner(self) -> Option<T> {
        self.inner.into_inner()
    }

    /// Takes the value out of this cell, leaving it empty.
    pub fn take(&mut self) -> Option<T> {
        self.get_mut().take()
    }

    /// Converts an existing mutable reference into OptionCell.
    pub fn from_mut(slice: &mut Option<T>) -> &mut Self {
        // Safety: layout is compatible as observed in Cell.
        // The ownership invariant is the same.
        unsafe { &mut *(slice as *mut Option<T> as *mut Self) }
    }

    /// Converts an existing mutable slice into a slice of OptionCell.
    pub fn from_mut_slice(slice: &mut [Option<T>]) -> &mut [Self] {
        // Safety: layout is compatible as observed in Cell.
        // The ownership invariant is the same.
        unsafe { std::slice::from_raw_parts_mut(slice.as_mut_ptr() as *mut Self, slice.len()) }
    }
}

impl<T> From<Option<T>> for OptionCell<T> {
    fn from(opt: Option<T>) -> Self {
        Self {
            inner: UnsafeCell::new(opt),
        }
    }
}

impl<T> Default for OptionCell<T> {
    fn default() -> Self {
        OptionCell::from(None)
    }
}

impl<T> From<OptionCell<T>> for Option<T> {
    fn from(cell: OptionCell<T>) -> Self {
        cell.into_inner()
    }
}

impl<T> Clone for OptionCell<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        OptionCell::from(self.get().cloned())
    }
}

impl<T> PartialEq<OptionCell<T>> for OptionCell<T>
where
    T: PartialEq<T>,
{
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }

    fn ne(&self, other: &Self) -> bool {
        self.get() != other.get()
    }
}

impl<T> fmt::Debug for OptionCell<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OptionCell").field(&self.get()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_get() {
        let cell = OptionCell::<i32>::new();
        assert_eq!(cell.get(), None);
    }

    #[test]
    fn test_set_get() {
        let cell = OptionCell::<i32>::new();
        let cell_ref1 = &cell;
        let cell_ref2 = &cell;
        let cell_ref3 = &cell;
        assert_eq!(cell_ref1.get(), None);
        cell_ref2.set(42).unwrap();
        assert_eq!(cell_ref3.get(), Some(&42));
    }

    #[test]
    fn test_set_fail_get() {
        let cell = OptionCell::<i32>::new();
        cell.set(42).unwrap();
        let cell_ref1 = &cell;
        let cell_ref2 = &cell;
        assert!(cell_ref1.set(43).is_err());
        assert_eq!(cell_ref2.get(), Some(&42));
    }

    #[test]
    fn test_from_mut() {
        {
            let mut opt = Some(42);
            let cell = OptionCell::from_mut(&mut opt);
            let cell_ref1 = &*cell;
            let cell_ref2 = &*cell;
            assert_eq!(cell_ref1.get(), Some(&42));
            assert!(cell_ref2.set(43).is_err());
        }
        {
            let mut opt = None;
            let cell = OptionCell::from_mut(&mut opt);
            assert_eq!(cell.get(), None);
            assert!(cell.set(43).is_ok());
            assert_eq!(opt, Some(43));
        }
    }

    #[test]
    fn test_from_mut_slice() {
        let mut opts = vec![Some(42), None, Some(43)];
        let cells = OptionCell::from_mut_slice(&mut opts);
        let cells_ref1 = &*cells;
        let cells_ref2 = &*cells;
        let cells_ref3 = &*cells;
        assert_eq!(cells_ref1.len(), 3);
        assert_eq!(cells_ref1[0].get(), Some(&42));
        assert_eq!(cells_ref1[1].get(), None);
        assert_eq!(cells_ref1[2].get(), Some(&43));

        assert!(cells_ref2[1].set(44).is_ok());
        assert_eq!(cells_ref3[1].get(), Some(&44));
    }
}
