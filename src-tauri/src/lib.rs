mod free_games;
mod state;
mod steam;

use chrono::{Duration as ChronoDuration, Utc};
use state::{Settings, Snapshot, Store};
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::Notify;

pub(crate) struct AppState {
    pub(crate) store: Mutex<Store>,
    /// Wakes the background poller (check now / settings changed).
    wake: Arc<Notify>,
    checking: Mutex<bool>,
    /// Pending Steam add result (auto-add waits on it).
    pub(crate) steam_waiter: Mutex<Option<tokio::sync::oneshot::Sender<String>>>,
    /// One Steam auto-add queue at a time.
    pub(crate) steam_busy: Mutex<bool>,
}

/// Hosts the in-app sign-in/claim windows may open. Anything else goes to the system browser.
const STORE_HOSTS: &[&str] = &[
    "epicgames.com",
    "gog.com",
    "amazon.com",
    "amazon.de",
    "amazon.co.uk",
    "amazon.fr",
    "amazon.es",
    "amazon.it",
    "amazon.ca",
    "steampowered.com",
];

fn host_allowed(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && url.host_str().is_some_and(|h| {
            STORE_HOSTS.iter().any(|a| h == *a || h.ends_with(&format!(".{a}")))
        })
}

/// Pseudo-random jitter in seconds within [-max, +max], so checks don't happen on a fixed clock.
pub(crate) fn jitter_secs(max: i64) -> i64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as i64 ^ (d.as_secs() as i64).rotate_left(13))
        .unwrap_or(0);
    (nanos.rem_euclid(2 * max + 1)) - max
}

pub(crate) fn emit_snapshot(app: &AppHandle) {
    let snap = app.state::<AppState>().store.lock().unwrap().snapshot();
    let _ = app.emit("snapshot", snap);
}

async fn run_check(app: &AppHandle, reason: &str) -> Result<Snapshot, String> {
    let st = app.state::<AppState>();
    {
        let mut busy = st.checking.lock().unwrap();
        if *busy {
            return Err("A check is already running".into());
        }
        *busy = true;
    }
    let settings = st.store.lock().unwrap().data.settings.clone();
    let mut result = free_games::fetch_all(settings.epic, settings.gog, settings.prime, settings.steam).await;

    let new_items = {
        let mut store = st.store.lock().unwrap();
        // Keep last known GOG homepage giveaways until phase 2 refreshes them (avoids flicker).
        if settings.gog {
            let kept: Vec<_> = store.data.games.iter().filter(|g| g.id.starts_with(GOG_HOME_PREFIX)).cloned().collect();
            result.games.extend(kept);
        }
        for e in &result.errors {
            store.log("free-games", "error", e.clone());
        }
        let total = result.games.len();
        let new_items = store.apply_games(result.games);
        store.log("free-games", "info", format!("Check ({reason}): {total} offers, {} new", new_items.len()));
        for g in &new_items {
            store.log("free-games", "reward", format!("New free game: {} ({:?})", g.title, g.store));
        }
        store.save();
        new_items
    };
    *st.checking.lock().unwrap() = false;
    if settings.notify {
        notify_new(app, &new_items);
    }
    if settings.steam && settings.steam_auto_add {
        let steam_new: Vec<_> = new_items.iter().filter(|g| g.store == free_games::Store::Steam).cloned().collect();
        steam::spawn_auto_add(app.clone(), steam_new);
    }

    // Phase 2 (slow, ~10 s): GOG homepage giveaway banner, merged in when it arrives.
    if settings.gog {
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            let res = free_games::fetch_gog_homepage().await;
            let st = app2.state::<AppState>();
            let new_items = {
                let mut store = st.store.lock().unwrap();
                match res {
                    Ok(found) => {
                        let mut games: Vec<_> = store.data.games.iter().filter(|g| !g.id.starts_with(GOG_HOME_PREFIX)).cloned().collect();
                        games.extend(found.into_iter().filter(|f| !store.data.games.iter().any(|g| g.url == f.url && !g.id.starts_with(GOG_HOME_PREFIX))));
                        let new_items = store.merge_games(games);
                        for g in &new_items {
                            store.log("free-games", "reward", format!("New free game: {} (GOG giveaway)", g.title));
                        }
                        store.save();
                        new_items
                    }
                    Err(e) => {
                        store.log("free-games", "error", format!("GOG homepage: {e}"));
                        vec![]
                    }
                }
            };
            if settings.notify {
                notify_new(&app2, &new_items);
            }
            emit_snapshot(&app2);
        });
    }
    emit_snapshot(app);
    Ok(st.store.lock().unwrap().snapshot())
}

const GOG_HOME_PREFIX: &str = "gog-giveaway:";

fn notify_new(app: &AppHandle, items: &[free_games::FreeGame]) {
    if items.is_empty() {
        return;
    }
    let body = if items.len() == 1 {
        items[0].title.clone()
    } else {
        format!("{} and {} more", items[0].title, items.len() - 1)
    };
    let _ = app.notification().builder().title("Free games available").body(body).show();
}

