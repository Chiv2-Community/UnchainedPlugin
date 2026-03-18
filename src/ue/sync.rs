use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use windows::Win32::System::Threading::{
    EnterCriticalSection, LeaveCriticalSection, CRITICAL_SECTION,
};

#[derive(Debug)]
#[repr(C)]
pub struct FWindowsCriticalSection(UnsafeCell<CRITICAL_SECTION>);

impl FWindowsCriticalSection {
    fn crit_ptr_mut(&self) -> *mut CRITICAL_SECTION {
        &self.0 as *const _ as *mut _
    }

    pub fn lock(&self) {
        unsafe { EnterCriticalSection(self.crit_ptr_mut()) };
    }

    pub fn unlock(&self) {
        unsafe { LeaveCriticalSection(self.crit_ptr_mut()) };
    }
}

pub struct CriticalSectionGuard<'crit, 'data, T: ?Sized> {
    critical_section: &'crit FWindowsCriticalSection,
    data: &'data UnsafeCell<T>,
}

impl<'crit, 'data, T: ?Sized> CriticalSectionGuard<'crit, 'data, T> {
    pub fn lock(critical_section: &'crit FWindowsCriticalSection, data: &'data UnsafeCell<T>) -> Self {
        critical_section.lock();
        Self {
            critical_section,
            data,
        }
    }
}

impl<T: ?Sized> Drop for CriticalSectionGuard<'_, '_, T> {
    fn drop(&mut self) {
        self.critical_section.unlock();
    }
}

impl<T: ?Sized> Deref for CriticalSectionGuard<'_, '_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.data.get() }
    }
}

impl<T: ?Sized> DerefMut for CriticalSectionGuard<'_, '_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }
}
