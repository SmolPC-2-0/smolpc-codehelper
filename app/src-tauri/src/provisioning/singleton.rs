//! Singleton guard to prevent concurrent provisioning runs.
//!
//! On Windows a named mutex (`Global\SmolPC-Provisioning`) is acquired for the
//! lifetime of the guard. On other platforms the guard is a no-op.

#[cfg(windows)]
mod platform {
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::Threading::CreateMutexW;

    pub struct SingletonGuard {
        handle: HANDLE,
    }

    impl SingletonGuard {
        pub fn acquire() -> Result<Self, String> {
            // Safety: CreateMutexW is safe to call with these arguments.
            let handle = unsafe { CreateMutexW(None, true, w!("Global\\SmolPC-Provisioning")) }
                .map_err(|e| format!("CreateMutexW failed: {e}"))?;

            // GetLastError returns ERROR_ALREADY_EXISTS when the mutex already
            // existed — meaning another instance holds it.
            if unsafe { windows::Win32::Foundation::GetLastError() } == ERROR_ALREADY_EXISTS {
                // Release the handle we just opened before returning.
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(
                    "Another SmolPC instance is already setting up models".to_string(),
                );
            }

            Ok(Self { handle })
        }
    }

    impl Drop for SingletonGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.handle);
            }
        }
    }

    // Safety: Windows HANDLE is a process-wide kernel object identifier,
    // safe to use from any thread. CloseHandle is thread-safe.
    unsafe impl Send for SingletonGuard {}
}

#[cfg(not(windows))]
mod platform {
    pub struct SingletonGuard;

    impl SingletonGuard {
        pub fn acquire() -> Result<Self, String> {
            Ok(Self)
        }
    }
}

pub use platform::SingletonGuard;
