// v3.3 自愈与更新辅助。
// 背景：NSIS 的卸载键/安装目录/快捷方式名均派生自 productName，v3.2 CI 曾将其改写为
// "polygon-txt" 导致 3.1 用户自动更新后安装目录漂移、快捷方式仍指旧目录。
// 本模块负责：启动静默清理漂移残留 + 修复快捷方式 + 留包模式的下载/验签/静默安装。

pub const PRODUCT_DIR: &str = "极思G界址点互转工具";
pub const STALE_PRODUCT_DIR: &str = "polygon-txt";
pub const MAIN_BINARY: &str = "jisig-bpoint-converter.exe";
// tauri.conf.json plugins.updater.pubkey（base64 的 minisign 公钥文件），留包验签用
const UPDATER_PUBKEY_B64: &str = "dW50cnVzdGVkIGNvbWU6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEEwMjYzRkZGQzk3RkI1RjAKUldUd3RYL0ovejhtb05aR2YxeFIrNHNTZDNMdlE0Q01kY2dxMXUvaktBdnlka0VhV2dyTkV2YWUK";

// ── Tauri 命令 ──

#[tauri::command]
pub fn ensure_shortcuts() -> Result<Vec<String>, String> {
    imp::repair_shortcuts(true)
}

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
) -> Result<String, String> {
    imp::download_and_run_setup(app, url, signature, version).await
}

// ── 启动静默自愈（lib.rs setup 线程调用，无 UI，失败仅记录）──

