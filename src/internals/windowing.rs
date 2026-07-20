use std::os::raw::c_void;

use windows::{
    Win32::{Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*},
    core::*,
};

use crate::plugins::PluginWindowState;

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize) };
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CLOSE => {
            let user_data =
                unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut PluginWindowState;
            if !user_data.is_null() {
                let state = unsafe { &*user_data };

                // Call the `on_close` callback of the window
                (state.on_close)();
            }
            let _ = unsafe { DestroyWindow(hwnd) };
            LRESULT(0)
        }
        WM_DESTROY => {
            let user_data =
                unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut PluginWindowState;
            if !user_data.is_null() {
                // We only reacquire the ownership if the pointer will never be accessed again.
                // SAFETY: AFTER `WM_DESTROY` THE WINDOW CANNOT BE ACCESSED AGAIN
                let state = unsafe { Box::from_raw(user_data) };

                // Call the `on_destroy` callback of the window
                (state.on_destroy)();
            }
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn register_class(class_name: PCWSTR) -> Result<PCWSTR> {
    let instance = unsafe { GetModuleHandleW(None)? };

    let wc = WNDCLASSW {
        style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: instance.into(),
        lpszClassName: class_name,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        ..Default::default()
    };

    unsafe { RegisterClassW(&wc) };
    Ok(class_name)
}

pub fn create_window(
    class_name: PCWSTR,
    width: i32,
    height: i32,
    window_state: *mut c_void,
) -> Result<HWND> {
    let instance = unsafe { GetModuleHandleW(None)? };

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST,
            class_name,
            class_name,
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            width,
            height,
            None, // no parent — top-level window
            None, // no menu
            Some(instance.into()),
            Some(window_state),
        )?
    };

    Ok(hwnd)
}

pub fn run_message_loop() {
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
