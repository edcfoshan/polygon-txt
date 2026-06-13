use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod shp;
pub mod txt;
pub mod convert;
pub mod gdb;
pub mod gpkg;
pub mod geometry;
pub mod smoke;

use convert::{FieldMapping, HeaderConfig, ShpToTxtOptions, TxtToShpOptions};

// ─── IPC Types ───

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShpImportResult {
    files: Vec<ShpFileItem>,
    dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShpFileItem {
    shp_path: String,
    dbf_path: Option<String>,
    prj_path: Option<String>,
    name: String,
    field_names: Vec<String>,
    num_features: usize,
    shape_type: String,
    prj_text: Option<String>,
    crs_info: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GdbImportResult {
    path: String,
    name: String,
    layers: Vec<GdbLayerItem>,
    field_names: Vec<String>,
    num_features: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GpkgImportResult {
    files: Vec<GpkgFileItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GpkgFileItem {
    path: String,
    name: String,
    layers: Vec<GdbLayerItem>,
    field_names: Vec<String>,
    num_features: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GdbLayerItem {
    name: String,
    field_names: Vec<String>,
    num_features: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxtImportResult {
    files: Vec<TxtFileItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxtFileItem {
    path: String,
    name: String,
    size: u64,
    parse_log: String,
    plot_count: usize,
    point_count: usize,
    crs_info: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConvertResultPayload {
    success: bool,
    message: String,
    output_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SmokeTestConfig {
    pub txt_path: PathBuf,
    pub output_dir: PathBuf,
}

// ─── Commands ───

#[tauri::command]
fn pick_shp_files(app: tauri::AppHandle) -> Result<ShpImportResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let files = app
        .dialog()
        .file()
        .add_filter("SHP 文件", &["shp"])
        .blocking_pick_files();

    let picked = match files {
        Some(f) => f,
        None => {
            return Ok(ShpImportResult {
                files: vec![],
                dir: String::new(),
            })
        }
    };

    let base_dir = picked
        .first()
        .and_then(|f| f.as_path())
        .and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut items = Vec::new();
    for file in &picked {
        let shp_path = match file.as_path() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        if shp_path.extension().map(|e| e != "shp").unwrap_or(true) {
            continue;
        }
        match shp::read_shp_file_group(&shp_path) {
            Ok(info) => items.push(ShpFileItem {
                shp_path: info.shp_path,
                dbf_path: info.dbf_path,
                prj_path: info.prj_path,
                name: info.name,
                field_names: info.field_names,
                num_features: info.num_features,
                shape_type: info.shape_type,
                prj_text: info.prj_text,
                crs_info: info.crs_info,
            }),
            Err(e) => eprintln!("读 SHP 失败: {}", e),
        }
    }

    Ok(ShpImportResult {
        files: items,
        dir: base_dir,
    })
}

#[tauri::command]
fn import_gdb(app: tauri::AppHandle) -> Result<GdbImportResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let dir = app.dialog().file().blocking_pick_folder();
    let gdb_path = match dir.and_then(|d| d.as_path().map(|p| p.to_path_buf())) {
        Some(p) => p,
        None => {
            return Err("未选择文件夹".to_string());
        }
    };

    // Validate: either has .gdb extension, or contains GDB system catalog file
    let has_gdb_ext = gdb_path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase() == "gdb")
        .unwrap_or(false);
    let has_gdb_catalog = gdb_path.join("a00000001.gdbtable").exists();
    if !has_gdb_ext && !has_gdb_catalog {
        return Err("请选择 .gdb 文件夹".to_string());
    }

    let info = gdb::read_gdb(&gdb_path)?;
    let name = gdb_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let field_names = info.all_field_names.first().cloned().unwrap_or_default();
    let num_features = info.layers.first().map(|l| l.num_features).unwrap_or(0);
    let layers = info
        .layers
        .iter()
        .map(|l| GdbLayerItem {
            name: l.name.clone(),
            field_names: l.field_names.clone(),
            num_features: l.num_features,
        })
        .collect();

    Ok(GdbImportResult {
        path: gdb_path.to_string_lossy().to_string(),
        name,
        layers,
        field_names,
        num_features,
    })
}

#[tauri::command]
fn import_gpkg(app: tauri::AppHandle) -> Result<GpkgImportResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let files = app
        .dialog()
        .file()
        .add_filter("GeoPackage 文件", &["gpkg"])
        .blocking_pick_files();

    let picked = match files {
        Some(f) => f,
        None => return Ok(GpkgImportResult { files: vec![] }),
    };

    let mut items = Vec::new();
    for file in &picked {
        let gpkg_path = match file.as_path() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        if gpkg_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase() != "gpkg")
            .unwrap_or(true)
        {
            continue;
        }
        match gpkg::read_gpkg(&gpkg_path) {
            Ok(info) => {
                let name = gpkg_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let field_names = info.all_field_names.first().cloned().unwrap_or_default();
                let num_features: usize = info.layers.iter().map(|l| l.num_features).sum();
                let layers = info
                    .layers
                    .iter()
                    .map(|l| GdbLayerItem {
                        name: l.name.clone(),
                        field_names: l.field_names.clone(),
                        num_features: l.num_features,
                    })
                    .collect();
                items.push(GpkgFileItem {
                    path: gpkg_path.to_string_lossy().to_string(),
                    name,
                    layers,
                    field_names,
                    num_features,
                });
            }
            Err(e) => eprintln!("读 GPKG 失败: {}", e),
        }
    }

    Ok(GpkgImportResult { files: items })
}

#[tauri::command]
fn pick_txt_files(app: tauri::AppHandle) -> Result<TxtImportResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let files = app
        .dialog()
        .file()
        .add_filter("TXT 文件", &["txt"])
        .blocking_pick_files();

    let picked = match files {
        Some(f) => f,
        None => return Ok(TxtImportResult { files: vec![] }),
    };

    let mut items = Vec::new();
    for file in &picked {
        let path = match file.as_path() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };

        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("读 TXT 失败: {}", e);
                continue;
            }
        };

        let parsed = txt::parse_txt(&text);
        let total_points: usize = parsed.plots.iter().map(|p| p.coords.len()).sum();

        let mut crs = HashMap::new();
        for key in &["坐标系", "几度分带", "投影类型", "计量单位", "带号", "精度"] {
            if let Some(v) = parsed.attrs.get(*key) {
                crs.insert(key.to_string(), v.clone());
            }
        }

        let log = generate_parse_log(&path, &parsed, total_points);

        items.push(TxtFileItem {
            path: path.to_string_lossy().to_string(),
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            size: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            parse_log: log,
            plot_count: parsed.plots.len(),
            point_count: total_points,
            crs_info: crs,
        });
    }

    Ok(TxtImportResult { files: items })
}

