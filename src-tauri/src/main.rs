use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use toml_edit::{value, DocumentMut, Item, Table};

const EXTERNAL_PROVIDER_ID: &str = "ExternalAPI";
const EXTERNAL_URL: &str = "https://code.adinsolution.link";
const EXTERNAL_MODEL: &str = "gpt-5.5";
const KEYCHAIN_SERVICE: &str = "CodexSwitchLite.ExternalAPI";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Plus,
    External,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedState {
    mode: Mode,
}

#[derive(Debug, Clone, Serialize)]
struct Status {
    initialized: bool,
    mode: Mode,
    live_provider: Option<String>,
    auth_exists: bool,
    key_saved: bool,
    routing_ok: bool,
    config_path: String,
    last_backup: Option<String>,
    warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ActionResult {
    ok: bool,
    message: String,
}

fn home_dir() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "无法确定用户 Home 目录".to_string())
}

fn codex_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".codex"))
}

fn live_config_path() -> Result<PathBuf, String> {
    Ok(codex_dir()?.join("config.toml"))
}

fn auth_path() -> Result<PathBuf, String> {
    Ok(codex_dir()?.join("auth.json"))
}

fn app_dir() -> Result<PathBuf, String> {
    Ok(codex_dir()?.join("codex-switch-lite"))
}

fn plus_profile_path() -> Result<PathBuf, String> {
    Ok(app_dir()?.join("plus.toml"))
}

fn external_profile_path() -> Result<PathBuf, String> {
    Ok(app_dir()?.join("external.toml"))
}

fn state_path() -> Result<PathBuf, String> {
    Ok(app_dir()?.join("state.json"))
}

fn backups_dir() -> Result<PathBuf, String> {
    Ok(app_dir()?.join("backups"))
}

fn ensure_dirs() -> Result<(), String> {
    fs::create_dir_all(codex_dir()?).map_err(|e| format!("创建 ~/.codex 失败: {e}"))?;
    fs::create_dir_all(app_dir()?).map_err(|e| format!("创建 Lite 配置目录失败: {e}"))?;
    fs::create_dir_all(backups_dir()?).map_err(|e| format!("创建备份目录失败: {e}"))?;
    Ok(())
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))
}

fn validate_toml(text: &str) -> Result<(), String> {
    text.parse::<DocumentMut>()
        .map(|_| ())
        .map_err(|e| format!("TOML 校验失败: {e}"))
}

fn atomic_write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    validate_toml(text)?;
    let tmp = path.with_extension("toml.tmp");
    {
        let mut file = fs::File::create(&tmp).map_err(|e| format!("创建临时文件失败: {e}"))?;
        file.write_all(text.as_bytes())
            .map_err(|e| format!("写临时文件失败: {e}"))?;
        file.sync_all().map_err(|e| format!("同步临时文件失败: {e}"))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("原子替换 {} 失败: {e}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn read_live_or_empty() -> Result<String, String> {
    let path = live_config_path()?;
    if path.exists() {
        read_text(&path)
    } else {
        Ok(String::new())
    }
}

fn state_read() -> Option<PersistedState> {
    let path = state_path().ok()?;
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn state_write(mode: Mode) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&PersistedState { mode })
        .map_err(|e| format!("序列化状态失败: {e}"))?;
    fs::write(state_path()?, text).map_err(|e| format!("写状态失败: {e}"))
}

fn model_provider(text: &str) -> Option<String> {
    let parsed: toml::Value = toml::from_str(text).ok()?;
    parsed
        .get("model_provider")
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
}

