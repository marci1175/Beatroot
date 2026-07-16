use windows::{
    Win32::{Foundation::*, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*},
    core::*,
};

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLOSE => {
            let _ = unsafe { DestroyWindow(hwnd) };
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn register_class(class_name: PCWSTR) -> Result<PCWSTR> {
    let instance = unsafe { GetModuleHandleW(None)? };

    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: instance.into(),
        lpszClassName: class_name,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        ..Default::default()
    };

    unsafe { RegisterClassW(&wc) };
    Ok(class_name)
}

pub fn create_window(class_name: PCWSTR, width: i32, height: i32) -> Result<HWND> {
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
            None,
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