#[tauri::command]
fn read_shp_to_txt_preview(
    shp_paths: Vec<String>,
    source_type: Option<String>,
    source_path: Option<String>,
    header_cfg: HeaderConfig,
    field_mapping: FieldMapping,
    options: ShpToTxtOptions,
    selected_layers: Option<Vec<String>>,
) -> Result<String, String> {
    let shp_bufs: Vec<PathBuf> = shp_paths.iter().map(PathBuf::from).collect();
    let source_buf = source_path.as_ref().map(PathBuf::from);

    convert::shp_to_txt_preview(
        &shp_bufs,
        source_type.as_deref(),
        source_buf.as_ref(),
        &header_cfg,
        &field_mapping,
        &options,
        selected_layers.as_deref(),
    )
    .map_err(|e| format!("预览失败: {}", e))
}

#[tauri::command]
fn run_shp_to_txt(
    shp_paths: Vec<String>,
    source_type: Option<String>,
    source_path: Option<String>,
    header_cfg: HeaderConfig,
    field_mapping: FieldMapping,
    options: ShpToTxtOptions,
    output_dir: String,
    selected_layers: Option<Vec<String>>,
) -> Result<ConvertResultPayload, String> {
    let out_dir = PathBuf::from(&output_dir);
    let shp_bufs: Vec<PathBuf> = shp_paths.iter().map(PathBuf::from).collect();
    let source_buf = source_path.as_ref().map(PathBuf::from);

    let result = convert::convert_shp_to_txt(
        &shp_bufs,
        source_type.as_deref(),
        source_buf.as_ref(),
        &header_cfg,
        &field_mapping,
        &options,
        &out_dir,
        selected_layers.as_deref(),
    )
    .map_err(|e| format!("面转 TXT 失败: {}", e))?;

    Ok(ConvertResultPayload {
        success: result.success,
        message: result.message,
        output_files: result.output_files,
    })
}

#[tauri::command]
fn run_txt_to_shp(
    txt_paths: Vec<String>,
    options: TxtToShpOptions,
    header_cfg: HeaderConfig,
) -> Result<ConvertResultPayload, String> {
    let txt_bufs: Vec<PathBuf> = txt_paths.iter().map(PathBuf::from).collect();
    let result = convert::convert_txt_to_shp(&txt_bufs, &options, &header_cfg)
        .map_err(|e| format!("TXT 转面失败: {}", e))?;

    Ok(ConvertResultPayload {
        success: result.success,
        message: result.message,
        output_files: result.output_files,
    })
}

#[tauri::command]
fn pick_output_dir(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dir = app.dialog().file().blocking_pick_folder();
    Ok(dir.and_then(|d| d.as_path().map(|p| p.to_string_lossy().to_string())))
}