/// Background poller: checks every N hours (+/- 20 min random jitter).
fn spawn_poller(app: AppHandle, wake: Arc<Notify>) {
    tauri::async_runtime::spawn(async move {
        // Small randomized delay after startup instead of hammering immediately.
        tokio::time::sleep(std::time::Duration::from_secs((5 + jitter_secs(3).abs()) as u64)).await;
        loop {
            let _ = run_check(&app, "scheduled").await;
            let hours = app.state::<AppState>().store.lock().unwrap().data.settings.check_interval_hours.max(1) as i64;
            let wait = hours * 3600 + jitter_secs(20 * 60);
            {
                let st = app.state::<AppState>();
                let mut store = st.store.lock().unwrap();
                store.data.next_check = Some(Utc::now() + ChronoDuration::seconds(wait));
                store.save();
            }
            emit_snapshot(&app);
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(wait as u64)) => {}
                _ = wake.notified() => {}
            }
        }
    });
}

// ------------------------------------------------------------------ commands

#[tauri::command]
fn get_snapshot(state: tauri::State<AppState>) -> Snapshot {
    state.store.lock().unwrap().snapshot()
}

#[tauri::command]
async fn check_now(app: AppHandle) -> Result<Snapshot, String> {
    run_check(&app, "manual").await
}

#[tauri::command]
fn set_claimed(app: AppHandle, state: tauri::State<AppState>, id: String, claimed: bool) -> Snapshot {
    let snap = {
        let mut store = state.store.lock().unwrap();
        let title = store.data.games.iter().find(|g| g.id == id).map(|g| g.title.clone()).unwrap_or(id.clone());
        if claimed {
            store.data.claimed.insert(id);
            store.log("free-games", "reward", format!("Marked as claimed: {title}"));
        } else {
            store.data.claimed.remove(&id);
            store.log("free-games", "info", format!("Unmarked: {title}"));
        }
        store.save();
        store.snapshot()
    };
    let _ = app.emit("snapshot", &snap);
    snap
}

#[tauri::command]
fn update_settings(app: AppHandle, state: tauri::State<AppState>, settings: Settings) -> Snapshot {
    let autolaunch = app.autolaunch();
    if autolaunch.is_enabled().unwrap_or(false) != settings.launch_at_startup {
        let _ = if settings.launch_at_startup { autolaunch.enable() } else { autolaunch.disable() };
    }
    let mut store = state.store.lock().unwrap();
    let interval_changed = store.data.settings.check_interval_hours != settings.check_interval_hours;
    let mut s = settings;
    s.check_interval_hours = s.check_interval_hours.clamp(1, 48);
    store.data.settings = s;
    store.save();
    if interval_changed {
        state.wake.notify_one();
    }
    store.snapshot()
}

/// Opens the store's official page in an in-app window. The user signs in there
/// directly; the window has no IPC access and this app never reads its cookies.
#[tauri::command]
async fn open_store_window(app: AppHandle, url: String, title: Option<String>) -> Result<(), String> {
    let parsed: tauri::Url = url.parse().map_err(|_| "Invalid URL".to_string())?;
    if !host_allowed(&parsed) {
        return Err("This site is not on the store allow-list".into());
    }
    let host = parsed.host_str().unwrap_or("store");
    let key = STORE_HOSTS.iter().find(|a| host == **a || host.ends_with(&format!(".{a}"))).unwrap_or(&"store");
    let label = format!("store-{}", key.replace('.', "-"));
    let window_title = title.clone().unwrap_or_else(|| format!("Free Stuff Hub - {key}"));
    if let Some(w) = app.get_webview_window(&label) {
        w.navigate(parsed).map_err(|e| e.to_string())?;
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    if steam::is_steam_host(&parsed) {
        steam::build_window(&app, &label, parsed, true, window_title)?;
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(parsed))
        .title(window_title)
        .inner_size(1180.0, 820.0)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_external(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let parsed: tauri::Url = url.parse().map_err(|_| "Invalid URL".to_string())?;
    if parsed.scheme() != "https" {
        return Err("Only https links can be opened".into());
    }
    app.opener().open_url(parsed.as_str(), None::<&str>).map_err(|e| e.to_string())
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let wake = Arc::new(Notify::new());
            app.manage(AppState {
                store: Mutex::new(Store::load(dir)),
                wake: wake.clone(),
                checking: Mutex::new(false),
                steam_waiter: Mutex::new(None),
                steam_busy: Mutex::new(false),
            });

            // Tray
            let open = MenuItem::with_id(app, "open", "Open dashboard", true, None::<&str>)?;
            let check = MenuItem::with_id(app, "check", "Check free games now", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &check, &quit])?;
            TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().cloned().expect("icon"))
                .tooltip("Free Stuff Hub")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "open" => show_main(app),
                    "check" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = run_check(&app, "tray").await;
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            // Started by Windows at login: stay in the tray.
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            // Keep the saved setting in sync with the real registry entry.
            {
                let enabled = app.autolaunch().is_enabled().unwrap_or(false);
                let st = app.state::<AppState>();
                let mut store = st.store.lock().unwrap();
                store.data.settings.launch_at_startup = enabled;
            }

            spawn_poller(app.handle().clone(), wake);
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    let to_tray = window.app_handle().state::<AppState>().store.lock().unwrap().data.settings.close_to_tray;
                    if to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            check_now,
            set_claimed,
            update_settings,
            open_store_window,
            open_external,
            steam::steam_add_to_account
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