fn provider_base_url(text: &str, provider_id: &str) -> Option<String> {
    let parsed: toml::Value = toml::from_str(text).ok()?;
    parsed
        .get("model_providers")?
        .get(provider_id)?
        .get("base_url")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn provider_bearer_token(text: &str, provider_id: &str) -> Option<String> {
    let parsed: toml::Value = toml::from_str(text).ok()?;
    parsed
        .get("model_providers")?
        .get(provider_id)?
        .get("experimental_bearer_token")?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn provider_requires_auth(text: &str, provider_id: &str) -> Option<bool> {
    let parsed: toml::Value = toml::from_str(text).ok()?;
    parsed
        .get("model_providers")?
        .get(provider_id)?
        .get("requires_openai_auth")?
        .as_bool()
}

fn find_external_bearer_token(text: &str) -> Option<String> {
    let parsed: toml::Value = toml::from_str(text).ok()?;
    let providers = parsed.get("model_providers")?.as_table()?;
    for (id, provider) in providers {
        let Some(table) = provider.as_table() else {
            continue;
        };
        let is_external = id == EXTERNAL_PROVIDER_ID
            || table
                .get("base_url")
                .and_then(toml::Value::as_str)
                == Some(EXTERNAL_URL);
        if !is_external {
            continue;
        }
        if let Some(token) = table
            .get("experimental_bearer_token")
            .and_then(toml::Value::as_str)
            .filter(|s| !s.trim().is_empty())
        {
            return Some(token.to_string());
        }
    }
    None
}

fn detect_mode(text: &str) -> Mode {
    let provider = model_provider(text);
    if provider.as_deref() == Some(EXTERNAL_PROVIDER_ID) {
        return Mode::External;
    }
    if let Some(id) = provider.as_deref() {
        if provider_base_url(text, id).as_deref() == Some(EXTERNAL_URL) {
            return Mode::External;
        }
    }
    if provider.as_deref() == Some("openai") || provider.is_none() {
        return Mode::Plus;
    }
    Mode::Unknown
}

fn is_external_provider_item(id: &str, item: &Item) -> bool {
    if id == EXTERNAL_PROVIDER_ID {
        return true;
    }
    let base_url = item
        .as_table()
        .and_then(|t| t.get("base_url"))
        .and_then(Item::as_value)
        .and_then(|v| v.as_str());
    base_url == Some(EXTERNAL_URL)
}

fn remove_external_provider_tables(doc: &mut DocumentMut) {
    if let Some(mp) = doc.get_mut("model_providers").and_then(Item::as_table_mut) {
        let keys: Vec<String> = mp
            .iter()
            .filter_map(|(k, item)| is_external_provider_item(k, item).then(|| k.to_string()))
            .collect();
        for key in keys {
            mp.remove(&key);
        }
        if mp.is_empty() {
            doc.as_table_mut().remove("model_providers");
        }
    }
}

fn remove_external_top_level_reroute(doc: &mut DocumentMut) {
    let should_remove = doc
        .get("openai_base_url")
        .and_then(Item::as_value)
        .and_then(|v| v.as_str())
        == Some(EXTERNAL_URL);
    if should_remove {
        doc.as_table_mut().remove("openai_base_url");
    }
    doc.as_table_mut().remove("experimental_bearer_token");
}

fn apply_plus(text: &str) -> Result<String, String> {
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|e| format!("Plus 配置解析失败: {e}"))?;
    remove_external_provider_tables(&mut doc);
    remove_external_top_level_reroute(&mut doc);
    doc["model_provider"] = value("openai");
    Ok(doc.to_string())
}

fn apply_external(text: &str, key: Option<&str>) -> Result<String, String> {
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|e| format!("External 配置解析失败: {e}"))?;
    remove_external_provider_tables(&mut doc);
    remove_external_top_level_reroute(&mut doc);

    doc["model_provider"] = value(EXTERNAL_PROVIDER_ID);
    doc["model"] = value(EXTERNAL_MODEL);
    doc["review_model"] = value(EXTERNAL_MODEL);

    if doc.get("model_providers").and_then(Item::as_table).is_none() {
        doc["model_providers"] = Item::Table(Table::new());
    }

    let mut provider = Table::new();
    provider["name"] = value("External API");
    provider["base_url"] = value(EXTERNAL_URL);
    provider["wire_api"] = value("responses");
    if let Some(key) = key {
        provider["experimental_bearer_token"] = value(key);
    }
    // Mirrors CC Switch's official-login preservation mode: keep Codex Desktop
    // logged into ChatGPT while the provider-scoped bearer token short-circuits
    // actual inference auth to the external endpoint.
    provider["requires_openai_auth"] = value(true);

    doc["model_providers"][EXTERNAL_PROVIDER_ID] = Item::Table(provider);
    Ok(doc.to_string())
}

fn strip_external_secret(text: &str) -> Result<String, String> {
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|e| format!("External profile 解析失败: {e}"))?;
    if let Some(table) = doc
        .get_mut("model_providers")
        .and_then(Item::as_table_mut)
        .and_then(|mp| mp.get_mut(EXTERNAL_PROVIDER_ID))
        .and_then(Item::as_table_mut)
    {
        table.remove("experimental_bearer_token");
    }
    Ok(doc.to_string())
}

