use std::sync::{Mutex, OnceLock};

const FORCE_EP_ENV: &str = "SMOLPC_FORCE_EP";
const DML_DEVICE_ENV: &str = "SMOLPC_DML_DEVICE_ID";

#[allow(unused_unsafe)]
fn set_env_var(name: &str, value: &str) {
    unsafe {
        std::env::set_var(name, value);
    }
}

#[allow(unused_unsafe)]
fn remove_env_var(name: &str) {
    unsafe {
        std::env::remove_var(name);
    }
}

fn env_lock() -> &'static Mutex<()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

struct RuntimeEnvGuard {
    previous_force_ep: Option<String>,
    previous_dml_device: Option<String>,
}

impl RuntimeEnvGuard {
    fn capture() -> Self {
        Self {
            previous_force_ep: std::env::var(FORCE_EP_ENV).ok(),
            previous_dml_device: std::env::var(DML_DEVICE_ENV).ok(),
        }
    }
}

impl Drop for RuntimeEnvGuard {
    fn drop(&mut self) {
        match self.previous_force_ep.as_deref() {
            Some(value) => set_env_var(FORCE_EP_ENV, value),
            None => remove_env_var(FORCE_EP_ENV),
        }
        match self.previous_dml_device.as_deref() {
            Some(value) => set_env_var(DML_DEVICE_ENV, value),
            None => remove_env_var(DML_DEVICE_ENV),
        }
    }
}

pub(super) fn with_runtime_env(
    force_ep: Option<&str>,
    dml_device_id: Option<&str>,
    test: impl FnOnce(),
) {
    let _lock = env_lock().lock().expect("env lock");
    let _guard = RuntimeEnvGuard::capture();

    match force_ep {
        Some(value) => set_env_var(FORCE_EP_ENV, value),
        None => remove_env_var(FORCE_EP_ENV),
    }
    match dml_device_id {
        Some(value) => set_env_var(DML_DEVICE_ENV, value),
        None => remove_env_var(DML_DEVICE_ENV),
    }

    test();
}
