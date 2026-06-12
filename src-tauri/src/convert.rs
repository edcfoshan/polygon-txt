use crate::gdb;
use crate::gpkg;
use crate::shp;
use crate::txt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    pub name: String,
    pub id: String,
    pub area: String,
    pub use_field: String,
    pub tfh: String,
    pub dlbm: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShpToTxtOptions {
    pub ox: bool,
    pub oj: bool,
    pub op: bool,
    pub on: bool,
    pub oo: bool,
    pub om: bool,
    pub buffer: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxtToShpOptions {
    pub output_shp: bool,
    pub output_gpkg: bool,
    pub merge: bool,
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertResult {
    pub success: bool,
    pub message: String,
    pub output_files: Vec<String>,
    pub processed_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShpSourceInfo {
    pub file_type: String,
    pub shp_paths: Vec<String>,
    pub source_path: Option<String>,
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

pub fn read_shp_source(shp_paths: &[PathBuf]) -> Result<ShpSourceInfo, String> {
    if shp_paths.is_empty() {
        return Err("没有选择 SHP 文件".to_string());
    }

    let first = shp::read_shp_file_group(&shp_paths[0])?;

    Ok(ShpSourceInfo {
        file_type: "shp".to_string(),
        shp_paths: shp_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        source_path: None,
        field_names: first.field_names.clone(),
        field_records: first.field_records.clone(),
        num_features: first.num_features,
        crs_info: first.crs_info.clone(),
        layers: vec![],
    })
}

pub fn read_gdb_source(gdb_path: &Path) -> Result<ShpSourceInfo, String> {
    let info = gdb::read_gdb(gdb_path)?;

    let layers = info
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
        source_path: Some(gdb_path.to_string_lossy().to_string()),
        field_names: info.all_field_names.first().cloned().unwrap_or_default(),
        field_records: vec![],
        num_features: info.layers.first().map(|l| l.num_features).unwrap_or(0),
        crs_info: HashMap::new(),
        layers,
    })
}

pub fn read_gpkg_source(gpkg_path: &Path) -> Result<ShpSourceInfo, String> {
    let info = gpkg::read_gpkg(gpkg_path)?;

    let layers = info
        .layers
        .iter()
        .map(|l| GdbLayerItem {
            name: l.name.clone(),
            field_names: l.field_names.clone(),
            num_features: l.num_features,
        })
        .collect();

    Ok(ShpSourceInfo {
        file_type: "gpkg".to_string(),
        shp_paths: vec![],
        source_path: Some(gpkg_path.to_string_lossy().to_string()),
        field_names: info.all_field_names.first().cloned().unwrap_or_default(),
        field_records: vec![],
        num_features: info.layers.first().map(|l| l.num_features).unwrap_or(0),
        crs_info: HashMap::new(),
        layers,
    })
}

pub fn shp_to_txt_preview(
    shp_paths: &[PathBuf],
    source_type: Option<&str>,
    source_path: Option<&PathBuf>,
    header_cfg: &HeaderConfig,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    selected_layers: Option<&[String]>,
) -> Result<String, String> {
    let result = match source_type {
        Some("gdb") => {
            let path = source_path.ok_or_else(|| "缺少 GDB 路径".to_string())?;
            let info = gdb::read_gdb(path)?;
            let plots =
                gdb_features_to_plots(&info, field_mapping, options, selected_layers)?;
            txt::generate_txt(
                &header_cfg.project_info,
                &make_header_attrs(header_cfg),
                &plots,
                options.oj,
            )
        }
        Some("gpkg") => {
            let path = source_path.ok_or_else(|| "缺少 GPKG 路径".to_string())?;
            let info = gpkg::read_gpkg(path)?;
            let plots = gpkg_features_to_plots(&info, field_mapping, options);
            txt::generate_txt(
                &header_cfg.project_info,
                &make_header_attrs(header_cfg),
                &plots,
                options.oj,
            )
        }
        _ => shp_files_to_txt_preview(shp_paths, header_cfg, field_mapping, options)?,
    };

    Ok(result.lines().take(200).collect::<Vec<_>>().join("\n"))
}

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

pub fn convert_shp_to_txt(
    shp_paths: &[PathBuf],
    source_type: Option<&str>,
    source_path: Option<&PathBuf>,
    header_cfg: &HeaderConfig,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    output_dir: &Path,
    selected_layers: Option<&[String]>,
) -> Result<ConvertResult, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("创建输出目录失败: {}", e))?;
    let mut output_files = Vec::new();

    match source_type {
        Some("gdb") => {
            let path = source_path.ok_or_else(|| "缺少 GDB 路径".to_string())?;
            let info = gdb::read_gdb(path)?;
            let plots =
                gdb_features_to_plots(&info, field_mapping, options, selected_layers)?;
            let txt_content = txt::generate_txt(
                &header_cfg.project_info,
                &make_header_attrs(header_cfg),
                &plots,
                options.oj,
            );
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "output".to_string());
            let txt_path = output_dir.join(format!("{}.txt", stem));
            std::fs::write(&txt_path, &txt_content)
                .map_err(|e| format!("写 TXT 失败: {}", e))?;
            output_files.push(txt_path.to_string_lossy().to_string());
        }
        Some("gpkg") => {
            let path = source_path.ok_or_else(|| "缺少 GPKG 路径".to_string())?;
            let info = gpkg::read_gpkg(path)?;
            let plots = gpkg_features_to_plots(&info, field_mapping, options);
            let txt_content = txt::generate_txt(
                &header_cfg.project_info,
                &make_header_attrs(header_cfg),
                &plots,
                options.oj,
            );
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "output".to_string());
            let txt_path = output_dir.join(format!("{}.txt", stem));
            std::fs::write(&txt_path, &txt_content)
                .map_err(|e| format!("写 TXT 失败: {}", e))?;
            output_files.push(txt_path.to_string_lossy().to_string());
        }
        _ if options.om => {
            let mut all_plots = Vec::new();
            for shp_path in shp_paths {
                all_plots.extend(single_shp_to_plots(shp_path, field_mapping, options)?);
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
        }
        _ => {
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
    }

    let count = output_files.len();
    Ok(ConvertResult {
        success: true,
        message: format!("成功转换 {} 个文件", count),
        output_files,
        processed_count: count,
    })
}

