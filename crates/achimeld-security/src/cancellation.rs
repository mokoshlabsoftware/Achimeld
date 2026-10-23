use crate::SecurityError;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn check(&self) -> Result<(), SecurityError> {
        if self.0.load(Ordering::Acquire) {
            Err(SecurityError("Steam read cancelled"))
        } else {
            Ok(())
        }
    }
}

static CONSOLE_CANCELLED: AtomicBool = AtomicBool::new(false);
pub fn install_console_handler() -> Result<(), SecurityError> {
    #[cfg(windows)]
    {
        // Detached supervised processes have no console. Their cancellation token
        // remains active; terminating the bootstrapper closes the outer Job.
        // SAFETY: GetConsoleCP queries process console state without output pointers or ownership transfer.
        if unsafe { windows_sys::Win32::System::Console::GetConsoleCP() } == 0 {
            return Ok(());
        }
        unsafe extern "system" fn handler(event: u32) -> i32 {
            if event <= 1 {
                CONSOLE_CANCELLED.store(true, Ordering::Release);
                1
            } else {
                0
            }
        }
        // SAFETY: static handler does no allocation and remains valid for process lifetime.
        // SAFETY: Handler has the required Windows ABI and static lifetime; it performs only an atomic store.
        if unsafe { windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(handler), 1) }
            == 0
        {
            return Err(SecurityError("Console cancellation unavailable"));
        }
    }
    Ok(())
}
pub fn check_console() -> Result<(), SecurityError> {
    if CONSOLE_CANCELLED.load(Ordering::Acquire) {
        Err(SecurityError("Steam read cancelled"))
    } else {
        Ok(())
    }
}
