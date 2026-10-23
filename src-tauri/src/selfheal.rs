// v3.3 自愈与更新辅助。
// 背景：NSIS 的卸载键/安装目录/快捷方式名均派生自 productName，v3.2 CI 曾将其改写为
// "polygon-txt" 导致 3.1 用户自动更新后安装目录漂移、快捷方式仍指旧目录。
// 本模块负责：安装版快捷方式修复 + 留包模式的下载/验签/静默安装。
// 便携版不接管安装版；启动时不删除其他副本或卸载记录。

pub const PRODUCT_DIR: &str = "极思G界址点互转工具";
pub const STALE_PRODUCT_DIR: &str = "polygon-txt";
pub const MAIN_BINARY: &str = "jisig-bpoint-converter.exe";
// tauri.conf.json plugins.updater.pubkey（base64 的 minisign 公钥文件），留包验签用
const UPDATER_PUBKEY_B64: &str = "dW50cnVzdGVkIGNvbWU6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEEwMjYzRkZGQzk3RkI1RjAKUldUd3RYL0ovejhtb05aR2YxeFIrNHNTZDNMdlE0Q01kY2dxMXUvaktBdnlka0VhV2dyTkV2YWUK";

/// latest.json 的 signature 字段 → minisign .sig 原文。
/// tauri 的 signature 是 .sig 文件内容的 base64（插件路径内部自行解码）；
/// 留包路径此前直接把 base64 喂给 Signature::decode 导致必失败（v4.0.1 修复），两种形态均兼容。
fn signature_minisign_str(signature: &str) -> Result<String, String> {
    use base64::Engine;
    let s = signature.trim();
    if s.starts_with("untrusted comment") {
        return Ok(s.replace("\r\n", "\n"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("签名 base64 解码失败：{e}"))?;
    String::from_utf8(bytes)
        .map_err(|e| format!("签名不是合法 UTF-8：{e}"))
        .map(|t| t.replace("\r\n", "\n"))
}

// ── Tauri 命令 ──

#[tauri::command]
pub fn restart_into_updated_app(fix_links: bool) -> Result<(), String> {
    imp::restart_into_updated_app(fix_links)
}

#[tauri::command]
pub async fn download_and_run_setup(
    app: tauri::AppHandle,
    url: String,
    signature: String,
    version: String,
    keep_save: bool,
    create_shortcuts: bool,
) -> Result<String, String> {
    imp::download_and_run_setup(app, url, signature, version, keep_save, create_shortcuts).await
}

// ── 启动静默自愈（lib.rs setup 线程调用，无 UI，失败仅记录）──

pub fn silent_selfheal(app: &tauri::AppHandle) {
    // 开发模式不修改系统快捷方式。
    if cfg!(debug_assertions) {
        return;
    }
    imp::silent_selfheal(app);
}

#[cfg(windows)]
mod imp {
    use std::io::Read;
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use tauri::Emitter;
    use windows::core::{Interface, PCWSTR, PWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{
        IShellLinkW, ShellLink, FOLDERID_Desktop, FOLDERID_Programs,
        KF_FLAG_DEFAULT, SHGetKnownFolderPath, SLGP_UNCPRIORITY,
    };

    use super::{MAIN_BINARY, PRODUCT_DIR, STALE_PRODUCT_DIR, UPDATER_PUBKEY_B64};

    // ── 基础工具 ──

    fn to_wide(s: impl AsRef<Path>) -> Vec<u16> {
        let mut v: Vec<u16> = s.as_ref().as_os_str().encode_wide().collect();
        v.push(0);
        v
    }

    fn com_init() {
        // 已初始化（S_FALSE）或模式不同（RPC_E_CHANGED_MODE）时忽略，均表示 COM 可用
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
    }

    fn known_folder(id: &windows::core::GUID) -> Result<PathBuf, String> {
        unsafe {
            let pw: PWSTR = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None)
                .map_err(|e| e.to_string())?;
            let s = pw.to_string().map_err(|e| e.to_string())?;
            CoTaskMemFree(Some(pw.as_ptr() as *const _));
            Ok(PathBuf::from(s))
        }
    }

    // NSIS 的注册记录包含用户实际选择的目录，不能假定始终装在 LOCALAPPDATA。
    fn registered_install_dir() -> Option<PathBuf> {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{PRODUCT_DIR}"))
            .ok()?;
        let location: String = key.get_value("InstallLocation").ok()?;
        let location = location.trim().trim_matches('"');
        if location.is_empty() { return None; }
        Some(PathBuf::from(location))
    }

    fn installed_exe(dir: &Path) -> Option<PathBuf> {
        let exe = dir.join(MAIN_BINARY);
        (exe.is_file() && dir.join("uninstall.exe").is_file()).then_some(exe)
    }

    fn same_path(a: &Path, b: &Path) -> bool {
        match (a.canonicalize(), b.canonicalize()) {
            (Ok(a), Ok(b)) => a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()),
            _ => false,
        }
    }

    // ── 快捷方式读写（COM IShellLinkW）──

    fn read_shortcut_target(path: &Path) -> Option<PathBuf> {
        unsafe {
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let pf: IPersistFile = link.cast().ok()?;
            let w = to_wide(path);
            pf.Load(PCWSTR(w.as_ptr()), STGM_READ).ok()?;
            let mut buf = [0u16; 1024];
            link.GetPath(&mut buf, std::ptr::null_mut(), SLGP_UNCPRIORITY.0 as u32)
                .ok()?;
            let end = buf.iter().position(|&c| c == 0).unwrap_or(0);
            if end == 0 {
                return None;
            }
            Some(PathBuf::from(String::from_utf16_lossy(&buf[..end])))
        }
    }

    fn write_shortcut(link_path: &Path, target: &Path) -> Result<(), String> {
        com_init();
        unsafe {
            let shell: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
            let pf: IPersistFile = shell.cast().map_err(|e| e.to_string())?;
            let lw = to_wide(link_path);
            // 保留已有快捷方式的 AppUserModelID 等属性。
            if link_path.is_file() {
                let _ = pf.Load(PCWSTR(lw.as_ptr()), STGM_READ);
            }
            let tw = to_wide(target);
            shell.SetPath(PCWSTR(tw.as_ptr())).map_err(|e| e.to_string())?;
            if let Some(dir) = target.parent() {
                let dw = to_wide(dir);
                shell
                    .SetWorkingDirectory(PCWSTR(dw.as_ptr()))
                    .map_err(|e| e.to_string())?;
            }
            shell
                .SetIconLocation(PCWSTR(tw.as_ptr()), 0)
                .map_err(|e| e.to_string())?;
            pf.Save(PCWSTR(lw.as_ptr()), false).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn collect_lnk_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth < 4 {
                    collect_lnk_files(&p, depth + 1, out);
                }
            } else if p.extension().map(|x| x == "lnk").unwrap_or(false) {
                out.push(p);
            }
        }
    }

    // 修复（create=false，自愈路径：只修不建）或修复+补建（create=true，更新完成路径）
    pub fn repair_shortcuts(create: bool) -> Result<Vec<String>, String> {
        let cur = std::env::current_exe().map_err(|e| e.to_string())?;
        let Some(dir) = registered_install_dir() else { return Ok(Vec::new()) };
        let bases: Vec<PathBuf> = [FOLDERID_Desktop, FOLDERID_Programs].iter()
            .filter_map(|id| known_folder(id).ok()).collect();
        repair_shortcuts_in(&cur, &dir, &bases, create)
    }

    fn repair_shortcuts_in(cur: &Path, dir: &Path, bases: &[PathBuf], create: bool) -> Result<Vec<String>, String> {
        let Some(target) = installed_exe(dir) else { return Ok(Vec::new()) };
        // 同名的便携 exe 也不能冒充注册安装版。
        if !same_path(cur, &target) { return Ok(Vec::new()); }
        com_init();
        let mut links = Vec::new();
        for base in bases { collect_lnk_files(base, 0, &mut links); }

        let mut actions = Vec::new();
        for lnk in &links {
            let name = lnk.file_stem().unwrap_or_default();
            if name != PRODUCT_DIR && name != STALE_PRODUCT_DIR { continue; }
            if read_shortcut_target(lnk).is_some_and(|t| same_path(&t, &target)) { continue; }
            match write_shortcut(lnk, &target) {
                Ok(()) => actions.push(format!("已修复快捷方式：{}", lnk.display())),
                Err(e) => actions.push(format!("快捷方式修复失败：{} ({e})", lnk.display())),
            }
        }

        if create {
            // 仅补建标准位置缺的快捷方式（桌面 + 开始菜单）
            for base in bases {
                let std_lnk = base.join(format!("{PRODUCT_DIR}.lnk"));
                if !std_lnk.is_file() {
                    write_shortcut(&std_lnk, &target)?;
                    actions.push(format!("已新建快捷方式：{}", std_lnk.display()));
                }
            }
        }
        Ok(actions)
    }

    pub fn silent_selfheal(app: &tauri::AppHandle) {
        let mut logs = Vec::new();
        if let Ok(a) = repair_shortcuts(false) {
            logs.extend(a);
        }
        persist_log(app, &logs);
    }

    fn persist_log(app: &tauri::AppHandle, logs: &[String]) {
        if logs.is_empty() {
            return;
        }
        use tauri::Manager;
        let Ok(dir) = app.path().app_log_dir() else { return };
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let line = format!(
            "[{}] {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            logs.join("\n")
        );
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("selfheal.log"))
        {
            use std::io::Write;
            let _ = f.write_all(line.as_bytes());
        }
    }

    // ── 重启进新装目录 ──

    pub fn restart_into_updated_app(fix_links: bool) -> Result<(), String> {
        let exe = registered_install_dir().and_then(|dir| installed_exe(&dir))
            .ok_or("未找到更新后的安装版，请重新运行安装包")?;
        let _ = repair_shortcuts(fix_links);
        Command::new(&exe).spawn().map_err(|e| e.to_string())?;
        std::process::exit(0);
    }

    // ── 留包：下载 + 验签 + 静默安装 ──

    pub async fn download_and_run_setup(
        app: tauri::AppHandle,
        url: String,
        signature: String,
        version: String,
        keep_save: bool,
        create_shortcuts: bool,
    ) -> Result<String, String> {
        let r: Result<String, String> =
            tauri::async_runtime::spawn_blocking(move || download_impl(&app, &url, &signature, &version, keep_save, create_shortcuts))
                .await
                .map_err(|e| e.to_string())?;
        r
    }

    fn download_impl(
        app: &tauri::AppHandle,
        url: &str,
        signature: &str,
        version: &str,
        keep_save: bool,
        create_shortcuts: bool,
    ) -> Result<String, String> {
        if version.is_empty() || !version.chars().all(|c| c.is_ascii_alphanumeric() || ".-+".contains(c)) {
            return Err("更新版本号不合法".into());
        }
        let base = if keep_save {
            known_folder(&FOLDERID_Desktop)?
        } else {
            let dir = std::env::temp_dir().join("jisig-bpoint-converter-updater");
            std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建更新临时目录：{e}"))?;
            dir
        };
        let dest = base.join(format!("JisigG_{version}_x64-setup.exe"));

        let mut resp = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(900))
            .build()
            .map_err(|e| e.to_string())?
            .get(url)
            .send()
            .map_err(|e| format!("下载失败：{e}"))?;
        if !resp.status().is_success() {
            return Err(format!("下载失败：HTTP {}", resp.status()));
        }
        let total = resp.content_length();
        let _ = app.emit(
            "upd-dl-progress",
            serde_json::json!({"event": "Started", "data": {"contentLength": total}}),
        );
        let mut file = std::fs::File::create(&dest).map_err(|e| format!("无法写入安装包：{e}"))?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = resp.read(&mut buf).map_err(|e| format!("下载中断：{e}"))?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| format!("写入失败：{e}"))?;
            let _ = app.emit(
                "upd-dl-progress",
                serde_json::json!({"event": "Progress", "data": {"chunkLength": n}}),
            );
        }
        drop(file);

        verify_signature(&dest, signature)?;

        if keep_save {
            let _ = Command::new("explorer")
                .arg(format!("/select,{}", dest.display()))
                .spawn();
        }
        let mut installer = Command::new(&dest);
        installer.args(["/S", "/R", "/UPDATE"]);
        if !create_shortcuts { installer.arg("/NS"); }
        installer.spawn()
            .map_err(|e| format!("无法启动安装包：{e}"))?;
        Ok(dest.display().to_string())
    }

    fn verify_signature(file: &Path, signature: &str) -> Result<(), String> {
        use base64::Engine;
        use minisign_verify::{PublicKey, Signature};
        let pem = base64::engine::general_purpose::STANDARD
            .decode(UPDATER_PUBKEY_B64)
            .map_err(|e| e.to_string())?;
        let pem = String::from_utf8(pem).map_err(|e| e.to_string())?;
        let pk = PublicKey::decode(&pem).map_err(|e| format!("公钥解析失败：{e}"))?;
        let sig_text = super::signature_minisign_str(signature)?;
        let sig = Signature::decode(&sig_text).map_err(|e| format!("签名解析失败：{e}"))?;
        let data = std::fs::read(file).map_err(|e| e.to_string())?;
        pk.verify(&data, &sig, false)
            .map_err(|e| format!("安装包验签失败：{e}"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn mock_install(dir: &Path) -> PathBuf {
            std::fs::create_dir_all(dir).unwrap();
            let exe = dir.join(MAIN_BINARY);
            std::fs::write(&exe, b"installed application").unwrap();
            std::fs::write(dir.join("uninstall.exe"), b"uninstaller").unwrap();
            exe
        }

        #[test]
        fn portable_never_changes_installed_files_or_shortcuts() {
            let temp = tempfile::tempdir().unwrap();
            let dir = temp.path().join(PRODUCT_DIR);
            let installed = mock_install(&dir);
            let desktop = temp.path().join("Desktop");
            std::fs::create_dir(&desktop).unwrap();
            let link = desktop.join(format!("{PRODUCT_DIR}.lnk"));
            write_shortcut(&link, &installed).unwrap();
            let original = std::fs::read(&link).unwrap();
            let portable_dir = temp.path().join("portable");
            std::fs::create_dir(&portable_dir).unwrap();
            // 包含未改名的绿色 exe，不能只按文件名判断是否为安装版。
            for name in [MAIN_BINARY, "极思G界址点互转工具_4.7.1_x64-portable.exe"] {
                let portable = portable_dir.join(name);
                std::fs::write(&portable, b"portable application").unwrap();
                assert!(repair_shortcuts_in(&portable, &dir, &[desktop.clone()], true).unwrap().is_empty());
                assert_eq!(std::fs::read(&installed).unwrap(), b"installed application");
                assert!(dir.join("uninstall.exe").is_file());
                assert_eq!(std::fs::read(&link).unwrap(), original);
                assert_eq!(std::fs::read_dir(&desktop).unwrap().count(), 1);
            }
        }

        #[test]
        fn custom_install_repairs_versioned_portable_and_broken_links() {
            let temp = tempfile::tempdir().unwrap();
            let dir = temp.path().join("custom location");
            let installed = mock_install(&dir);
            let programs = temp.path().join("Programs");
            let nested = programs.join("旧版入口");
            std::fs::create_dir_all(&nested).unwrap();
            let link = programs.join(format!("{PRODUCT_DIR}.lnk"));
            let broken = nested.join(format!("{STALE_PRODUCT_DIR}.lnk"));
            let other = programs.join("其他软件.lnk");
            let old = temp.path().join("极思G界址点互转工具_4.6.0_x64-portable.exe");
            std::fs::write(&old, b"old portable").unwrap();
            write_shortcut(&link, &old).unwrap();
            write_shortcut(&broken, &temp.path().join("missing.exe")).unwrap();
            write_shortcut(&other, &old).unwrap();
            let other_before = std::fs::read(&other).unwrap();
            let logs = repair_shortcuts_in(&installed, &dir, &[programs], false).unwrap();
            assert_eq!(logs.len(), 2);
            assert!(same_path(&read_shortcut_target(&link).unwrap(), &installed));
            assert!(same_path(&read_shortcut_target(&broken).unwrap(), &installed));
            assert_eq!(std::fs::read(&other).unwrap(), other_before);
            assert_eq!(std::fs::read(&old).unwrap(), b"old portable");
        }

        #[test]
        fn shortcut_creation_respects_user_choice_and_is_idempotent() {
            let temp = tempfile::tempdir().unwrap();
            let dir = temp.path().join("installed");
            let installed = mock_install(&dir);
            let bases: Vec<_> = ["Desktop", "Programs"].iter().map(|name| {
                let base = temp.path().join(name);
                std::fs::create_dir(&base).unwrap();
                base
            }).collect();
            assert!(repair_shortcuts_in(&installed, &dir, &bases, false).unwrap().is_empty());
            for base in &bases { assert_eq!(std::fs::read_dir(base).unwrap().count(), 0); }
            assert_eq!(repair_shortcuts_in(&installed, &dir, &bases, true).unwrap().len(), 2);
            for base in &bases {
                let link = base.join(format!("{PRODUCT_DIR}.lnk"));
                assert!(same_path(&read_shortcut_target(&link).unwrap(), &installed));
            }
            assert!(repair_shortcuts_in(&installed, &dir, &bases, true).unwrap().is_empty());
        }

        #[test]
        fn incomplete_installation_does_not_claim_shortcuts() {
            let temp = tempfile::tempdir().unwrap();
            let exe = temp.path().join(MAIN_BINARY);
            std::fs::write(&exe, b"standalone executable").unwrap();
            assert!(installed_exe(temp.path()).is_none());
            assert!(repair_shortcuts_in(&exe, temp.path(), &[temp.path().to_path_buf()], true).unwrap().is_empty());
            assert!(!temp.path().join(format!("{PRODUCT_DIR}.lnk")).exists());
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn silent_selfheal(_app: &tauri::AppHandle) {}

    pub fn restart_into_updated_app(_fix_links: bool) -> Result<(), String> {
        Err("仅支持 Windows".into())
    }

    pub async fn download_and_run_setup(
        _app: tauri::AppHandle,
        _url: String,
        _signature: String,
        _version: String,
        _keep_save: bool,
        _create_shortcuts: bool,
    ) -> Result<String, String> {
        Err("仅支持 Windows".into())
    }
}

#[cfg(test)]
mod tests {
    use super::signature_minisign_str;

    // v4.0.0 CI 产出的 latest.json signature（.sig 文件内容的 base64），公开数据
    const CI_SIG_B64: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUd3RYL0ovejhtb01GOU9QQno0SzNJMGV2TE8zR3pvRnM0T1Bqa0h5QldHMWZkZTZXb01ERzBqb3J6OEJLdi9GUGpIUVQzeGdSVUwzMFh3WnhlM3JZNTBhRnQ2T0VlNkFJPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzg4ODY2MjMyCWZpbGU65p6B5oCdR+eVjOWdgOeCueS6kui9rOW3peWFt180LjAuMF94NjQtc2V0dXAuZXhlClU2K1RwMkFEb09rbnVUWkZBV2tmd2NhMjZUZVJkODl4V3NCTkxBTzhiaHBDNUFGSGtHWkc1bElRc0k2ZHZuK2ZzV0QxaERoT1h0OE1qbjJ6bUNlbkF3PT0K";

    #[test]
    fn signature_base64_form_normalizes() {
        let text = signature_minisign_str(CI_SIG_B64).expect("base64 形态应规范化成功");
        assert!(text.starts_with("untrusted comment: signature from tauri secret key"), "应以 minisign 注释行开头: {text}");
        assert!(text.contains("trusted comment: timestamp:"), "应含 trusted comment 行");
        // 规范化结果必须是 minisign-verify 可解析的签名
        minisign_verify::Signature::decode(&text).expect("规范化结果应可被 Signature::decode 解析");
    }

    #[test]
    fn signature_raw_form_passthrough() {
        let raw = "untrusted comment: signature from tauri secret key\nRWw2+f8P5gS7testdata=\ntrusted comment: timestamp:1\tfile:x\nAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n";
        let text = signature_minisign_str(raw).expect("原文形态应原样通过");
        // 函数会 trim 首尾空白（对 minisign 行解析无影响），正文逐行应保持一致
        assert_eq!(text, raw.trim());
    }

    #[test]
    fn signature_crlf_normalized() {
        let raw = "untrusted comment: x\r\nQUJD\r\n";
        let text = signature_minisign_str(raw).unwrap();
        assert!(!text.contains('\r'), "CRLF 应被归一为 LF");
    }

    #[test]
    fn signature_invalid_base64_rejected() {
        assert!(signature_minisign_str("!!不是base64!!").is_err());
    }
}