pub fn convert_txt_to_shp(
    txt_paths: &[PathBuf],
    options: &TxtToShpOptions,
    header_cfg: &HeaderConfig,
) -> Result<ConvertResult, String> {
    let output_dir = Path::new(&options.output_dir);
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("创建输出目录失败: {}", e))?;
    let mut output_files = Vec::new();

    for txt_path in txt_paths {
        let text =
            std::fs::read_to_string(txt_path).map_err(|e| format!("读取 TXT 失败: {}", e))?;
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

        if options.output_gpkg {
            let (geometries, attributes) = plots_to_shp_data(&parsed.plots);
            let fields: Vec<(String, String, u8, u32)> = vec![
                ("DKMC".into(), "地块名称".into(), 4u8, 50u32),
                ("DKBH".into(), "地块编号".into(), 4u8, 30u32),
                ("MJ".into(), "面积".into(), 3u8, 14u32),
                ("DKYT".into(), "用途".into(), 4u8, 50u32),
                ("TFH".into(), "图幅号".into(), 4u8, 20u32),
                ("DLBM".into(), "地类编码".into(), 4u8, 10u32),
            ];

            let mut crs_info = HashMap::new();
            crs_info.insert("c".to_string(), header_cfg.crs.clone());
            crs_info.insert("b".to_string(), header_cfg.band.clone());
            crs_info.insert("z".to_string(), header_cfg.zone.clone());

            let gpkg_files = gpkg::write_gpkg_output(
                output_dir,
                &stem,
                &fields,
                &attributes,
                &geometries,
                &crs_info,
            )?;
            output_files.extend(gpkg_files);
        }
    }

    let count = txt_paths.len();
    Ok(ConvertResult {
        success: true,
        message: format!("成功转换 {} 个 TXT 文件", count),
        output_files,
        processed_count: count,
    })
}

fn shp_files_to_plots(
    shp_paths: &[PathBuf],
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
) -> Result<Vec<txt::PlotData>, String> {
    let mut all_plots = Vec::new();
    for shp_path in shp_paths {
        all_plots.extend(single_shp_to_plots(shp_path, field_mapping, options)?);
    }
    Ok(all_plots)
}

