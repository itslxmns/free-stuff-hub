//! Auto-update from GitHub Releases (Tauri updater plugin).
//!
//! - Release builds publish `latest.json` plus a signed installer. The app only installs
//!   updates signed with the project's key (public key in tauri.conf.json).
//! - Checks ~20 s after start, then about every 6 h.
//! - With "Install updates automatically" on, it installs only when that interrupts nothing:
//!   the window is hidden in the tray and card idling isn't running. Otherwise it shows a banner.

use crate::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    pub current: String,
    /// "idle" | "checking" | "available" | "downloading" | "installing" | "error" | "latest"
    pub status: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub progress: Option<f32>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Updater {
    view: UpdateView,
    pending: Option<Update>,
    notified: Option<String>,
}

fn emit(app: &AppHandle) {
    let v = app.state::<AppState>().updater.lock().unwrap().view.clone();
    let _ = app.emit("update", v);
}

fn set(app: &AppHandle, f: impl FnOnce(&mut UpdateView)) {
    {
        let st = app.state::<AppState>();
        let mut u = st.updater.lock().unwrap();
        f(&mut u.view);
    }
    emit(app);
}

/// Check GitHub for a newer signed release. Returns the new version, if any.
pub async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    set(app, |v| {
        v.status = "checking".into();
        v.error = None;
    });
    let result = async {
        let updater = app.updater().map_err(|e| e.to_string())?;
        updater.check().await.map_err(|e| e.to_string())
    }
    .await;
    match result {
        Ok(Some(update)) => {
            let version = update.version.clone();
            let notes = update.body.clone();
            let first_time = {
                let st = app.state::<AppState>();
                let mut u = st.updater.lock().unwrap();
                u.view.status = "available".into();
                u.view.version = Some(version.clone());
                u.view.notes = notes;
                u.pending = Some(update);
                let first = u.notified.as_deref() != Some(version.as_str());
                u.notified = Some(version.clone());
                first
            };
            emit(app);
            if first_time {
                let _ = app
                    .notification()
                    .builder()
                    .title("Free Stuff Hub update")
                    .body(format!("Version {version} is available"))
                    .show();
            }
            Ok(Some(version))
        }
        Ok(None) => {
            set(app, |v| {
                v.status = "latest".into();
                v.version = None;
            });
            Ok(None)
        }
        Err(e) => {
            set(app, |v| {
                v.status = "error".into();
                v.error = Some(e.clone());
            });
            Err(e)
        }
    }
}

/// Download, verify the signature, run the installer and restart.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let update = app.state::<AppState>().updater.lock().unwrap().pending.take();
    let Some(update) = update else {
        return Err("No update to install".into());
    };
    set(app, |v| {
        v.status = "downloading".into();
        v.progress = Some(0.0);
    });
    let handle = app.clone();
    let mut got: u64 = 0;
    let res = update
        .download_and_install(
            move |chunk, total| {
                got += chunk as u64;
                if let Some(t) = total.filter(|t| *t > 0) {
                    let p = (got as f32 / t as f32).min(1.0);
                    set(&handle, |v| v.progress = Some(p));
                }
            },
            || {},
        )
        .await;
    match res {
        Ok(()) => {
            set(app, |v| v.status = "installing".into());
            crate::idler::shutdown(app);
            app.restart();
        }
        Err(e) => {
            let msg = e.to_string();
            set(app, |v| {
                v.status = "error".into();
                v.error = Some(msg.clone());
                v.progress = None;
            });
            Err(msg)
        }
    }
}

fn quiet_moment(app: &AppHandle) -> bool {
    let hidden = app.get_webview_window("main").map(|w| !w.is_visible().unwrap_or(true)).unwrap_or(true);
    let idling = app.state::<AppState>().idler.lock().unwrap().running;
    hidden && !idling
}

pub fn spawn_checker(app: AppHandle) {
    set(&app, |v| {
        v.current = app.package_info().version.to_string();
        v.status = "idle".into();
    });
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        loop {
            if let Ok(Some(_)) = check(&app).await {
                let auto = app.state::<AppState>().store.lock().unwrap().data.settings.auto_update;
                if auto && quiet_moment(&app) {
                    let _ = install(&app).await;
                }
            }
            let wait = 6 * 3600 + crate::jitter_secs(30 * 60);
            tokio::time::sleep(std::time::Duration::from_secs(wait as u64)).await;
        }
    });
}

// ------------------------------------------------------------------ commands

#[tauri::command]
pub fn update_state(state: tauri::State<AppState>) -> UpdateView {
    state.updater.lock().unwrap().view.clone()
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<UpdateView, String> {
    check(&app).await?;
    Ok(app.state::<AppState>().updater.lock().unwrap().view.clone())
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    install(&app).await
}