pub fn silent_selfheal(app: &tauri::AppHandle) {
    // dev 模式运行目录≠安装目录，直接跳过，防误删开发机上的安装版
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
        IShellLinkW, ShellLink, FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop,
        KF_FLAG_DEFAULT, SHGetKnownFolderPath, SLGP_UNCPRIORITY,
    };

    use super::{MAIN_BINARY, PRODUCT_DIR, STALE_PRODUCT_DIR, UPDATER_PUBKEY_B64};

    // ── 基础工具 ──

    fn local_appdata() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }

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

    // 安装目录目标 exe：中文安装目录存在则优先（3.2 受害机更新后当前 exe 仍在旧目录，
    // 快捷方式与重启都应指向刚装好的新目录），否则退回当前 exe
    fn install_target_exe() -> Result<PathBuf, String> {
        let cur = std::env::current_exe().map_err(|e| e.to_string())?;
        if let Some(local) = local_appdata() {
            let cand = local.join(PRODUCT_DIR).join(MAIN_BINARY);
            if cand.is_file() {
                return Ok(cand);
            }
        }
        Ok(cur)
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
            let pf: IPersistFile = shell.cast().map_err(|e| e.to_string())?;
            let lw = to_wide(link_path);
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
        com_init();
        let target = install_target_exe()?;
        let target_dir = target.parent().ok_or("目标 exe 无父目录")?.to_path_buf();

        let mut links: Vec<PathBuf> = Vec::new();
        if let Ok(programs) = known_folder(&FOLDERID_Programs) {
            collect_lnk_files(&programs, 0, &mut links);
        }
        for id in [FOLDERID_Desktop, FOLDERID_PublicDesktop] {
            if let Ok(d) = known_folder(&id) {
                collect_lnk_files(&d, 0, &mut links);
            }
        }

        let mut actions = Vec::new();
        for lnk in &links {
            if let Some(t) = read_shortcut_target(lnk) {
                if t.file_name().map(|x| x == MAIN_BINARY).unwrap_or(false)
                    && t.parent() != Some(target_dir.as_path())
                {
                    write_shortcut(lnk, &target)?;
                    actions.push(format!("已修复快捷方式：{}", lnk.display()));
                }
            }
        }

        if create {
            // 仅补建标准位置缺的快捷方式（桌面 + 开始菜单）
            for base_id in [FOLDERID_Desktop, FOLDERID_Programs] {
                let base = known_folder(&base_id)?;
                let std_lnk = base.join(format!("{PRODUCT_DIR}.lnk"));
                if !std_lnk.is_file() {
                    write_shortcut(&std_lnk, &target)?;
                    actions.push(format!("已新建快捷方式：{}", std_lnk.display()));
                }
            }
        }
        Ok(actions)
    }

    // ── 残留清理 ──

    fn remove_stale_dirs(cur_exe: &Path, logs: &mut Vec<String>) {
        let Some(local) = local_appdata() else { return };
        let cur_dir = cur_exe.parent().map(Path::to_path_buf);
        for name in [STALE_PRODUCT_DIR, PRODUCT_DIR] {
            let dir = local.join(name);
            if !dir.join(MAIN_BINARY).is_file() {
                continue;
            }
            if cur_dir.as_deref() == Some(dir.as_path()) {
                continue;
            }
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => logs.push(format!("已删除残留安装目录：{}", dir.display())),
                Err(e) => logs.push(format!("残留目录暂无法删除（可能被占用）：{} ({e})", dir.display())),
            }
        }
    }

    fn remove_stale_uninstall_key(cur_exe: &Path, logs: &mut Vec<String>) {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let Some(local) = local_appdata() else { return };
        // 仅当本程序已装回中文目录时，polygon-txt 键才确定是 v3.2 残留
        if cur_exe.parent() != Some(local.join(PRODUCT_DIR).as_path()) {
            return;
        }
        let path = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\polygon-txt";
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if hkcu.open_subkey(path).is_ok() {
            match hkcu.delete_subkey_all(path) {
                Ok(()) => logs.push("已删除残留卸载注册表键：HKCU\\...\\Uninstall\\polygon-txt".into()),
                Err(e) => logs.push(format!("残留卸载键删除失败：{e}")),
            }
        }
    }

    pub fn silent_selfheal(app: &tauri::AppHandle) {
        let mut logs = Vec::new();
        let Ok(cur) = std::env::current_exe() else { return };
        remove_stale_dirs(&cur, &mut logs);
        remove_stale_uninstall_key(&cur, &mut logs);
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
        if fix_links {
            let _ = repair_shortcuts(true);
        }
        let cur = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut exe = cur.clone();
        if let Some(local) = local_appdata() {
            let cand = local.join(PRODUCT_DIR).join(MAIN_BINARY);
            // 更新装进了不同目录（漂移修复场景）：重启进新目录，避免 relaunch 回旧 exe
            if cand.is_file() && cand != cur {
                exe = cand;
            }
        }
        Command::new(&exe).spawn().map_err(|e| e.to_string())?;
        std::process::exit(0);
    }

    // ── 留包：下载 + 验签 + 静默安装 ──

    pub async fn download_and_run_setup(
        app: tauri::AppHandle,
        url: String,
        signature: String,
        version: String,
    ) -> Result<String, String> {
        let r: Result<String, String> =
            tauri::async_runtime::spawn_blocking(move || download_impl(&app, &url, &signature, &version))
                .await
                .map_err(|e| e.to_string())?;
        r
    }

    fn download_impl(
        app: &tauri::AppHandle,
        url: &str,
        signature: &str,
        version: &str,
    ) -> Result<String, String> {
        let desktop = known_folder(&FOLDERID_Desktop)?;
        let short = version.split('.').take(2).collect::<Vec<_>>().join(".");
        let dest = desktop.join(format!("polygon-txt_{short}_x64-setup.exe"));

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
        let mut file = std::fs::File::create(&dest).map_err(|e| format!("无法写入桌面：{e}"))?;
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

        let _ = Command::new("explorer")
            .arg(format!("/select,{}", dest.display()))
            .spawn();
        Command::new(&dest)
            .args(["/S", "/R"])
            .spawn()
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
        let sig = Signature::decode(signature).map_err(|e| format!("签名解析失败：{e}"))?;
        let data = std::fs::read(file).map_err(|e| e.to_string())?;
        pk.verify(&data, &sig, false)
            .map_err(|e| format!("安装包验签失败：{e}"))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn silent_selfheal(_app: &tauri::AppHandle) {}

    pub fn repair_shortcuts(_create: bool) -> Result<Vec<String>, String> {
        Err("仅支持 Windows".into())
    }

    pub fn restart_into_updated_app(_fix_links: bool) -> Result<(), String> {
        Err("仅支持 Windows".into())
    }

    pub async fn download_and_run_setup(
        _app: tauri::AppHandle,
        _url: String,
        _signature: String,
        _version: String,
    ) -> Result<String, String> {
        Err("仅支持 Windows".into())
    }
}
