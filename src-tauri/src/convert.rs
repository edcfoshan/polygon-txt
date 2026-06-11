// 转换编排模块 — 面↔TXT 的主逻辑（纯 Rust，无 GDAL）
use crate::gdb;
use crate::shp;
use crate::txt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 前端传来的字段映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    pub name: String,       // DKMC
    pub id: String,         // DKBH
    pub area: String,       // MJ
    pub use_field: String,  // DKYT
    pub tfh: String,        // TFH
    pub dlbm: String,       // DLBM
}

/// 前端传来的表头配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderConfig {
    pub crs: String,
    pub band: String,
    pub proj: String,
    pub unit: String,
    pub zone: String,
    pub precision: String,
    pub transform: String,
    pub project_info: String,
}

/// SHP→TXT 选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShpToTxtOptions {
    pub ox: bool,     // 坐标互换
    pub oj: bool,     // J 编号
    pub op: bool,     // 部件号从 1 开始
    pub on: bool,     // 起始点西北角
    pub oo: bool,     // 首末点重合
    pub om: bool,     // 合并到一个 TXT
    pub buffer: f64,  // 缓冲区（米），预留
}

/// TXT→SHP/GDB 选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxtToShpOptions {
    pub output_shp: bool,
    pub output_gdb: bool,
    pub merge: bool,
    pub output_dir: String,
}

/// 转换结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertResult {
    pub success: bool,
    pub message: String,
    pub output_files: Vec<String>,
    pub processed_count: usize,
}

// ─── SHP/PRJ 信息（前端预览用）───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShpSourceInfo {
    pub file_type: String, // "shp" or "gdb"
    pub shp_paths: Vec<String>,
    pub gdb_path: Option<String>,
    pub field_names: Vec<String>,
    pub field_records: Vec<Vec<String>>,
    pub num_features: usize,
    pub crs_info: HashMap<String, String>,
    pub layers: Vec<GdbLayerItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdbLayerItem {
    pub name: String,
    pub field_names: Vec<String>,
    pub num_features: usize,
}

// ─── 核心转换函数 ───

/// 读取 SHP 文件组，返回前端预览/字段映射用信息
pub fn read_shp_source(shp_paths: &[PathBuf]) -> Result<ShpSourceInfo, String> {
    if shp_paths.is_empty() {
        return Err("没有选择 SHP 文件".to_string());
    }

    let first = shp::read_shp_file_group(&shp_paths[0])?;
    // 汇总所有文件的字段名（用第一个文件的）
    let field_names = first.field_names.clone();
    let field_records = first.field_records.clone();
    let crs_info = first.crs_info.clone();

    Ok(ShpSourceInfo {
        file_type: "shp".to_string(),
        shp_paths: shp_paths.iter().map(|p| p.to_string_lossy().to_string()).collect(),
        gdb_path: None,
        field_names,
        field_records,
        num_features: first.num_features,
        crs_info,
        layers: vec![],
    })
}

/// 读取 GDB，返回前端预览用信息
pub fn read_gdb_source(gdb_path: &Path) -> Result<ShpSourceInfo, String> {
    let info = gdb::read_gdb(gdb_path)?;

    // 取第一个要素类的字段名
    let field_names = info
        .all_field_names
        .first()
        .cloned()
        .unwrap_or_default();
    let crs_info = HashMap::new(); // geonative 目前没有暴露 CRS 信息

    let layers: Vec<GdbLayerItem> = info
        .layers
        .iter()
        .map(|l| GdbLayerItem {
            name: l.name.clone(),
            field_names: l.field_names.clone(),
            num_features: l.num_features,
        })
        .collect();

    Ok(ShpSourceInfo {
        file_type: "gdb".to_string(),
        shp_paths: vec![],
        gdb_path: Some(gdb_path.to_string_lossy().to_string()),
        field_names,
        field_records: vec![], // GDB 的预览不返回具体记录，只返回字段列表
        num_features: info.layers.first().map(|l| l.num_features).unwrap_or(0),
        crs_info,
        layers,
    })
}

/// SHP→TXT 生成预览
pub fn shp_to_txt_preview(
    shp_paths: &[PathBuf],
    gdb_path: Option<&PathBuf>,
    header_cfg: &HeaderConfig,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
) -> Result<String, String> {
    if let Some(gdb) = gdb_path {
        let info = gdb::read_gdb(gdb)?;
        let plots = gdb_features_to_plots(&info, field_mapping, options, header_cfg)?;
        let result = txt::generate_txt(
            &header_cfg.project_info,
            &make_header_attrs(header_cfg),
            &plots,
            options.oj,
        );
        // 只截取前 200 行作为预览
        let trimmed: Vec<&str> = result.lines().take(200).collect();
        Ok(trimmed.join("\n"))
    } else {
        let txt = shp_files_to_txt_preview(shp_paths, header_cfg, field_mapping, options)?;
        let trimmed: Vec<&str> = txt.lines().take(200).collect();
        Ok(trimmed.join("\n"))
    }
}

