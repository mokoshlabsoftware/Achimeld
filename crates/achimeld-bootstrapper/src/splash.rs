//! A bootstrapper-owned startup window. It has no package, process or browser authority.
use std::{
    cell::RefCell,
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACKNESS, BeginPaint, DIB_RGB_COLORS, DT_CENTER,
        DT_VCENTER, DT_WORDBREAK, DrawTextW, EndPaint, InvalidateRect, PAINTSTRUCT, PatBlt,
        SRCCOPY, SetBkMode, SetTextColor, StretchDIBits, TRANSPARENT, UpdateWindow,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows,
        GetClientRect, GetSystemMetrics, GetWindowThreadProcessId, IsWindowVisible, MSG, PM_REMOVE,
        PeekMessageW, PostQuitMessage, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW,
        ShowWindow, TranslateMessage, WM_CLOSE, WM_DESTROY, WM_LBUTTONUP, WM_PAINT, WNDCLASSW,
        WS_EX_TOOLWINDOW, WS_POPUP,
    },
};

const IMAGE_WIDTH: u32 = 786;
const IMAGE_HEIGHT: u32 = 515;
const WIDTH: i32 = 393;
const HEIGHT: i32 = 258;
const FAILURE_HEIGHT: i32 = 48;

struct Pixels(Vec<u8>);
thread_local! {
    static PIXELS: RefCell<Option<Pixels>> = const { RefCell::new(None) };
    static FAILED: RefCell<bool> = const { RefCell::new(false) };
}

struct Shared {
    editor_pid: AtomicU32,
    stop: AtomicBool,
    failed: AtomicBool,
}