fn sync_common(source_text: &str, target_text: &str) -> Result<String, String> {
    let source = source_text
        .parse::<DocumentMut>()
        .map_err(|e| format!("当前配置解析失败: {e}"))?;
    let mut target = target_text
        .parse::<DocumentMut>()
        .map_err(|e| format!("目标配置解析失败: {e}"))?;

    let owned: HashSet<&str> = ["model_provider", "model", "review_model", "model_providers"]
        .into_iter()
        .collect();

    let source_keys: HashSet<String> = source
        .as_table()
        .iter()
        .filter_map(|(k, _)| (!owned.contains(k)).then(|| k.to_string()))
        .collect();

    let target_keys: Vec<String> = target
        .as_table()
        .iter()
        .filter_map(|(k, _)| (!owned.contains(k)).then(|| k.to_string()))
        .collect();

    for key in target_keys {
        if !source_keys.contains(&key) {
            target.as_table_mut().remove(&key);
        }
    }

    for (key, item) in source.as_table().iter() {
        if !owned.contains(key) {
            target.as_table_mut().insert(key, item.clone());
        }
    }

    // Synchronize all unrelated provider definitions, but never leak the Lite
    // External provider across modes. The target mode re-adds its own provider.
    let mut common_mp: Option<Item> = source.get("model_providers").cloned();
    if let Some(item) = common_mp.as_mut() {
        if let Some(table) = item.as_table_mut() {
            let keys: Vec<String> = table
                .iter()
                .filter_map(|(k, item)| is_external_provider_item(k, item).then(|| k.to_string()))
                .collect();
            for key in keys {
                table.remove(&key);
            }
        }
    }

    match common_mp {
        Some(item) if item.as_table().map(|t| !t.is_empty()).unwrap_or(true) => {
            target.as_table_mut().insert("model_providers", item);
        }
        _ => {
            target.as_table_mut().remove("model_providers");
        }
    }

    Ok(target.to_string())
}

fn old_profile_candidates() -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let old = codex_dir()?.join("external-api-switcher");
    Ok((
        vec![old.join("config.plus.toml")],
        vec![old.join("config.api.toml"), old.join("config.external.toml")],
    ))
}

fn first_existing(paths: &[PathBuf]) -> Option<String> {
    paths.iter().find_map(|p| fs::read_to_string(p).ok())
}

fn backup_live(live_text: &str) -> Result<PathBuf, String> {
    ensure_dirs()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("系统时间错误: {e}"))?
        .as_secs();
    let path = backups_dir()?.join(format!("config-{stamp}.toml"));
    fs::write(&path, live_text).map_err(|e| format!("创建备份失败: {e}"))?;
    prune_backups(20)?;
    Ok(path)
}

fn list_backups() -> Result<Vec<PathBuf>, String> {
    let dir = backups_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("读取备份目录失败: {e}"))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("toml"))
        .collect();
    paths.sort();
    Ok(paths)
}