/// SHP 文件 → TXT 预览文本
fn shp_files_to_txt_preview(
    shp_paths: &[PathBuf],
    header_cfg: &HeaderConfig,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
) -> Result<String, String> {
    let plots = shp_files_to_plots(shp_paths, field_mapping, options)?;
    Ok(txt::generate_txt(
        &header_cfg.project_info,
        &make_header_attrs(header_cfg),
        &plots,
        options.oj,
    ))
}

/// 执行 SHP→TXT 转换
pub fn convert_shp_to_txt(
    shp_paths: &[PathBuf],
    gdb_path: Option<&PathBuf>,
    header_cfg: &HeaderConfig,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    output_dir: &Path,
) -> Result<ConvertResult, String> {
    let mut output_files = Vec::new();

    if let Some(gdb) = gdb_path {
        let info = gdb::read_gdb(gdb)?;
        let plots = gdb_features_to_plots(&info, field_mapping, options, header_cfg)?;
        let txt_content = txt::generate_txt(
            &header_cfg.project_info,
            &make_header_attrs(header_cfg),
            &plots,
            options.oj,
        );
        let stem = gdb
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        let txt_path = output_dir.join(format!("{}.txt", stem));
        std::fs::write(&txt_path, &txt_content)
            .map_err(|e| format!("写 TXT 失败: {}", e))?;
        output_files.push(txt_path.to_string_lossy().to_string());
    } else if options.om {
        // om: 合并到一个 TXT — 收集所有 SHP 的 plots 输出单个文件
        let mut all_plots = Vec::new();
        for shp_path in shp_paths {
            let plots = single_shp_to_plots(shp_path, field_mapping, options)?;
            all_plots.extend(plots);
        }
        let txt_content = txt::generate_txt(
            &header_cfg.project_info,
            &make_header_attrs(header_cfg),
            &all_plots,
            options.oj,
        );
        let txt_path = output_dir.join("merged_output.txt");
        std::fs::write(&txt_path, &txt_content)
            .map_err(|e| format!("写 TXT 失败: {}", e))?;
        output_files.push(txt_path.to_string_lossy().to_string());
    } else {
        for shp_path in shp_paths {
            let plots = single_shp_to_plots(shp_path, field_mapping, options)?;
            let txt_content = txt::generate_txt(
                &header_cfg.project_info,
                &make_header_attrs(header_cfg),
                &plots,
                options.oj,
            );
            let stem = shp_path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "output".to_string());
            let txt_path = output_dir.join(format!("{}.txt", stem));
            std::fs::write(&txt_path, &txt_content)
                .map_err(|e| format!("写 TXT 失败: {}", e))?;
            output_files.push(txt_path.to_string_lossy().to_string());
        }
    }

    let count = output_files.len();
    Ok(ConvertResult {
        success: true,
        message: format!("成功转换 {} 个文件", count),
        output_files,
        processed_count: count,
        })
}

/// TXT→SHP/GDB 转换
pub fn convert_txt_to_shp(
    txt_paths: &[PathBuf],
    options: &TxtToShpOptions,
    header_cfg: &HeaderConfig,
) -> Result<ConvertResult, String> {
    let output_dir = Path::new(&options.output_dir);
    let mut output_files = Vec::new();

    for txt_path in txt_paths {
        let text = std::fs::read_to_string(txt_path)
            .map_err(|e| format!("读取 TXT 失败: {}", e))?;
        let parsed = txt::parse_txt(&text);
        let stem = txt_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());

        if options.output_shp {
            let out_dir = output_dir.join(&stem);
            std::fs::create_dir_all(&out_dir)
                .map_err(|e| format!("创建目录失败: {}", e))?;

            let (geometries, attributes) = plots_to_shp_data(&parsed.plots);
            let shp_files = shp::write_shapefile(
                &out_dir,
                &stem,
                &geometries,
                &attributes,
                &header_cfg.crs,
                &header_cfg.band,
                &header_cfg.zone,
            )?;
            output_files.extend(shp_files);
        }

        if options.output_gdb {
            let (geometries, attributes) = plots_to_shp_data(&parsed.plots);

            // 字段定义
            let fields: Vec<(String, String, u8, u32)> = vec![
                ("DKMC".into(), "地块名称".into(), 4u8, 50u32),
                ("DKBH".into(), "地块编号".into(), 4u8, 30u32),
                ("MJ".into(), "面积".into(), 3u8, 14u32),      // Float64
                ("DKYT".into(), "用途".into(), 4u8, 50u32),
                ("TFH".into(), "图幅号".into(), 4u8, 20u32),
                ("DLBM".into(), "地类编码".into(), 4u8, 10u32),
            ];

            let mut crs_info = HashMap::new();
            crs_info.insert("c".to_string(), header_cfg.crs.clone());
            crs_info.insert("b".to_string(), header_cfg.band.clone());
            crs_info.insert("z".to_string(), header_cfg.zone.clone());

            let gdb_files = gdb::write_gdb_output(
                output_dir,
                &stem,
                &fields,
                &attributes,
                &geometries,
                &crs_info,
            )?;
            output_files.extend(gdb_files);
        }
    }

    let count = txt_paths.len();
    let message = format!("成功转换 {} 个 TXT 文件", count);
    Ok(ConvertResult {
        success: true,
        message,
        output_files,
        processed_count: count,
    })
}

