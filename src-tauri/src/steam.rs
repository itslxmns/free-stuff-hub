//! Steam "Add to account" for 100%-off games.
//!
//! How it stays safe:
//! - Runs inside the official store.steampowered.com page in an app window, in the user's own
//!   signed-in session. The app never sees the password, cookies or Steam Guard codes.
//! - The helper script only acts on a page we opened with the `#fsh-add` marker and clicks Steam's
//!   own "Add to Account" button, after a random human-like delay.
//! - Reports back by navigating to the real "Licenses" page with `?fsh=<result>`, which the Rust
//!   side sees in `on_navigation`. The window has no IPC permissions (see capabilities).
//! - Auto-add runs one game at a time with random gaps and a daily cap.

use crate::free_games::{FreeGame, Store as StoreKind};
use crate::AppState;
use chrono::{Duration as ChronoDuration, Utc};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const VISIBLE_LABEL: &str = "store-steampowered-com";
const AUTO_LABEL: &str = "steam-auto";

/// Injected into Steam store windows. Does nothing unless the page was opened with `#fsh-add`
/// (or is the same app page we returned to after sign-in).
const ADD_SCRIPT: &str = r#"
(function () {
  if (location.hostname !== 'store.steampowered.com') return;
  var KEY = 'fsh-add';
  var m = location.pathname.match(/^\/(app|sub|bundle)\/(\d+)/);
  if (!m) return;
  // Marker survives the sign-in round trip for 10 minutes, then expires.
  var saved = (sessionStorage.getItem(KEY) || '').split('|');
  var pending = saved[0] === m[2] && Date.now() - Number(saved[1] || 0) < 600000;
  if (location.hash !== '#fsh-add' && !pending) return;
  if (location.hash === '#fsh-add') sessionStorage.setItem(KEY, m[2] + '|' + Date.now());
  function report(r) {
    sessionStorage.removeItem(KEY);
    location.replace('https://store.steampowered.com/account/licenses/?fsh=' + r + '&id=' + m[2]);
  }
  function run() {
    var signedIn = !!document.querySelector('#account_pulldown, .user_avatar, #global_actions .playerAvatar');
    if (!signedIn) {
      location.replace('https://store.steampowered.com/login/?redir=' + encodeURIComponent(location.pathname.slice(1)));
      return;
    }
    if (document.querySelector('.game_area_already_owned')) { report('owned'); return; }
    var candidates = Array.prototype.slice.call(document.querySelectorAll(
      '.game_area_purchase_game a, .game_area_purchase_game span[onclick], .game_area_purchase_game button, ' +
      'a[href*="AddFreeLicense"], [onclick*="AddFreeLicense"], form[action*="addfreelicense"] [type=submit]'));
    var btn = candidates.find(function (el) {
      var t = (el.textContent || el.value || '').trim().toLowerCase();
      var h = (el.getAttribute('href') || '') + (el.getAttribute('onclick') || '');
      return t === 'add to account' || t === 'add to library' || /AddFreeLicense|addfreelicense/i.test(h);
    });
    if (!btn) {
      var form = document.querySelector('form[action*="addfreelicense"]');
      if (!form) { report('nobutton'); return; }
      btn = { click: function () { form.submit(); } };
    }
    var delay = 1500 + Math.floor(Math.random() * 3500);
    setTimeout(function () {
      btn.click();
      setTimeout(function () { report('added'); }, 3500 + Math.floor(Math.random() * 1500));
    }, delay);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run); else run();
})();
"#;

pub fn is_steam_host(url: &tauri::Url) -> bool {
    url.host_str().is_some_and(|h| h == "steampowered.com" || h.ends_with(".steampowered.com"))
}

/// Build a Steam store window with the helper script and the result watcher.
pub fn build_window(app: &AppHandle, label: &str, url: tauri::Url, visible: bool, title: String) -> Result<WebviewWindow, String> {
    let handle = app.clone();
    WebviewWindowBuilder::new(app, label, WebviewUrl::External(url))
        .title(title)
        .inner_size(1180.0, 820.0)
        .visible(visible)
        .initialization_script(ADD_SCRIPT)
        .on_navigation(move |u| {
            on_nav(&handle, u);
            true
        })
        .build()
        .map_err(|e| e.to_string())
}

