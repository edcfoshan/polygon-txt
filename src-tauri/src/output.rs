//! CLI 预检与实际转换共用同一次字节生成；捕获期间不创建目录或文件。
use std::{
    cell::RefCell,
    io,
    path::{Path, PathBuf},
};

pub type PlannedFile = (PathBuf, Vec<u8>);
thread_local! { static PLAN: RefCell<Option<Vec<PlannedFile>>> = const { RefCell::new(None) }; }

struct CaptureGuard;
impl Drop for CaptureGuard {
    fn drop(&mut self) {
        PLAN.with(|p| {
            p.borrow_mut().take();
        });
    }
}

pub fn capture<T>(f: impl FnOnce() -> Result<T, String>) -> Result<(T, Vec<PlannedFile>), String> {
    PLAN.with(|p| {
        if p.borrow().is_some() {
            return Err("不可嵌套捕获转换输出".to_string());
        }
        *p.borrow_mut() = Some(Vec::new());
        Ok(())
    })?;
    let _guard = CaptureGuard;
    let result = f()?;
    let files = PLAN.with(|p| p.borrow_mut().take().unwrap());
    Ok((result, files))
}

pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    if PLAN.with(|p| p.borrow().is_some()) {
        Ok(())
    } else {
        std::fs::create_dir_all(path)
    }
}

pub fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let captured = PLAN.with(|p| {
        let mut plan = p.borrow_mut();
        if let Some(files) = plan.as_mut() {
            files.push((path.as_ref().to_path_buf(), bytes.as_ref().to_vec()));
            true
        } else {
            false
        }
    });
    if captured {
        Ok(())
    } else {
        std::fs::write(path, bytes)
    }
}

pub fn conflicts(files: &[PlannedFile]) -> Vec<String> {
    files
        .iter()
        .filter(|(p, _)| std::fs::symlink_metadata(p).is_ok())
        .map(|(p, _)| p.display().to_string())
        .collect()
}

pub fn validate(files: &[PlannedFile]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for (path, _) in files {
        let key = path.to_string_lossy().to_string();
        #[cfg(target_os = "windows")]
        let key = key.to_lowercase();
        if !seen.insert(key) {
            return Err(format!("同批输出文件名重复：{}", path.display()));
        }
        if path.is_dir() {
            return Err(format!("输出文件位置已被目录占用：{}", path.display()));
        }
        for parent in path.ancestors().skip(1) {
            if parent.exists() && !parent.is_dir() {
                return Err(format!("输出路径的父级不是目录：{}", parent.display()));
            }
        }
    }
    Ok(())
}

pub fn commit(files: &[PlannedFile], overwrite: bool) -> Result<(), String> {
    validate(files)?;
    let collisions = conflicts(files);
    if !overwrite && !collisions.is_empty() {
        return Err(format!(
            "输出文件已存在（使用 --overwrite 才可覆盖）：\n{}",
            collisions.join("\n")
        ));
    }
    for (path, bytes) in files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true);
        if overwrite {
            options.create(true).truncate(true);
        } else {
            options.create_new(true);
        }
        use std::io::Write;
        let mut file = options
            .open(path)
            .map_err(|e| format!("{}: {}", path.display(), e))?;
        file.write_all(bytes)
            .map_err(|e| format!("{}: {}", path.display(), e))?;
    }
    Ok(())
}