// ─── 内部辅助函数 ───

/// SHP 文件 → 地块列表
fn shp_files_to_plots(
    shp_paths: &[PathBuf],
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
) -> Result<Vec<txt::PlotData>, String> {
    let mut all_plots = Vec::new();
    for shp_path in shp_paths {
        let plots = single_shp_to_plots(shp_path, field_mapping, options)?;
        all_plots.extend(plots);
    }
    Ok(all_plots)
}

/// 单个 SHP 文件 → 地块列表
fn single_shp_to_plots(
    shp_path: &PathBuf,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
) -> Result<Vec<txt::PlotData>, String> {
    let info = shp::read_shp_file_group(shp_path)?;
    let features = shp::read_shp(shp_path)?;
    // TODO: 缓冲区处理 — 当 options.buffer > 0 时，对多边形进行膨胀/收缩
    // 需要引入计算几何库（如 geo crate），后续版本实现

    let mut plots = Vec::new();
    for (fi, feat) in features.iter().enumerate() {
        let record = info.field_records.get(fi).cloned().unwrap_or_default();

        let plot_name = get_field_value(&field_mapping.name, &info.field_names, &record);
        let plot_area = get_field_value(&field_mapping.area, &info.field_names, &record);
        let plot_use = get_field_value(&field_mapping.use_field, &info.field_names, &record);
        let plot_tfh = get_field_value(&field_mapping.tfh, &info.field_names, &record);
        let plot_dlbm = get_field_value(&field_mapping.dlbm, &info.field_names, &record);

        let mut coords: Vec<(f64, f64)> = feat
            .points
            .iter()
            .map(|&(x, y)| {
                if options.ox {
                    (x, y) // swapped
                } else {
                    (y, x) // default: TXT format (northing, easting)
                }
            })
            .collect();

        // on: 起始点西北角 — 找到 Y 最大（最北）且 X 最小（最西）的点，旋转使其成为起点
        if options.on && coords.len() > 2 {
            let mut best_idx = 0;
            let mut best_y = f64::NEG_INFINITY;
            let mut best_x = f64::INFINITY;
            for (i, &(y, x)) in coords.iter().enumerate() {
                // 优先选 Y 最大（最北），同等 Y 时选 X 最小（最西）
                if y > best_y || (y == best_y && x < best_x) {
                    best_y = y;
                    best_x = x;
                    best_idx = i;
                }
            }
            if best_idx > 0 {
                coords.rotate_left(best_idx);
            }
        }

        // oo: 首末点重合 — 确保多边形首尾坐标相同
        if options.oo && coords.len() >= 2 {
            let first = coords[0];
            let last = coords[coords.len() - 1];
            if (first.0 - last.0).abs() > 1e-9 || (first.1 - last.1).abs() > 1e-9 {
                coords.push(first);
            }
        }

        plots.push(txt::PlotData {
            point_count: coords.len() as u32,
            area: plot_area,
            fid: "FID_0".to_string(),
            name: plot_name,
            geom_type: "面".to_string(),
            tfh: plot_tfh,
            use_field: plot_use,
            dlbm: plot_dlbm,
            coords,
        });
    }

    Ok(plots)
}