fn prune_backups(keep: usize) -> Result<(), String> {
    let paths = list_backups()?;
    if paths.len() > keep {
        for path in &paths[..paths.len() - keep] {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn latest_backup() -> Result<Option<PathBuf>, String> {
    Ok(list_backups()?.into_iter().last())
}

#[cfg(target_os = "macos")]
fn keychain_account() -> String {
    std::env::var("USER").unwrap_or_else(|_| "codex-user".to_string())
}

#[cfg(target_os = "macos")]
fn keychain_set(key: &str) -> Result<(), String> {
    let account = keychain_account();
    let output = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-a",
            account.as_str(),
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
            key,
        ])
        .output()
        .map_err(|e| format!("调用 macOS Keychain 失败: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn keychain_set(_key: &str) -> Result<(), String> {
    Err("Codex Switch Lite 当前只支持 macOS".to_string())
}

#[cfg(target_os = "macos")]
fn keychain_get() -> Option<String> {
    let account = keychain_account();
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-a",
            account.as_str(),
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(not(target_os = "macos"))]
fn keychain_get() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn restart_codex() -> Result<(), String> {
    let _ = Command::new("osascript")
        .args(["-e", "tell application \"Codex\" to quit"])
        .status();
    thread::sleep(Duration::from_millis(1200));
    Command::new("open")
        .args(["-a", "Codex"])
        .spawn()
        .map_err(|e| format!("重新打开 Codex 失败: {e}"))?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn restart_codex() -> Result<(), String> {
    Err("自动重启 Codex 当前只支持 macOS".to_string())
}

fn ensure_initialized() -> Result<(), String> {
    ensure_dirs()?;
    let plus = plus_profile_path()?;
    let external = external_profile_path()?;
    if plus.exists() && external.exists() {
        return Ok(());
    }

    let live = read_live_or_empty()?;
    let live_mode = detect_mode(&live);
    let (old_plus_candidates, old_external_candidates) = old_profile_candidates()?;

    let plus_seed = first_existing(&old_plus_candidates).unwrap_or_else(|| live.clone());
    let external_seed = first_existing(&old_external_candidates).unwrap_or_else(|| live.clone());

    let plus_text = apply_plus(&plus_seed)?;
    let imported_external_key = find_external_bearer_token(&external_seed);
    let external_with_possible_key = apply_external(
        &external_seed,
        imported_external_key.as_deref(),
    )?;

    // Migrate an existing provider-scoped token into Keychain, but never inspect auth.json.
    if keychain_get().is_none() {
        if let Some(existing_key) = find_external_bearer_token(&external_with_possible_key)
            .or_else(|| find_external_bearer_token(&live))
        {
            let _ = keychain_set(&existing_key);
        }
    }

    atomic_write(&plus, &plus_text)?;
    atomic_write(&external, &strip_external_secret(&external_with_possible_key)?)?;
    state_write(live_mode)?;
    Ok(())
}

#[tauri::command]
fn initialize_profiles() -> Result<ActionResult, String> {
    ensure_initialized()?;
    Ok(ActionResult {
        ok: true,
        message: "初始化完成：Plus / External 两套模式档案已建立，auth.json 未修改。".to_string(),
    })
}

#[tauri::command]
fn save_external_key(key: String) -> Result<ActionResult, String> {
    if key.trim().is_empty() {
        return Err("API Key 不能为空".to_string());
    }
    keychain_set(key.trim())?;

    // If External mode is live, update its provider-scoped token immediately and restart Codex.
    let live = read_live_or_empty()?;
    if detect_mode(&live) == Mode::External {
        let backup = backup_live(&live)?;
        let live_path = live_config_path()?;
        match apply_external(&live, Some(key.trim())).and_then(|text| atomic_write(&live_path, &text)) {
            Ok(_) => {
                restart_codex()?;
                return Ok(ActionResult {
                    ok: true,
                    message: "Key 已保存并更新到 External live config；Codex 已重启，请新建聊天。".to_string(),
                });
            }
            Err(err) => {
                let original = read_text(&backup)?;
                let _ = atomic_write(&live_path, &original);
                return Err(format!("更新 External live config 失败，已回滚：{err}"));
            }
        }
    }

    Ok(ActionResult {
        ok: true,
        message: "External API Key 已保存到 macOS Keychain。".to_string(),
    })
}

#[tauri::command]
fn switch_mode(mode: String) -> Result<ActionResult, String> {
    ensure_initialized()?;
    let target_mode = match mode.as_str() {
        "plus" => Mode::Plus,
        "external" => Mode::External,
        _ => return Err("未知模式".to_string()),
    };

    let live_path = live_config_path()?;
    let live = read_live_or_empty()?;
    let current_mode = detect_mode(&live);

    // Persist the live state back to its explicit profile before leaving it.
    match current_mode {
        Mode::Plus => {
            let saved = apply_plus(&live)?;
            atomic_write(&plus_profile_path()?, &saved)?;
        }
        Mode::External => {
            let saved = strip_external_secret(&apply_external(&live, None)?)?;
            atomic_write(&external_profile_path()?, &saved)?;
        }
        Mode::Unknown => {}
    }

    let target_path = match target_mode {
        Mode::Plus => plus_profile_path()?,
        Mode::External => external_profile_path()?,
        Mode::Unknown => unreachable!(),
    };
    let target_seed = read_text(&target_path)?;
    let synced = sync_common(&live, &target_seed)?;

    let final_text = match target_mode {
        Mode::Plus => apply_plus(&synced)?,
        Mode::External => {
            let key = keychain_get().ok_or_else(|| "请先保存 External API Key".to_string())?;
            apply_external(&synced, Some(&key))?
        }
        Mode::Unknown => unreachable!(),
    };
    validate_toml(&final_text)?;

    let backup = backup_live(&live)?;
    if let Err(err) = atomic_write(&live_path, &final_text) {
        let original = read_text(&backup)?;
        let _ = atomic_write(&live_path, &original);
        return Err(format!("切换写入失败，已回滚：{err}"));
    }

    // Keep profile copies explicit and secret-free.
    let profile_text = match target_mode {
        Mode::Plus => apply_plus(&final_text)?,
        Mode::External => strip_external_secret(&final_text)?,
        Mode::Unknown => unreachable!(),
    };
    atomic_write(&target_path, &profile_text)?;
    state_write(target_mode.clone())?;

    if let Err(err) = restart_codex() {
        return Ok(ActionResult {
            ok: true,
            message: format!(
                "配置已切到 {}，但自动重启 Codex 失败：{err}。请手动完全退出并重新打开 Codex，然后新建聊天。",
                match target_mode { Mode::Plus => "ChatGPT Plus", Mode::External => "External API", Mode::Unknown => "未知" }
            ),
        });
    }

    Ok(ActionResult {
        ok: true,
        message: format!(
            "已切到 {}，Codex Desktop 已重启。请新建聊天；旧线程不会原地换 Provider。",
            match target_mode { Mode::Plus => "ChatGPT Plus", Mode::External => "External API", Mode::Unknown => "未知" }
        ),
    })
}

#[tauri::command]
fn restore_latest_backup() -> Result<ActionResult, String> {
    ensure_dirs()?;
    let backup = latest_backup()?.ok_or_else(|| "没有可恢复的备份".to_string())?;
    let text = read_text(&backup)?;
    validate_toml(&text)?;
    atomic_write(&live_config_path()?, &text)?;
    let mode = detect_mode(&text);
    state_write(mode)?;
    let _ = restart_codex();
    Ok(ActionResult {
        ok: true,
        message: format!("已恢复最近备份：{}。Codex 已尝试重启。", backup.display()),
    })
}

#[tauri::command]
fn get_status() -> Result<Status, String> {
    ensure_dirs()?;
    let live = read_live_or_empty()?;
    let mode = detect_mode(&live);
    let provider = model_provider(&live);
    let initialized = plus_profile_path()?.exists() && external_profile_path()?.exists();
    let auth_exists = auth_path()?.exists();
    let key_saved = keychain_get().is_some();

    let routing_ok = mode == Mode::External
        && provider.as_deref() == Some(EXTERNAL_PROVIDER_ID)
        && provider_base_url(&live, EXTERNAL_PROVIDER_ID).as_deref() == Some(EXTERNAL_URL)
        && provider_bearer_token(&live, EXTERNAL_PROVIDER_ID).is_some()
        && provider_requires_auth(&live, EXTERNAL_PROVIDER_ID) == Some(true);

    let state_mode = state_read().map(|s| s.mode);
    let warning = if let Some(saved) = state_mode {
        if saved != mode && saved != Mode::Unknown && mode != Mode::Unknown {
            Some("状态记录与 live config 不一致；以 live config 为准，建议重新切换一次。".to_string())
        } else if mode == Mode::External && !routing_ok {
            Some("External 模式配置不完整：Provider / endpoint / bearer token / OAuth 保留项至少有一项未闭环。".to_string())
        } else if !auth_exists {
            Some("未检测到 ~/.codex/auth.json；如果你希望保留 ChatGPT Plus，请先在 Codex Desktop 完成官方登录。".to_string())
        } else {
            None
        }
    } else if !auth_exists {
        Some("未检测到官方 auth.json；初始化前建议先确认 Codex Desktop 已登录 ChatGPT。".to_string())
    } else {
        None
    };

    Ok(Status {
        initialized,
        mode,
        live_provider: provider,
        auth_exists,
        key_saved,
        routing_ok,
        config_path: live_config_path()?.display().to_string(),
        last_backup: latest_backup()?.map(|p| p.display().to_string()),
        warning,
    })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            initialize_profiles,
            save_external_key,
            switch_mode,
            restore_latest_backup,
            get_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Switch Lite");
}