fn on_nav(app: &AppHandle, url: &tauri::Url) {
    if !is_steam_host(url) {
        return;
    }
    let q = |k: &str| url.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string());
    let result = if url.path().starts_with("/account/licenses") {
        match q("fsh") {
            Some(r) => r,
            None => return,
        }
    } else if url.path().starts_with("/login") {
        "login".to_string()
    } else {
        return;
    };
    let id = q("id").unwrap_or_default();
    let st = app.state::<AppState>();
    if result == "added" || result == "owned" {
        let mut store = st.store.lock().unwrap();
        let hit = store.data.games.iter().find(|g| {
            g.store == StoreKind::Steam && !id.is_empty()
                && (g.url.contains(&format!("/app/{id}/")) || g.url.contains(&format!("/sub/{id}/")) || g.url.contains(&format!("/bundle/{id}/")))
        }).cloned();
        if let Some(g) = hit {
            store.data.claimed.insert(g.id.clone());
            let msg = if result == "added" { format!("Added to Steam account: {}", g.title) } else { format!("Already in your Steam library: {}", g.title) };
            store.log("steam", "reward", msg);
            if result == "added" {
                store.data.steam_adds.push(Utc::now());
            }
            store.save();
        }
    } else if result == "nobutton" {
        let mut store = st.store.lock().unwrap();
        store.log("steam", "error", format!("No 'Add to Account' button on Steam page {id} (promo may have ended or need the Steam client)"));
        store.save();
    }
    if let Some(tx) = st.steam_waiter.lock().unwrap().take() {
        let _ = tx.send(result);
    }
    crate::emit_snapshot(app);
}

fn flagged(url: &str) -> Result<tauri::Url, String> {
    let mut u: tauri::Url = url.parse().map_err(|_| "Invalid URL".to_string())?;
    if !is_steam_host(&u) {
        return Err("Not a Steam store URL".into());
    }
    // English UI so the button label is predictable (the addfreelicense form is the fallback).
    u.query_pairs_mut().append_pair("l", "english");
    u.set_fragment(Some("fsh-add"));
    Ok(u)
}

/// Manual "Add to account": visible window, the user can watch and sign in if needed.
#[tauri::command]
pub async fn steam_add_to_account(app: AppHandle, id: String) -> Result<(), String> {
    let game = {
        let st = app.state::<AppState>();
        let store = st.store.lock().unwrap();
        store.data.games.iter().find(|g| g.id == id && g.store == StoreKind::Steam).cloned()
    }
    .ok_or("Unknown Steam game")?;
    let url = flagged(&game.url)?;
    if let Some(w) = app.get_webview_window(VISIBLE_LABEL) {
        let _ = w.close();
    }
    build_window(&app, VISIBLE_LABEL, url, true, format!("Steam - adding {}", game.title))?;
    Ok(())
}

/// Auto-add newly found Steam games in a hidden window: one at a time, random gaps, daily cap.
pub fn spawn_auto_add(app: AppHandle, games: Vec<FreeGame>) {
    if games.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        {
            let st = app.state::<AppState>();
            if *st.steam_busy.lock().unwrap() {
                return;
            }
            *st.steam_busy.lock().unwrap() = true;
        }
        for g in games {
            let (cap, used) = {
                let st = app.state::<AppState>();
                let store = st.store.lock().unwrap();
                let since = Utc::now() - ChronoDuration::hours(24);
                (store.data.settings.steam_daily_cap, store.data.steam_adds.iter().filter(|t| **t > since).count() as u32)
            };
            if used >= cap {
                log(&app, "info", format!("Steam daily cap reached ({cap}); remaining games wait for the next check"));
                break;
            }
            // Random human-like gap between games.
            tokio::time::sleep(std::time::Duration::from_secs(8 + crate::jitter_secs(6).unsigned_abs() + crate::jitter_secs(10).unsigned_abs())).await;

            let Ok(url) = flagged(&g.url) else { continue };
            let (tx, rx) = tokio::sync::oneshot::channel();
            *app.state::<AppState>().steam_waiter.lock().unwrap() = Some(tx);
            if let Some(w) = app.get_webview_window(AUTO_LABEL) {
                let _ = w.close();
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            if let Err(e) = build_window(&app, AUTO_LABEL, url, false, "Steam auto-add".into()) {
                log(&app, "error", format!("Steam auto-add window failed: {e}"));
                break;
            }
            let result = tokio::time::timeout(std::time::Duration::from_secs(60), rx).await;
            if let Some(w) = app.get_webview_window(AUTO_LABEL) {
                let _ = w.close();
            }
            match result {
                Ok(Ok(r)) if r == "login" => {
                    log(&app, "error", "Steam auto-add needs you to sign in once: use 'Add to account' on any Steam game".to_string());
                    break;
                }
                Ok(Ok(_)) => {}
                _ => {
                    *app.state::<AppState>().steam_waiter.lock().unwrap() = None;
                    log(&app, "error", format!("Steam auto-add timed out for {}", g.title));
                }
            }
        }
        *app.state::<AppState>().steam_busy.lock().unwrap() = false;
        crate::emit_snapshot(&app);
    });
}

fn log(app: &AppHandle, level: &str, msg: String) {
    let st = app.state::<AppState>();
    let mut store = st.store.lock().unwrap();
    store.log("steam", level, msg);
    store.save();
    drop(store);
    let _ = app.emit("snapshot", st.store.lock().unwrap().snapshot());
}