#[tauri::command]
fn pick_shp_files_from_paths(paths: Vec<String>) -> Result<ShpImportResult, String> {
    let mut items = Vec::new();
    let base_dir = paths
        .first()
        .and_then(|p| Path::new(p).parent())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    for p in &paths {
        let shp_path = PathBuf::from(p);
        if shp_path.extension().map(|e| e != "shp").unwrap_or(true) {
            continue;
        }
        match shp::read_shp_file_group(&shp_path) {
            Ok(info) => items.push(ShpFileItem {
                shp_path: info.shp_path,
                dbf_path: info.dbf_path,
                prj_path: info.prj_path,
                name: info.name,
                field_names: info.field_names,
                num_features: info.num_features,
                shape_type: info.shape_type,
                prj_text: info.prj_text,
                crs_info: info.crs_info,
            }),
            Err(e) => eprintln!("拖放读 SHP 失败: {}", e),
        }
    }
    Ok(ShpImportResult { files: items, dir: base_dir })
}

#[tauri::command]
fn pick_txt_files_from_paths(paths: Vec<String>) -> Result<TxtImportResult, String> {
    let mut items = Vec::new();
    for p in &paths {
        let path = PathBuf::from(p);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => { eprintln!("拖放读 TXT 失败: {}", e); continue; }
        };
        let parsed = txt::parse_txt(&text);
        let total_points: usize = parsed.plots.iter().map(|p| p.coords.len()).sum();
        let mut crs = HashMap::new();
        for key in &["坐标系", "几度分带", "投影类型", "计量单位", "带号", "精度"] {
            if let Some(v) = parsed.attrs.get(*key) { crs.insert(key.to_string(), v.clone()); }
        }
        let log = generate_parse_log(&path, &parsed, total_points);
        items.push(TxtFileItem {
            path: path.to_string_lossy().to_string(),
            name: path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            size: std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0),
            parse_log: log,
            plot_count: parsed.plots.len(),
            point_count: total_points,
            crs_info: crs,
        });
    }
    Ok(TxtImportResult { files: items })
}

#[tauri::command]
fn read_txt_preview(path: String) -> Result<String, String> {
    let text = std::fs::read_to_string(&path).map_err(|e| format!("读 TXT 失败: {}", e))?;
    let parsed = txt::parse_txt(&text);
    let total_points: usize = parsed.plots.iter().map(|p| p.coords.len()).sum();
    Ok(generate_parse_log(Path::new(&path), &parsed, total_points))
}

/// 生成 TXT 解析日志（前端展示用）
#[tauri::command]
fn minimize_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|e| format!("窗口最小化失败: {}", e))
}

#[tauri::command]
fn close_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.close().map_err(|e| format!("窗口关闭失败: {}", e))
}

fn generate_parse_log(
    path: &Path,
    parsed: &txt::TxtParseResult,
    total_points: usize,
) -> String {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let crs = parsed
        .attrs
        .get("坐标系")
        .map(|s| s.as_str())
        .unwrap_or("未识别");
    let band = parsed
        .attrs
        .get("几度分带")
        .map(|s| s.as_str())
        .unwrap_or("?");
    let zone = parsed
        .attrs
        .get("带号")
        .map(|s| s.as_str())
        .unwrap_or("?");
    let precision = parsed
        .attrs
        .get("精度")
        .map(|s| s.as_str())
        .unwrap_or("0.001");

    let mut log = format!("◆ {}\n", name);
    log.push_str(&format!(
        "  坐标系: {} / {}度分带 / 带号{}\n", crs, band, zone
    ));
    log.push_str(&format!(
        "  精度: {} | 地块: {} | 坐标点: 共{}个\n",
        precision,
        parsed.plots.len(),
        total_points
    ));

    let max_show = parsed.plots.len().min(10);
    for (j, plot) in parsed.plots.iter().enumerate().take(max_show) {
        let prefix = if j == max_show - 1 { "└─" } else { "├─" };
        log.push_str(&format!(
            "  {} {} {} {} {}点\n",
            prefix,
            plot.name,
            plot.use_field,
            plot.area,
            plot.coords.len()
        ));
    }
    if parsed.plots.len() > 10 {
        log.push_str(&format!(
            "  ... 还有 {} 个地块\n",
            parsed.plots.len() - 10
        ));
    }
    log
}

// ─── App Entry ───

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            pick_shp_files,
            import_gdb,
            import_gpkg,
            pick_txt_files,
            pick_output_dir,
            pick_shp_files_from_paths,
            pick_txt_files_from_paths,
            read_shp_to_txt_preview,
            read_txt_preview,
            run_shp_to_txt,
            run_txt_to_shp,
            minimize_window,
            close_window,
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
