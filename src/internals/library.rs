use std::{ffi::CString, path::PathBuf};

use windows::{
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
    core::PCSTR,
};

use crate::internals::mem::str_to_pcwstr;

pub fn load_library(path: &PathBuf) -> anyhow::Result<HMODULE> {
    // Create a PCWSTR from the string
    let (str, _chars) = str_to_pcwstr(&path.to_string_lossy());

    // Load the library and retrive a handle to the library.
    let library_handle = unsafe { LoadLibraryW(str) }?;

    // Return the library's handle
    Ok(library_handle)
}

pub fn unload_library(handle: HMODULE) -> anyhow::Result<()> {
    unsafe { Ok(FreeLibrary(handle)?) }
}

pub fn get_fn_addr(module: HMODULE, name: &str) -> Option<unsafe extern "system" fn() -> isize> {
    let c_name = CString::new(name).ok()?;
    unsafe { GetProcAddress(module, PCSTR(c_name.as_ptr() as *const u8)) }
}
