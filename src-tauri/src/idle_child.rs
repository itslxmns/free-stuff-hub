//! Child-process mode: `free-stuff-hub.exe --idle <appid> <path-to-steam_api64.dll>`
//!
//! Tells the running Steam client that game <appid> is open, using Steam's own
//! steam_api64.dll (taken from a game the user already has installed). No game code runs.
//! The process exits when the parent closes its stdin (stop button, app quit or crash).

use std::ffi::c_char;
use std::io::Read;

pub fn run(appid: &str, dll: &str) -> i32 {
    if appid.is_empty() || !appid.chars().all(|c| c.is_ascii_digit()) {
        return 3;
    }
    // Steam reads the app id from these before init (same as steam_appid.txt).
    unsafe {
        std::env::set_var("SteamAppId", appid);
        std::env::set_var("SteamGameId", appid);
    }

    let lib = match unsafe { libloading::Library::new(dll) } {
        Ok(l) => l,
        Err(_) => return 4,
    };

    let ok = unsafe {
        if let Ok(f) = lib.get::<unsafe extern "C" fn() -> bool>(b"SteamAPI_InitSafe\0") {
            f()
        } else if let Ok(f) = lib.get::<unsafe extern "C" fn() -> bool>(b"SteamAPI_Init\0") {
            f()
        } else if let Ok(f) = lib.get::<unsafe extern "C" fn(*mut c_char) -> i32>(b"SteamAPI_InitFlat\0") {
            let mut err = [0 as c_char; 1024];
            f(err.as_mut_ptr()) == 0
        } else {
            false
        }
    };
    if !ok {
        return 2; // Steam not running, not signed in, or the account doesn't own the game.
    }

    // Keep Steam's callback pump alive while waiting for the parent to close stdin.
    let run_callbacks = unsafe { lib.get::<unsafe extern "C" fn()>(b"SteamAPI_RunCallbacks\0").ok().map(|f| *f) };
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 64];
        let mut stdin = std::io::stdin();
        while let Ok(n) = stdin.read(&mut buf) {
            if n == 0 {
                break;
            }
        }
        let _ = tx.send(());
    });
    loop {
        if let Some(f) = run_callbacks {
            unsafe { f() };
        }
        if rx.recv_timeout(std::time::Duration::from_secs(1)).is_ok() {
            break;
        }
    }
    if let Ok(f) = unsafe { lib.get::<unsafe extern "C" fn()>(b"SteamAPI_Shutdown\0") } {
        unsafe { f() };
    }
    0
}