pub struct Splash {
    shared: Arc<Shared>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Splash {
    pub fn start() -> Self {
        let shared = Arc::new(Shared {
            editor_pid: AtomicU32::new(0),
            stop: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        });
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let state = Arc::clone(&shared);
        let worker = thread::spawn(move || show(state, ready_tx));
        let _ = ready_rx.recv_timeout(Duration::from_secs(5));
        Self {
            shared,
            worker: Some(worker),
        }
    }

    pub fn watch_editor(&self, pid: u32) {
        self.shared.editor_pid.store(pid, Ordering::Release);
    }

    pub fn show_failure(mut self) {
        if self
            .worker
            .as_ref()
            .is_some_and(thread::JoinHandle::is_finished)
        {
            // The editor may have opened and closed before supervision reported
            // failure. Open a new branded error surface in that case.
            self = Self::start();
        }
        self.shared.failed.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Splash {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn show(shared: Arc<Shared>, ready: mpsc::SyncSender<()>) {
    let decoded = match image::load_from_memory(include_bytes!("../assets/loading.png")) {
        Ok(image) => image.to_rgba8(),
        Err(_) => {
            let _ = ready.send(());
            return;
        }
    };
    if decoded.width() != IMAGE_WIDTH || decoded.height() != IMAGE_HEIGHT {
        let _ = ready.send(());
        return;
    }
    let mut pixels = image::imageops::resize(
        &decoded,
        WIDTH as u32,
        HEIGHT as u32,
        image::imageops::FilterType::Lanczos3,
    )
    .into_raw();
    for rgba in pixels.chunks_exact_mut(4) {
        rgba.swap(0, 2); // Windows DIB pixels are BGRA.
    }
    PIXELS.with(|slot| *slot.borrow_mut() = Some(Pixels(pixels)));
    let class: Vec<u16> = "AchimeldStartupSplash\0".encode_utf16().collect();
    let title: Vec<u16> = "Achimeld\0".encode_utf16().collect();
    // SAFETY: Class name and callback remain valid for the lifetime of this UI thread.
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let window_class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    // SAFETY: Win32 copies the class registration; the window is owned by this thread.
    if unsafe { RegisterClassW(&window_class) } == 0 {
        let _ = ready.send(());
        return;
    }
    // SAFETY: Registered class and null optional handles are valid for a top-level popup.
    let window = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            (GetSystemMetrics(SM_CXSCREEN) - WIDTH) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - HEIGHT) / 2,
            WIDTH,
            HEIGHT,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if window.is_null() {
        let _ = ready.send(());
        return;
    }
    // SAFETY: Window handle remains owned by this UI thread until DestroyWindow.
    unsafe {
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
    }
    let _ = ready.send(());
    let mut displayed_failure = false;
    loop {
        let mut message = MSG::default();
        // SAFETY: This thread owns the window and message storage is initialized.
        while unsafe { PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if message.message == windows_sys::Win32::UI::WindowsAndMessaging::WM_QUIT {
                return;
            }
            // SAFETY: The message came from this thread's queue and is live for dispatch.
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if shared.stop.load(Ordering::Acquire) {
            // SAFETY: This thread owns the live splash window.
            unsafe { DestroyWindow(window) };
            return;
        }
        if shared.failed.load(Ordering::Acquire) {
            if !displayed_failure {
                FAILED.with(|slot| *slot.borrow_mut() = true);
                // SAFETY: This thread owns the live splash window; repaint is requested without erasing the image.
                unsafe { InvalidateRect(window, ptr::null(), 0) };
                displayed_failure = true;
            }
        } else {
            let pid = shared.editor_pid.load(Ordering::Acquire);
            if pid != 0 && editor_window_visible(pid) {
                // SAFETY: This thread owns the live splash window.
                unsafe { DestroyWindow(window) };
                return;
            }
        }
        thread::sleep(Duration::from_millis(30));
    }
}

fn editor_window_visible(pid: u32) -> bool {
    // SAFETY: Callback only inspects top-level windows and does not retain handles.
    unsafe { EnumWindows(Some(find_editor_window), pid as LPARAM) == 0 }
}

unsafe extern "system" fn find_editor_window(window: HWND, target: LPARAM) -> i32 {
    let mut pid = 0;
    // SAFETY: The enumerated window is live for this callback; output is initialized.
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    // SAFETY: The enumerated top-level window is valid during this callback.
    if pid == target as u32 && unsafe { IsWindowVisible(window) } != 0 {
        0
    } else {
        1
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            // SAFETY: BeginPaint/EndPaint bracket drawing to this live window.
            let dc = unsafe { BeginPaint(window, &mut paint) };
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: WIDTH,
                    biHeight: -HEIGHT,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..Default::default()
                },
                ..Default::default()
            };
            PIXELS.with(|slot| {
                if let Some(pixels) = slot.borrow().as_ref() {
                    // SAFETY: Pixel buffer and bitmap description stay live for this synchronous draw.
                    unsafe {
                        StretchDIBits(
                            dc,
                            0,
                            0,
                            WIDTH,
                            HEIGHT,
                            0,
                            0,
                            WIDTH,
                            HEIGHT,
                            pixels.0.as_ptr().cast(),
                            &info,
                            DIB_RGB_COLORS,
                            SRCCOPY,
                        );
                    }
                }
            });
            FAILED.with(|slot| {
                if *slot.borrow() {
                    let mut client = RECT::default();
                    let message: Vec<u16> = "Achimeld could not start. Click to close.\0"
                        .encode_utf16()
                        .collect();
                    // SAFETY: The paint DC, client rectangle, and text buffer remain live for these synchronous draws.
                    unsafe {
                        GetClientRect(window, &mut client);
                        PatBlt(
                            dc,
                            0,
                            HEIGHT - FAILURE_HEIGHT,
                            WIDTH,
                            FAILURE_HEIGHT,
                            BLACKNESS,
                        );
                        SetBkMode(dc, TRANSPARENT as i32);
                        SetTextColor(dc, 0x00FF_FFFF);
                        client.top = HEIGHT - FAILURE_HEIGHT;
                        DrawTextW(
                            dc,
                            message.as_ptr(),
                            -1,
                            &mut client,
                            DT_CENTER | DT_VCENTER | DT_WORDBREAK,
                        );
                    }
                }
            });
            // SAFETY: Completes the BeginPaint call for the same live window and paint storage.
            unsafe { EndPaint(window, &paint) };
            0
        }
        WM_LBUTTONUP => {
            if FAILED.with(|slot| *slot.borrow()) {
                // SAFETY: The callback runs on the window's owning thread.
                unsafe { DestroyWindow(window) };
            }
            0
        }
        WM_CLOSE => {
            // SAFETY: The callback runs on the window's owning thread.
            unsafe { DestroyWindow(window) };
            0
        }
        WM_DESTROY => {
            // SAFETY: Posts quit only to this UI thread's message queue.
            unsafe { PostQuitMessage(0) };
            0
        }
        // SAFETY: Unhandled messages are passed to Win32 with their original values.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
