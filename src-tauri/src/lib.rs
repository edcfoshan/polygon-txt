use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod shp;
pub mod txt;
pub mod convert;
pub mod gdb;
pub mod geometry;

use convert::{FieldMapping, HeaderConfig, ShpToTxtOptions, TxtToShpOptions};

// ─── IPC Types ───

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShpImportResult {
    files: Vec<ShpFileItem>,
    dir: String,
    /// 被拒收的非面状 SHP 文件名（前端用于 toast 提示）
    skipped: Vec<String>,
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
    /// 被过滤掉的非面状图层名（前端用于 toast 提示）
    skipped: Vec<String>,
    /// 从坐标反推的带号（探测失败为 None，前端据此决定是否回填）
    zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GdbLayerItem {
    name: String,
    field_names: Vec<String>,
    num_features: usize,
    geometry_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxtImportResult {
    files: Vec<TxtFileItem>,
    /// 解析失败的文件名（前端用于 toast 提示）
    #[serde(default)]
    failed: Vec<String>,
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

// ─── Commands ───

/// 判断几何类型字符串是否为面状（Polygon / MultiPolygon / 面）
fn is_polygon_geometry_type(t: &str) -> bool {
    let s = t.to_lowercase();
    s.contains("polygon") || s.contains("面") || s == "multipolygon"
}

/// 从东坐标采样反推高斯投影带号。中国高斯投影东坐标自带带号前缀
/// （如 38500000 → 38 度带），与 SHP 从 .prj 中央经线反推口径一致。
/// 无前缀数据（easting < 1e6）或空采样 → None，由用户手填 + 校验兜底。
fn derive_zone_from_eastings(eastings: &[f64]) -> Option<String> {
    let mut counts: HashMap<i32, usize> = HashMap::new();
    for &e in eastings {
        let zone = (e / 1_000_000.0).floor() as i32;
        if (1..=60).contains(&zone) {
            *counts.entry(zone).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(z, _)| z.to_string())
}

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
                skipped: vec![],
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
    let mut skipped = Vec::new();
    for file in &picked {
        let shp_path = match file.as_path() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        if shp_path.extension().map(|e| e != "shp").unwrap_or(true) {
            continue;
        }
        match shp::read_shp_file_group(&shp_path) {
            Ok(info) => {
                // 仅接收面状 SHP；非面状（点/线等）拒收并记录
                if is_polygon_geometry_type(&info.shape_type) {
                    items.push(ShpFileItem {
                        shp_path: info.shp_path,
                        dbf_path: info.dbf_path,
                        prj_path: info.prj_path,
                        name: info.name,
                        field_names: info.field_names,
                        num_features: info.num_features,
                        shape_type: info.shape_type,
                        prj_text: info.prj_text,
                        crs_info: info.crs_info,
                    });
                } else {
                    skipped.push(format!(
                        "{}.shp（{}）",
                        info.name, info.shape_type
                    ));
                    eprintln!("拒收非面状 SHP: {} ({})", info.name, info.shape_type);
                }
            }
            Err(e) => eprintln!("读 SHP 失败: {}", e),
        }
    }

    Ok(ShpImportResult {
        files: items,
        dir: base_dir,
        skipped,
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

    // 按几何类型过滤：仅保留面状图层，非面状（点/线/注记等）跳过
    let mut skipped = Vec::new();
    let mut filtered_layers: Vec<gdb::GdbLayerInfo> = Vec::new();
    let mut all_field_names: Vec<Vec<String>> = Vec::new();
    let mut all_features: Vec<Vec<gdb::GdbFeature>> = Vec::new();
    for (li, layer) in info.layers.iter().enumerate() {
        if is_polygon_geometry_type(&layer.geometry_type) {
            filtered_layers.push(layer.clone());
            all_field_names.push(info.all_field_names[li].clone());
            all_features.push(info.all_features[li].clone());
        } else {
            skipped.push(format!("{}（{}）", layer.name, layer.geometry_type));
            eprintln!(
                "过滤非面状 GDB 图层: {} ({})",
                layer.name, layer.geometry_type
            );
        }
    }

    // 仅当面状图层全部被过滤掉时才报错
    if filtered_layers.is_empty() {
        return Err(format!(
            "该 GDB 没有面状要素类（共 {} 个图层均为非面状），无法导入",
            info.layers.len()
        ));
    }

    let field_names = all_field_names.first().cloned().unwrap_or_default();
    let num_features: usize = filtered_layers.iter().map(|l| l.num_features).sum();
    let layers = filtered_layers
        .iter()
        .map(|l| GdbLayerItem {
            name: l.name.clone(),
            field_names: l.field_names.clone(),
            num_features: l.num_features,
            geometry_type: l.geometry_type.clone(),
        })
        .collect();

    Ok(GdbImportResult {
        path: gdb_path.to_string_lossy().to_string(),
        name,
        layers,
        field_names,
        num_features,
        skipped,
        zone: {
            // 采样东坐标反推带号（与 SHP 从 .prj 中央经线反推对齐）
            let eastings: Vec<f64> = all_features
                .iter()
                .flat_map(|feats| feats.iter())
                .flat_map(|f| f.points.iter())
                .map(|(easting, _)| *easting)
                .collect();
            derive_zone_from_eastings(&eastings)
        },
    })
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
        None => return Ok(TxtImportResult { files: vec![], failed: vec![] }),
    };

    let mut items = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    for file in &picked {
        let path = match file.as_path() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };

        let text = match txt::read_text_file(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("读 TXT 失败: {}", e);
                failed.push(
                    path.file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                );
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

    Ok(TxtImportResult { files: items, failed })
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
    if header_cfg.attr("带号").trim().is_empty() {
        return Err("带号不能为空，请填写带号后再输出".to_string());
    }
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
    let mut skipped = Vec::new();
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
            Ok(info) => {
                if is_polygon_geometry_type(&info.shape_type) {
                    items.push(ShpFileItem {
                        shp_path: info.shp_path,
                        dbf_path: info.dbf_path,
                        prj_path: info.prj_path,
                        name: info.name,
                        field_names: info.field_names,
                        num_features: info.num_features,
                        shape_type: info.shape_type,
                        prj_text: info.prj_text,
                        crs_info: info.crs_info,
                    });
                } else {
                    skipped.push(format!("{}.shp（{}）", info.name, info.shape_type));
                    eprintln!("拒收非面状 SHP: {} ({})", info.name, info.shape_type);
                }
            }
            Err(e) => eprintln!("拖放读 SHP 失败: {}", e),
        }
    }
    Ok(ShpImportResult { files: items, dir: base_dir, skipped })
}

#[tauri::command]
fn pick_txt_files_from_paths(paths: Vec<String>) -> Result<TxtImportResult, String> {
    let mut items = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    for p in &paths {
        let path = PathBuf::from(p);
        let text = match txt::read_text_file(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("拖放读 TXT 失败: {}", e);
                failed.push(path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
                continue;
            }
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
    Ok(TxtImportResult { files: items, failed })
}

#[tauri::command]
fn read_txt_preview(path: String) -> Result<String, String> {
    let text = txt::read_text_file(std::path::Path::new(&path))?;
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
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            pick_shp_files,
            import_gdb,
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

#[cfg(test)]
mod tests {
    use super::derive_zone_from_eastings;

    #[test]
    fn derive_zone_from_eastings_with_prefix() {
        assert_eq!(
            derive_zone_from_eastings(&[38_383_243.0, 38_500_000.0]),
            Some("38".to_string())
        );
    }

    #[test]
    fn derive_zone_from_eastings_without_prefix() {
        assert_eq!(derive_zone_from_eastings(&[500_000.0]), None);
    }

    #[test]
    fn derive_zone_from_eastings_empty() {
        assert_eq!(derive_zone_from_eastings(&[]), None);
    }
}