/// GDB 要素 → 地块列表
fn gdb_features_to_plots(
    info: &gdb::GdbFileInfo,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    _header_cfg: &HeaderConfig,
) -> Result<Vec<txt::PlotData>, String> {
    let mut all_plots = Vec::new();

    for (layer_idx, features) in info.all_features.iter().enumerate() {
        let _field_names = info
            .all_field_names
            .get(layer_idx)
            .cloned()
            .unwrap_or_default();

        for feat in features {
            let plot_name = get_field_value_map(&field_mapping.name, &feat.attributes).to_string();
            let plot_area = get_field_value_map(&field_mapping.area, &feat.attributes).to_string();
            let plot_use = get_field_value_map(&field_mapping.use_field, &feat.attributes).to_string();
            let plot_tfh = get_field_value_map(&field_mapping.tfh, &feat.attributes).to_string();
            let plot_dlbm = get_field_value_map(&field_mapping.dlbm, &feat.attributes).to_string();

            let mut coords: Vec<(f64, f64)> = feat
                .points
                .iter()
                .map(|&(x, y)| {
                    if options.ox {
                        (x, y)
                    } else {
                        (y, x)
                    }
                })
                .collect();

            // on: 起始点西北角
            if options.on && coords.len() > 2 {
                let mut best_idx = 0;
                let mut best_y = f64::NEG_INFINITY;
                let mut best_x = f64::INFINITY;
                for (i, &(y, x)) in coords.iter().enumerate() {
                    if y > best_y || (y == best_y && x < best_x) {
                        best_y = y;
                        best_x = x;
                        best_idx = i;
                    }
                }
                if best_idx > 0 {
                    coords.rotate_left(best_idx);
                }
            }

            // oo: 首末点重合
            if options.oo && coords.len() >= 2 {
                let first = coords[0];
                let last = coords[coords.len() - 1];
                if (first.0 - last.0).abs() > 1e-9 || (first.1 - last.1).abs() > 1e-9 {
                    coords.push(first);
                }
            }

            all_plots.push(txt::PlotData {
                point_count: coords.len() as u32,
                area: plot_area,
                fid: "FID_0".to_string(),
                name: plot_name,
                geom_type: "面".to_string(),
                tfh: plot_tfh,
                use_field: plot_use,
                dlbm: plot_dlbm,
                coords,
            });
        }
    }

    Ok(all_plots)
}

/// 地块列表 → SHP 几何和属性
fn plots_to_shp_data(
    plots: &[txt::PlotData],
) -> (Vec<Vec<(f64, f64)>>, Vec<HashMap<String, String>>) {
    let mut geometries = Vec::new();
    let mut attributes = Vec::new();

    for plot in plots {
        // TXT: (y, x) → SHP: (x, y)
        let coords: Vec<(f64, f64)> =
            plot.coords.iter().map(|&(y, x)| (x, y)).collect();

        if coords.len() >= 3 {
            geometries.push(coords);

            let mut attr = HashMap::new();
            attr.insert("DKMC".to_string(), plot.name.clone());
            attr.insert("DKBH".to_string(), String::new());
            attr.insert("MJ".to_string(), plot.area.clone());
            attr.insert("DKYT".to_string(), plot.use_field.clone());
            attr.insert("TFH".to_string(), plot.tfh.clone());
            attr.insert("DLBM".to_string(), plot.dlbm.clone());
            attributes.push(attr);
        }
        // 坐标点 < 3 的地块直接跳过（既不 push geometry 也不 push attribute）
    }

    (geometries, attributes)
}

/// 从哈希表取字段值（兼容 SHP 的 Vec 和 GDB 的 HashMap）
fn get_field_value(
    field_name: &str,
    field_names: &[String],
    record: &[String],
) -> String {
    if field_name.is_empty() {
        return String::new();
    }
    if let Some(pos) = field_names.iter().position(|n| n == field_name) {
        if pos < record.len() {
            return record[pos].clone();
        }
    }
    String::new()
}

/// 从 HashMap 取字段值（GDB 用）
fn get_field_value_map<'a>(
    field_name: &str,
    attrs: &'a HashMap<String, String>,
) -> &'a str {
    if field_name.is_empty() {
        return "";
    }
    attrs.get(field_name).map(|s| s.as_str()).unwrap_or("")
}

/// 表头配置 → 属性描述字典
fn make_header_attrs(cfg: &HeaderConfig) -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("坐标系".to_string(), cfg.crs.clone());
    m.insert("几度分带".to_string(), cfg.band.clone());
    m.insert("投影类型".to_string(), cfg.proj.clone());
    m.insert("计量单位".to_string(), cfg.unit.clone());
    m.insert("带号".to_string(), cfg.zone.clone());
    m.insert("精度".to_string(), cfg.precision.clone());
    m.insert("转换参数".to_string(), cfg.transform.clone());
    m
}