fn single_shp_to_plots(
    shp_path: &PathBuf,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
) -> Result<Vec<txt::PlotData>, String> {
    let info = shp::read_shp_file_group(shp_path)?;
    let features = shp::read_shp(shp_path)?;

    let mut plots = Vec::new();
    for (fi, feat) in features.iter().enumerate() {
        let record = info.field_records.get(fi).cloned().unwrap_or_default();
        let plot_name = get_field_value(&field_mapping.name, &info.field_names, &record);
        let plot_area = get_field_value(&field_mapping.area, &info.field_names, &record);
        let plot_use = get_field_value(&field_mapping.use_field, &info.field_names, &record);
        let plot_tfh = get_field_value(&field_mapping.tfh, &info.field_names, &record);
        let plot_dlbm = get_field_value(&field_mapping.dlbm, &info.field_names, &record);

        plots.push(build_plot_data(
            &feat.points,
            plot_name,
            plot_area,
            plot_use,
            plot_tfh,
            plot_dlbm,
            options,
        ));
    }

    Ok(plots)
}

fn gdb_features_to_plots(
    info: &gdb::GdbFileInfo,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    selected_layers: Option<&[String]>,
) -> Result<Vec<txt::PlotData>, String> {
    let mut all_plots = Vec::new();

    for (layer_idx, features) in info.all_features.iter().enumerate() {
        let layer_name = info.layers.get(layer_idx).map(|l| l.name.as_str()).unwrap_or("");
        if let Some(sel) = selected_layers {
            if !sel.iter().any(|n| n == layer_name) {
                continue;
            }
        }

        for feat in features {
            all_plots.push(build_plot_data(
                &feat.points,
                get_field_value_map(&field_mapping.name, &feat.attributes).to_string(),
                get_field_value_map(&field_mapping.area, &feat.attributes).to_string(),
                get_field_value_map(&field_mapping.use_field, &feat.attributes).to_string(),
                get_field_value_map(&field_mapping.tfh, &feat.attributes).to_string(),
                get_field_value_map(&field_mapping.dlbm, &feat.attributes).to_string(),
                options,
            ));
        }
    }

    Ok(all_plots)
}

fn gpkg_features_to_plots(
    info: &gpkg::GpkgFileInfo,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
) -> Vec<txt::PlotData> {
    let mut all_plots = Vec::new();

    for features in &info.all_features {
        for feat in features {
            all_plots.push(build_plot_data(
                &feat.points,
                get_field_value_map(&field_mapping.name, &feat.attributes).to_string(),
                get_field_value_map(&field_mapping.area, &feat.attributes).to_string(),
                get_field_value_map(&field_mapping.use_field, &feat.attributes).to_string(),
                get_field_value_map(&field_mapping.tfh, &feat.attributes).to_string(),
                get_field_value_map(&field_mapping.dlbm, &feat.attributes).to_string(),
                options,
            ));
        }
    }

    all_plots
}

fn build_plot_data(
    points: &[(f64, f64)],
    plot_name: String,
    plot_area: String,
    plot_use: String,
    plot_tfh: String,
    plot_dlbm: String,
    options: &ShpToTxtOptions,
) -> txt::PlotData {
    let mut coords: Vec<(f64, f64)> = points
        .iter()
        .map(|&(x, y)| if options.ox { (x, y) } else { (y, x) })
        .collect();

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

    if options.oo && coords.len() >= 2 {
        let first = coords[0];
        let last = coords[coords.len() - 1];
        if (first.0 - last.0).abs() > 1e-9 || (first.1 - last.1).abs() > 1e-9 {
            coords.push(first);
        }
    }

    txt::PlotData {
        point_count: coords.len() as u32,
        area: plot_area,
        fid: "FID_0".to_string(),
        name: plot_name,
        geom_type: "面".to_string(),
        tfh: plot_tfh,
        use_field: plot_use,
        dlbm: plot_dlbm,
        coords,
    }
}

fn plots_to_shp_data(
    plots: &[txt::PlotData],
) -> (Vec<Vec<(f64, f64)>>, Vec<HashMap<String, String>>) {
    let mut geometries = Vec::new();
    let mut attributes = Vec::new();

    for plot in plots {
        let coords: Vec<(f64, f64)> = plot.coords.iter().map(|&(y, x)| (x, y)).collect();
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
    }

    (geometries, attributes)
}

fn get_field_value(field_name: &str, field_names: &[String], record: &[String]) -> String {
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

fn get_field_value_map<'a>(field_name: &str, attrs: &'a HashMap<String, String>) -> &'a str {
    if field_name.is_empty() {
        return "";
    }
    attrs.get(field_name).map(|s| s.as_str()).unwrap_or("")
}

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
