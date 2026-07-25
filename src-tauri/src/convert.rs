use crate::gdb;
use crate::geometry::{indexed_rings_to_surface, surface_to_indexed_rings, SurfaceGeometry};
use crate::projection;
use crate::shp;
use crate::txt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// 判断几何类型字符串是否为面状（与 lib.rs::is_polygon_geometry_type 保持一致）
fn is_polygon_geometry_type(t: &str) -> bool {
    let s = t.to_lowercase();
    s.contains("polygon") || s.contains("面") || s == "multipolygon"
}

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
pub struct AttrRow {
    pub k: String,
    pub v: String,
}

/// 表头配置。[属性描述] 段由 `attrs` 有序列表驱动（原 7 个固定项作为默认种子，
/// 用户可增删改键名、调顺序）。`project_info` 对应独立的 [项目信息] 段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderConfig {
    pub attrs: Vec<AttrRow>,
    pub project_info: String,
}

impl HeaderConfig {
    /// 按键名查找属性行的值（trim 比对键名，找不到返回空串）。
    /// 供 TXT→SHP 流程读取"坐标系/几度分带/带号"等使用。
    pub fn attr(&self, key: &str) -> String {
        self.attrs
            .iter()
            .find(|r| r.k.trim() == key)
            .map(|r| r.v.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShpToTxtOptions {
    /// XY 坐标标反：勾选时输出 (X,Y) 顺序（东坐标在前）；默认 (Y,X) 北坐标在前（标准界址点格式）
    pub ox: bool,
    /// 点号前加 "J"
    pub oj: bool,
    /// 起始点西北角
    pub on: bool,
    /// 首末点重合
    pub oo: bool,
    /// 闭合点编号模式：false=回到环首点（默认），true=续编下一号
    #[serde(default)]
    pub oc: bool,
    /// 输出模式："one_to_one"（一对一）/ "split_by_plot"（按地块拆分）/ "merge_all"（全合并）
    #[serde(default)]
    pub output_mode: String,
    /// 模式 2 下的文件名字段：DKMC / DKBH / FID / 空(序号)
    #[serde(default)]
    pub filename_field: String,
    /// 输出公里网：大地坐标系（度）→ CGCS2000 高斯-克吕格平面坐标（米）。仅对经纬度输入生效。
    #[serde(default)]
    pub og: bool,
    /// 带类型：3 = 3 度带（默认），6 = 6 度带。仅 og 时有意义。
    #[serde(default = "default_zone_type")]
    pub zone_type: u8,
}

fn default_zone_type() -> u8 {
    3
}

// ─── og 公里网投影（仅大地坐标系输入生效） ───

/// 投影配置：og 勾选时由表头「带号」+ zone_type 解析得出，下传给各源函数。
#[derive(Debug, Clone, Copy)]
struct ProjectionConfig {
    zone: i32,
    zone_type: u8,
}

impl ProjectionConfig {
    /// og 未勾选 → None；勾选 → 从表头读带号（缺失则 zone=0，由源函数按需报错）。
    fn from_options(options: &ShpToTxtOptions, header_cfg: &HeaderConfig) -> Option<ProjectionConfig> {
        if !options.og {
            return None;
        }
        let zone = header_cfg.attr("带号").trim().parse::<i32>().unwrap_or(0);
        Some(ProjectionConfig {
            zone,
            zone_type: options.zone_type,
        })
    }
}

/// 判定是否为大地坐标系（度）。优先坐标采样（PRJ 误标时仍可靠），回退 crs_info 的单位声明。
fn sample_is_geodetic(sample: Option<(f64, f64)>, crs_info: &HashMap<String, String>) -> bool {
    if let Some((x, y)) = sample {
        x.abs() <= 360.0 && y.abs() <= 90.0
    } else {
        crs_info.get("u").map(|s| s == "度").unwrap_or(false)
    }
}

/// 对单个 surface 做高斯-克吕格投影并加带号前缀。仅在大地坐标系输入时调用。
/// 东坐标 = 带号×1,000,000 + 500km 假东偏 + 真东偏（投影本身已含 500km 假东偏）。
fn project_surface_with_prefix(
    surface: &SurfaceGeometry,
    cfg: &ProjectionConfig,
    ellipsoid: projection::Ellipsoid,
) -> Result<SurfaceGeometry, String> {
    if cfg.zone <= 0 {
        return Err("勾选了输出公里网但未填写带号".to_string());
    }
    let cm = if cfg.zone_type == 6 {
        cfg.zone as f64 * 6.0 - 3.0
    } else {
        cfg.zone as f64 * 3.0
    };
    let mut s = projection::project_surface(surface, cm, ellipsoid);
    let zone_f = cfg.zone as f64 * 1_000_000.0;
    for part in &mut s.parts {
        for (x, _) in &mut part.exterior {
            *x += zone_f;
        }
        for hole in &mut part.holes {
            for (x, _) in hole.iter_mut() {
                *x += zone_f;
            }
        }
    }
    Ok(s)
}

/// og 输出恒为米：克隆表头并把「计量单位」行改为「米」。
fn header_with_meter_unit(header_cfg: &HeaderConfig) -> HeaderConfig {
    let mut h = header_cfg.clone();
    for row in &mut h.attrs {
        if row.k.trim() == "计量单位" {
            row.v = "米".to_string();
            break;
        }
    }
    h
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxtToShpOptions {
    pub output_shp: bool,
    /// 输出模式：one_to_one / split_by_plot / merge_all
    #[serde(default)]
    pub output_mode: String,
    /// split_by_plot 模式下的文件名字段：DKMC / FID / ""(序号)
    #[serde(default)]
    pub filename_field: String,
    pub output_dir: String,
    /// 保留输出路径：勾选后 DBF 增加 LUJIN 列（源 TXT 完整路径）
    #[serde(default)]
    pub keep_lujin: bool,
    /// 保留 txt 名称：勾选后 DBF 增加 MINGC 列（源 TXT 文件名带 .txt）
    #[serde(default)]
    pub keep_mingc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertResult {
    pub success: bool,
    pub message: String,
    pub output_files: Vec<String>,
    pub processed_count: usize,
}

/// 单个地块 + 源信息（模式 2 按地块拆分用）
#[derive(Debug, Clone)]
struct PlotWithSource {
    plot: txt::PlotData,
    /// 源的 stem（shp 文件名 / GDB 文件夹名_图层名），用于建子目录
    #[allow(dead_code)]
    source_stem: String,
    /// 该地块在源内的序号（0-based）
    index_in_source: usize,
    /// 该地块的完整属性表（用于按字段取文件名）
    attributes: HashMap<String, String>,
}

/// 一个导入源：SHP 文件或 GDB 单个要素类
#[derive(Debug, Clone)]
struct ImportSource {
    /// 源的 stem，用于命名（SHP 文件名 / GDB 文件夹名_图层名）
    stem: String,
    /// 源内所有地块
    plots: Vec<PlotWithSource>,
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

pub fn shp_to_txt_preview(
    shp_paths: &[PathBuf],
    source_type: Option<&str>,
    source_path: Option<&PathBuf>,
    header_cfg: &HeaderConfig,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    selected_layers: Option<&[String]>,
) -> Result<String, String> {
    // og 输出恒为米：改写表头「计量单位」（仅 og 勾选时）
    let header_owned;
    let header_cfg: &HeaderConfig = if options.og {
        header_owned = header_with_meter_unit(header_cfg);
        &header_owned
    } else {
        header_cfg
    };
    let proj_cfg = ProjectionConfig::from_options(options, header_cfg);

    let result = match source_type {
        Some("gdb") => {
            let path = source_path.ok_or_else(|| "缺少 GDB 路径".to_string())?;
            let info = gdb::read_gdb(path)?;
            let plots =
                gdb_features_to_plots(&info, field_mapping, options, proj_cfg.as_ref(), selected_layers)?;
            txt::generate_txt(
                &header_cfg.project_info,
                &header_cfg.attrs,
                &plots,
                options.oj,
                options.oc,
            )
        }
        _ => shp_files_to_txt_preview(shp_paths, header_cfg, field_mapping, options, proj_cfg.as_ref())?,
    };

    Ok(result.lines().take(2000).collect::<Vec<_>>().join("\n"))
}

fn shp_files_to_txt_preview(
    shp_paths: &[PathBuf],
    header_cfg: &HeaderConfig,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    proj_cfg: Option<&ProjectionConfig>,
) -> Result<String, String> {
    let plots = shp_files_to_plots(shp_paths, field_mapping, options, proj_cfg)?;
    Ok(txt::generate_txt(
        &header_cfg.project_info,
        &header_cfg.attrs,
        &plots,
        options.oj,
        options.oc,
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

    // og 输出恒为米：改写表头「计量单位」（仅 og 勾选时）
    let header_owned;
    let header_cfg: &HeaderConfig = if options.og {
        header_owned = header_with_meter_unit(header_cfg);
        &header_owned
    } else {
        header_cfg
    };
    let proj_cfg = ProjectionConfig::from_options(options, header_cfg);

    // 统一展开成「导入源列表」
    let sources = collect_import_sources(
        shp_paths,
        source_type,
        source_path,
        field_mapping,
        options,
        proj_cfg.as_ref(),
        selected_layers,
    )?;

    match options.output_mode.as_str() {
        "split_by_plot" => convert_split_by_plot(sources, header_cfg, options, output_dir),
        "merge_all" => convert_merge_all(sources, header_cfg, options, output_dir),
        _ => convert_one_to_one(sources, header_cfg, options, output_dir),
    }
}

/// 把 SHP / GDB 统一展开成 ImportSource 列表
fn collect_import_sources(
    shp_paths: &[PathBuf],
    source_type: Option<&str>,
    source_path: Option<&PathBuf>,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    proj_cfg: Option<&ProjectionConfig>,
    selected_layers: Option<&[String]>,
) -> Result<Vec<ImportSource>, String> {
    let mut sources = Vec::new();
    match source_type {
        Some("gdb") => {
            let path = source_path.ok_or_else(|| "缺少 GDB 路径".to_string())?;
            let info = gdb::read_gdb(path)?;
            sources = gdb_to_sources(&info, field_mapping, options, proj_cfg, selected_layers)?;
        }
        _ => {
            for shp_path in shp_paths {
                sources.push(single_shp_to_source(shp_path, field_mapping, options, proj_cfg)?);
            }
        }
    }
    Ok(sources)
}

/// 模式 1：一对一。SHP→每个文件一个 TXT；GDB→每个要素类一个 TXT。
/// 跨源同名冲突自动追加 _2/_3。
fn convert_one_to_one(
    sources: Vec<ImportSource>,
    header_cfg: &HeaderConfig,
    options: &ShpToTxtOptions,
    output_dir: &Path,
) -> Result<ConvertResult, String> {
    let mut output_files = Vec::new();
    let mut used_names: HashMap<String, usize> = HashMap::new();
    let mut conflict_count = 0usize;

    for src in &sources {
        let plots: Vec<txt::PlotData> = src.plots.iter().map(|p| p.plot.clone()).collect();
        let txt_content = txt::generate_txt(
            &header_cfg.project_info,
            &header_cfg.attrs,
            &plots,
            options.oj,
            options.oc,
        );
        let (final_name, bumped) = allocate_unique_name(&src.stem, &mut used_names);
        if bumped {
            conflict_count += 1;
        }
        let txt_path = output_dir.join(format!("{}.txt", final_name));
        std::fs::write(&txt_path, &txt_content)
            .map_err(|e| format!("写 TXT 失败: {}", e))?;
        output_files.push(txt_path.to_string_lossy().to_string());
    }

    let count = output_files.len();
    let mut message = format!("成功转换 {} 个文件", count);
    if conflict_count > 0 {
        message.push_str(&format!("（{} 个文件名冲突已自动追加序号）", conflict_count));
    }
    Ok(ConvertResult {
        success: true,
        message,
        output_files,
        processed_count: count,
    })
}

/// 模式 2：按地块拆分。每个源建子目录，内部按地块拆。
fn convert_split_by_plot(
    sources: Vec<ImportSource>,
    header_cfg: &HeaderConfig,
    options: &ShpToTxtOptions,
    output_dir: &Path,
) -> Result<ConvertResult, String> {
    let mut output_files = Vec::new();
    let mut subdir_count = 0usize;
    let mut fallback_count = 0usize; // 源未找到所选字段、用序号命名的源数
    let mut conflict_count = 0usize; // 文件名冲突次数

    for src in &sources {
        let subdir = output_dir.join(&src.stem);
        std::fs::create_dir_all(&subdir)
            .map_err(|e| format!("创建子目录失败: {}", e))?;
        subdir_count += 1;

        // 该源是否含所选字段
        let field_exists = src
            .plots
            .iter()
            .any(|p| !options.filename_field.is_empty() && p.attributes.contains_key(&options.filename_field));

        let mut used_names: HashMap<String, usize> = HashMap::new();

        for p in &src.plots {
            // 一个 feature = 一个文件，多部件合在一起
            let base_name = if !options.filename_field.is_empty() && field_exists {
                // 取所选字段值
                let raw = p
                    .attributes
                    .get(&options.filename_field)
                    .cloned()
                    .unwrap_or_default();
                sanitize_filename(&raw)
            } else {
                // 字段不存在 / 选了"序号" / 值为空 → 序号兜底
                String::new()
            };
            let base_name = if base_name.is_empty() {
                format!("{}_{}", src.stem, p.index_in_source + 1)
            } else {
                base_name
            };

            if !options.filename_field.is_empty() && !field_exists {
                fallback_count += 1; // 仅在有选字段但源里没该字段时累加（每个地块一次）
            }

            let (final_name, bumped) = allocate_unique_name(&base_name, &mut used_names);
            if bumped {
                conflict_count += 1;
            }

            let txt_content = txt::generate_txt(
                &header_cfg.project_info,
                &header_cfg.attrs,
                &[p.plot.clone()],
                options.oj,
                options.oc,
            );
            let txt_path = subdir.join(format!("{}.txt", final_name));
            std::fs::write(&txt_path, &txt_content)
                .map_err(|e| format!("写 TXT 失败: {}", e))?;
            output_files.push(txt_path.to_string_lossy().to_string());
        }
    }

    let count = output_files.len();
    let mut message = format!("成功拆分为 {} 个文件（位于 {} 个子目录）", count, subdir_count);
    if fallback_count > 0 {
        message.push_str(&format!("（{} 个地块未找到所选字段，已用序号命名）", fallback_count));
    }
    if conflict_count > 0 {
        message.push_str(&format!("（{} 个文件名冲突已自动追加序号）", conflict_count));
    }
    Ok(ConvertResult {
        success: true,
        message,
        output_files,
        processed_count: count,
    })
}

/// 模式 3：全合并为一个 TXT（文件名带时间戳）
fn convert_merge_all(
    sources: Vec<ImportSource>,
    header_cfg: &HeaderConfig,
    options: &ShpToTxtOptions,
    output_dir: &Path,
) -> Result<ConvertResult, String> {
    let mut all_plots: Vec<txt::PlotData> = Vec::new();
    for src in &sources {
        for p in &src.plots {
            all_plots.push(p.plot.clone());
        }
    }

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("merged_output_{}.txt", timestamp);
    let txt_content = txt::generate_txt(
        &header_cfg.project_info,
        &header_cfg.attrs,
        &all_plots,
        options.oj,
        options.oc,
    );
    let txt_path = output_dir.join(&filename);
    std::fs::write(&txt_path, &txt_content).map_err(|e| format!("写 TXT 失败: {}", e))?;

    let output_files = vec![txt_path.to_string_lossy().to_string()];
    Ok(ConvertResult {
        success: true,
        message: format!("已合并输出：{}", filename),
        output_files,
        processed_count: 1,
    })
}

/// 为文件名分配唯一名：遇到重名追加 _2、_3...
/// 返回 (最终 stem, 是否发生过冲突)
fn allocate_unique_name(base: &str, used: &mut HashMap<String, usize>) -> (String, bool) {
    let base = if base.is_empty() { "output".to_string() } else { base.to_string() };
    if !used.contains_key(&base) {
        used.insert(base.clone(), 1);
        return (base, false);
    }
    let count = used.get_mut(&base).unwrap();
    *count += 1;
    (format!("{}_{}", base, count), true)
}

/// 清理文件名中的非法字符（Windows: / \ : * ? " < > |），用 _ 替换
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string();
    cleaned
}

pub fn convert_txt_to_shp(
    txt_paths: &[PathBuf],
    options: &TxtToShpOptions,
    header_cfg: &HeaderConfig,
) -> Result<ConvertResult, String> {
    let output_dir = Path::new(&options.output_dir);
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("创建输出目录失败: {}", e))?;

    match options.output_mode.as_str() {
        "split_by_plot" => txt_to_shp_split_by_plot(txt_paths, options, header_cfg, output_dir),
        "merge_all" => txt_to_shp_merge_all(txt_paths, options, header_cfg, output_dir),
        _ => txt_to_shp_one_to_one(txt_paths, options, header_cfg, output_dir),
    }
}

/// 模式 1：一对一。每个 TXT 输出一个 SHP（含该 TXT 的所有地块作为要素），平铺到 output_dir 根目录。
fn txt_to_shp_one_to_one(
    txt_paths: &[PathBuf],
    options: &TxtToShpOptions,
    header_cfg: &HeaderConfig,
    output_dir: &Path,
) -> Result<ConvertResult, String> {
    let mut output_files = Vec::new();
    let mut used_names: HashMap<String, usize> = HashMap::new();
    let mut conflict_count = 0usize;
    let mut skipped_count = 0usize;
    let mut warnings: Vec<String> = Vec::new();
    for txt_path in txt_paths {
        let text = txt::read_text_file(txt_path)?;
        let parsed = txt::parse_txt(&text);
        let stem = txt_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());

        // 带号优先级：坐标提取值 > 表单值 > 无（跳过）
        // 东坐标的 8 位前缀就是带号本身，比人工填的声明值可靠；矛盾时以坐标为准并记提示。
        let extracted = extract_zone_from_coords(&parsed.plots);
        let declared = parsed.attrs.get("带号").map(|s| s.as_str());
        let final_zone = match (extracted, header_cfg.attr("带号").as_str()) {
            (Some(z), _) => {
                // 检测与 TXT 声明带号的矛盾
                if let Some(d) = declared {
                    if let Ok(dz) = d.trim().parse::<i32>() {
                        if dz != z {
                            warnings.push(format!(
                                "{}：声明带号{}与坐标提取{}不一致，已用提取值",
                                stem, dz, z
                            ));
                        }
                    }
                }
                z.to_string()
            }
            (None, fz) if !fz.is_empty() => fz.to_string(),
            (None, _) => {
                skipped_count += 1;
                continue;
            }
        };

        if options.output_shp {
            let (final_name, bumped) = allocate_unique_name(&stem, &mut used_names);
            if bumped {
                conflict_count += 1;
            }
            let (geometries, mut attributes) = plots_to_surfaces_and_attributes(&parsed.plots);
            tag_attrs_with_source(&mut attributes, txt_path, options.keep_lujin, options.keep_mingc);
            let shp_files = shp::write_shapefile_structured(
                output_dir,
                &final_name,
                &geometries,
                &attributes,
                &header_cfg.attr("坐标系"),
                &header_cfg.attr("几度分带"),
                &final_zone,
            )?;
            output_files.extend(shp_files);
        }
    }
    let count = txt_paths.len() - skipped_count;
    let mut message = format!("成功转换 {} 个 TXT 文件", count);
    if skipped_count > 0 {
        message.push_str(&format!("（{} 个因无法确定带号已跳过，请手动填写带号）", skipped_count));
    }
    if conflict_count > 0 {
        message.push_str(&format!("（{} 个文件名冲突已自动追加序号）", conflict_count));
    }
    for w in &warnings {
        message.push_str(&format!("；{}", w));
    }
    Ok(ConvertResult {
        success: true,
        message,
        output_files,
        processed_count: count,
    })
}

/// 模式 2：按地块拆分。每个 TXT 建子目录 output_dir/{txt_stem}/，内部每个地块一个 SHP。
fn txt_to_shp_split_by_plot(
    txt_paths: &[PathBuf],
    options: &TxtToShpOptions,
    header_cfg: &HeaderConfig,
    output_dir: &Path,
) -> Result<ConvertResult, String> {
    let mut output_files = Vec::new();
    let mut subdir_count = 0usize;
    let mut fallback_count = 0usize; // 字段缺失/空值兜底为序号的地块数
    let mut conflict_count = 0usize; // 文件名冲突次数
    let mut skipped_count = 0usize; // 无法确定带号而跳过的 TXT 数
    let mut warnings: Vec<String> = Vec::new();

    for txt_path in txt_paths {
        let text = txt::read_text_file(txt_path)?;
        let parsed = txt::parse_txt(&text);
        let txt_stem = txt_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());

        // 带号优先级：坐标提取值 > 表单值 > 无（跳过）
        let extracted = extract_zone_from_coords(&parsed.plots);
        let declared = parsed.attrs.get("带号").map(|s| s.as_str());
        let final_zone = match (extracted, header_cfg.attr("带号").as_str()) {
            (Some(z), _) => {
                if let Some(d) = declared {
                    if let Ok(dz) = d.trim().parse::<i32>() {
                        if dz != z {
                            warnings.push(format!(
                                "{}：声明带号{}与坐标提取{}不一致，已用提取值",
                                txt_stem, dz, z
                            ));
                        }
                    }
                }
                z.to_string()
            }
            (None, fz) if !fz.is_empty() => fz.to_string(),
            (None, _) => {
                skipped_count += 1;
                continue;
            }
        };

        let subdir = output_dir.join(&txt_stem);
        std::fs::create_dir_all(&subdir)
            .map_err(|e| format!("创建子目录失败: {}", e))?;
        subdir_count += 1;

        // 冲突计数作用域：每个 TXT 子目录独立
        let mut used_names: HashMap<String, usize> = HashMap::new();

        for (idx, plot) in parsed.plots.iter().enumerate() {
            // 选 filename_field：DKMC 取 plot.name，FID 取 plot.fid，"" 或未知字段走序号兜底
            let raw = match options.filename_field.as_str() {
                "DKMC" => plot.name.clone(),
                "FID" => plot.fid.clone(),
                _ => String::new(),
            };
            let base_name = sanitize_filename(&raw);
            let base_name = if base_name.is_empty() {
                fallback_count += 1;
                format!("{}_{}", txt_stem, idx + 1)
            } else {
                base_name
            };

            let (final_name, bumped) = allocate_unique_name(&base_name, &mut used_names);
            if bumped {
                conflict_count += 1;
            }

            // 每地块一个 SHP（单要素）
            let (geometries, mut attributes) = plots_to_surfaces_and_attributes(std::slice::from_ref(plot));
            if geometries.is_empty() {
                continue; // 空地块跳过（与 plots_to_surfaces_and_attributes 的空 surface 跳过一致）
            }
            tag_attrs_with_source(&mut attributes, txt_path, options.keep_lujin, options.keep_mingc);
            let shp_files = shp::write_shapefile_structured(
                &subdir,
                &final_name,
                &geometries,
                &attributes,
                &header_cfg.attr("坐标系"),
                &header_cfg.attr("几度分带"),
                &final_zone,
            )?;
            output_files.extend(shp_files);
        }
    }

    let count = output_files
        .iter()
        .filter(|f| f.ends_with(".shp"))
        .count();
    let mut message = format!("成功拆分为 {} 个文件（位于 {} 个子目录）", count, subdir_count);
    if skipped_count > 0 {
        message.push_str(&format!("（{} 个 TXT 因无法确定带号已跳过，请手动填写带号）", skipped_count));
    }
    if fallback_count > 0 {
        message.push_str(&format!("（{} 个地块未找到所选字段，已用序号命名）", fallback_count));
    }
    if conflict_count > 0 {
        message.push_str(&format!("（{} 个文件名冲突已自动追加序号）", conflict_count));
    }
    for w in &warnings {
        message.push_str(&format!("；{}", w));
    }
    Ok(ConvertResult {
        success: true,
        message,
        output_files,
        processed_count: count,
    })
}

/// 模式 3：全合并为一个 SHP（文件名带时间戳，避免重跑覆盖）。
fn txt_to_shp_merge_all(
    txt_paths: &[PathBuf],
    options: &TxtToShpOptions,
    header_cfg: &HeaderConfig,
    output_dir: &Path,
) -> Result<ConvertResult, String> {
    // 逐文件提取带号，检测冲突：merge_all 要求所有 TXT 带号一致，冲突直接拒绝
    let mut zones: Vec<Option<i32>> = Vec::new();
    for txt_path in txt_paths {
        let text = txt::read_text_file(txt_path)?;
        let parsed = txt::parse_txt(&text);
        let z = extract_zone_from_coords(&parsed.plots);
        if let Some(zv) = z {
            zones.push(Some(zv));
        } else if !header_cfg.attr("带号").is_empty() {
            // 提取失败时回退表单值（后续统一用表单值，此处只用于冲突检测）
            zones.push(None);
        } else {
            return Err(format!(
                "无法确定 {} 的带号（坐标无8位前缀且表单未填写），请手动填写带号",
                txt_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
            ));
        }
    }
    let distinct: Vec<i32> = zones.iter().filter_map(|z| *z).collect();
    if distinct.len() > 1 {
        let mut seen: Vec<i32> = Vec::new();
        for z in &distinct {
            if !seen.contains(z) {
                seen.push(*z);
            }
        }
        return Err(format!(
            "合并失败：各 TXT 带号不一致（{}），无法合并为同一坐标系，请先统一带号",
            seen.iter().map(|z| z.to_string()).collect::<Vec<_>>().join("/")
        ));
    }

    // 按 TXT 分组构建几何+属性，每组构建后立即 tag 来源路径/名称，再合并
    // （保证 geometry 与 attribute 顺序对齐，且每个 plot 记录正确的源 TXT）
    let mut all_geometries: Vec<SurfaceGeometry> = Vec::new();
    let mut all_attributes: Vec<HashMap<String, String>> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for txt_path in txt_paths {
        let text = txt::read_text_file(txt_path)?;
        let parsed = txt::parse_txt(&text);
        // 矛盾检测（merge 模式下带号已统一，仅记录矛盾提示）
        if let Some(z) = extract_zone_from_coords(&parsed.plots) {
            if let Some(d) = parsed.attrs.get("带号") {
                if let Ok(dz) = d.trim().parse::<i32>() {
                    if dz != z {
                        warnings.push(format!(
                            "{}：声明带号{}与坐标提取{}不一致，已用提取值",
                            txt_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                            dz, z
                        ));
                    }
                }
            }
        }
        let (geos, mut attrs) = plots_to_surfaces_and_attributes(&parsed.plots);
        tag_attrs_with_source(&mut attrs, txt_path, options.keep_lujin, options.keep_mingc);
        all_geometries.extend(geos);
        all_attributes.extend(attrs);
    }
    let mut output_files = Vec::new();
    if options.output_shp {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("merged_output_{}", timestamp);
        // 确定最终带号：提取值优先，否则回退表单值（前面已保证不会两者皆空）
        let final_zone = distinct.first().map(|z| z.to_string()).unwrap_or_else(|| header_cfg.attr("带号"));
        let shp_files = shp::write_shapefile_structured(
            output_dir,
            &filename,
            &all_geometries,
            &all_attributes,
            &header_cfg.attr("坐标系"),
            &header_cfg.attr("几度分带"),
            &final_zone,
        )?;
        output_files.extend(shp_files);
        let mut message = format!("已合并输出：{}.shp", filename);
        for w in &warnings {
            message.push_str(&format!("；{}", w));
        }
        Ok(ConvertResult {
            success: true,
            message,
            output_files,
            processed_count: 1,
        })
    } else {
        Ok(ConvertResult {
            success: true,
            message: "未选择 SHP 输出".to_string(),
            output_files,
            processed_count: 0,
        })
    }
}

fn shp_files_to_plots(
    shp_paths: &[PathBuf],
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    proj_cfg: Option<&ProjectionConfig>,
) -> Result<Vec<txt::PlotData>, String> {
    let mut all_plots = Vec::new();
    for shp_path in shp_paths {
        all_plots.extend(single_shp_to_plots(shp_path, field_mapping, options, proj_cfg)?);
    }
    Ok(all_plots)
}

/// 把单个 SHP 文件解析为一个 ImportSource（保留每个地块的完整属性）
fn single_shp_to_source(
    shp_path: &PathBuf,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    proj_cfg: Option<&ProjectionConfig>,
) -> Result<ImportSource, String> {
    let info = shp::read_shp_file_group(shp_path)?;
    let features = shp::read_shp(shp_path)?;
    let stem = shp_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());

    // og 投影：采样判大地坐标系，是则对每个 feature 投影（仅大地坐标系输入生效）
    let sample = features
        .first()
        .and_then(|f| f.surface.parts.first())
        .and_then(|p| p.exterior.first())
        .copied();
    let ellipsoid = info
        .crs_info
        .get("c")
        .and_then(|n| projection::Ellipsoid::from_crs_name(n))
        .unwrap_or(projection::Ellipsoid::CGCS2000);
    let proj_apply = match proj_cfg {
        Some(cfg) if sample_is_geodetic(sample, &info.crs_info) => {
            if cfg.zone <= 0 {
                return Err("勾选了输出公里网但未填写带号".to_string());
            }
            true
        }
        _ => false,
    };

    let mut plots = Vec::new();
    for (fi, feat) in features.iter().enumerate() {
        let record = info.field_records.get(fi).cloned().unwrap_or_default();
        let projected;
        let surface: &SurfaceGeometry = if proj_apply {
            projected = project_surface_with_prefix(&feat.surface, proj_cfg.unwrap(), ellipsoid)?;
            &projected
        } else {
            &feat.surface
        };
        let plot_name = resolve_value(&field_mapping.name, "name", &info.field_names, &record);
        let plot_area = resolve_area(&field_mapping.area, surface, &info.field_names, &record);
        let plot_use = resolve_value(&field_mapping.use_field, "use_field", &info.field_names, &record);
        let plot_tfh = resolve_value(&field_mapping.tfh, "tfh", &info.field_names, &record);
        let plot_dlbm = resolve_value(&field_mapping.dlbm, "dlbm", &info.field_names, &record);
        let plot_id = {
            let v = resolve_value(&field_mapping.id, "id", &info.field_names, &record);
            if v.is_empty() { (fi + 1).to_string() } else { v }
        };

        // 完整属性表（用于模式 2 按字段命名）
        let mut attributes = HashMap::new();
        for (idx, fname) in info.field_names.iter().enumerate() {
            if let Some(val) = record.get(idx) {
                attributes.insert(fname.clone(), val.clone());
            }
        }

        plots.push(PlotWithSource {
            plot: build_plot_data(
                surface,
                plot_id,
                plot_name,
                plot_area,
                plot_use,
                plot_tfh,
                plot_dlbm,
                options,
            ),
            source_stem: stem.clone(),
            index_in_source: fi,
            attributes,
        });
    }

    Ok(ImportSource { stem, plots })
}

/// 保留旧 API：仅返回 PlotData 列表（预览用）
fn single_shp_to_plots(
    shp_path: &PathBuf,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    proj_cfg: Option<&ProjectionConfig>,
) -> Result<Vec<txt::PlotData>, String> {
    let src = single_shp_to_source(shp_path, field_mapping, options, proj_cfg)?;
    Ok(src.plots.into_iter().map(|p| p.plot).collect())
}

fn gdb_features_to_plots(
    info: &gdb::GdbFileInfo,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    proj_cfg: Option<&ProjectionConfig>,
    selected_layers: Option<&[String]>,
) -> Result<Vec<txt::PlotData>, String> {
    let sources = gdb_to_sources(info, field_mapping, options, proj_cfg, selected_layers)?;
    let mut all_plots = Vec::new();
    for src in sources {
        for p in src.plots {
            all_plots.push(p.plot);
        }
    }
    Ok(all_plots)
}

/// 把 GDB 解析为多个 ImportSource（每个要素类一个源）
fn gdb_to_sources(
    info: &gdb::GdbFileInfo,
    field_mapping: &FieldMapping,
    options: &ShpToTxtOptions,
    proj_cfg: Option<&ProjectionConfig>,
    selected_layers: Option<&[String]>,
) -> Result<Vec<ImportSource>, String> {
    let gdb_stem = info.name.clone();
    let mut sources = Vec::new();
    // GDB 不带 crs_info：ellipsoid 默认 CGCS2000，is_geodetic 纯靠坐标采样判定
    let ellipsoid = projection::Ellipsoid::CGCS2000;
    let empty_crs: HashMap<String, String> = HashMap::new();

    for (layer_idx, features) in info.all_features.iter().enumerate() {
        let layer_info = info.layers.get(layer_idx);
        let layer_name = layer_info.map(|l| l.name.as_str()).unwrap_or("");
        let geom_type = layer_info.map(|l| l.geometry_type.as_str()).unwrap_or("");

        // 仅处理面状图层（导入时已过滤，这里兜底防止漏网）
        if !is_polygon_geometry_type(geom_type) {
            continue;
        }

        if let Some(sel) = selected_layers {
            if !sel.is_empty() && !sel.iter().any(|n| n == layer_name) {
                continue;
            }
        }

        // og 投影：采样本图层首个点判大地坐标系（是则投影，仅大地坐标系输入生效）
        let sample = features
            .first()
            .and_then(|f| f.surface.parts.first())
            .and_then(|p| p.exterior.first())
            .copied();
        let proj_apply = match proj_cfg {
            Some(cfg) if sample_is_geodetic(sample, &empty_crs) => {
                if cfg.zone <= 0 {
                    return Err("勾选了输出公里网但未填写带号".to_string());
                }
                true
            }
            _ => false,
        };

        let stem = format!("{}_{}", gdb_stem, layer_name);
        let mut plots = Vec::new();
        for (fi, feat) in features.iter().enumerate() {
            let projected;
            let surface: &SurfaceGeometry = if proj_apply {
                projected = project_surface_with_prefix(&feat.surface, proj_cfg.unwrap(), ellipsoid)?;
                &projected
            } else {
                &feat.surface
            };
            let plot_name = resolve_value_map(&field_mapping.name, "name", &feat.attributes);
            let plot_area = resolve_area_map(&field_mapping.area, surface, &feat.attributes);
            let plot_use = resolve_value_map(&field_mapping.use_field, "use_field", &feat.attributes);
            let plot_tfh = resolve_value_map(&field_mapping.tfh, "tfh", &feat.attributes);
            let plot_dlbm = resolve_value_map(&field_mapping.dlbm, "dlbm", &feat.attributes);
            let plot_id = {
                let v = resolve_value_map(&field_mapping.id, "id", &feat.attributes);
                if v.is_empty() { (fi + 1).to_string() } else { v }
            };

            plots.push(PlotWithSource {
                plot: build_plot_data(
                    surface,
                    plot_id,
                    plot_name,
                    plot_area,
                    plot_use,
                    plot_tfh,
                    plot_dlbm,
                    options,
                ),
                source_stem: stem.clone(),
                index_in_source: fi,
                attributes: feat.attributes.clone(),
            });
        }
        sources.push(ImportSource { stem, plots });
    }

    Ok(sources)
}

fn build_plot_data(
    surface: &SurfaceGeometry,
    plot_id: String,
    plot_name: String,
    plot_area: String,
    plot_use: String,
    plot_tfh: String,
    plot_dlbm: String,
    options: &ShpToTxtOptions,
) -> txt::PlotData {
    // 默认 (ox=false) 输出标准 (Y,X)（北坐标在前，与 TXT→SHP 认定的输入顺序一致，保证往返）；
    // 勾选 ox 时输出 (X,Y)（东坐标在前，标反）。取反是因为 swap_xy=true 才执行 xy_to_yx 交换。
    let rings = surface_to_indexed_rings(surface, options.on, options.oo, !options.ox);
    let coords = rings.iter().flat_map(|ring| ring.coords.iter().copied()).collect::<Vec<_>>();

    txt::PlotData {
        point_count: coords.len() as u32,
        area: plot_area,
        fid: plot_id,
        name: plot_name,
        geom_type: "面".to_string(),
        tfh: plot_tfh,
        use_field: plot_use,
        dlbm: plot_dlbm,
        coords,
        rings,
    }
}

/// 给一批属性行注入来源 TXT 路径/名称（按勾选状态）。
/// keep_lujin → 每行插入 "LUJIN" = 源 TXT 完整路径
/// keep_mingc → 每行插入 "MINGC" = 源 TXT 文件名（带 .txt）
fn tag_attrs_with_source(
    attributes: &mut [HashMap<String, String>],
    txt_path: &std::path::Path,
    keep_lujin: bool,
    keep_mingc: bool,
) {
    if keep_lujin {
        let p = txt_path.to_string_lossy().to_string();
        for a in attributes.iter_mut() {
            a.insert("LUJIN".to_string(), p.clone());
        }
    }
    if keep_mingc {
        let n = txt_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        for a in attributes.iter_mut() {
            a.insert("MINGC".to_string(), n.clone());
        }
    }
}

fn plots_to_surfaces_and_attributes(
    plots: &[txt::PlotData],
) -> (Vec<SurfaceGeometry>, Vec<HashMap<String, String>>) {
    let mut geometries = Vec::new();
    let mut attributes = Vec::new();

    for plot in plots {
        let surface = if plot.rings.is_empty() {
            indexed_rings_to_surface(&[crate::geometry::IndexedRing {
                part_index: 1,
                coords: plot.coords.clone(),
            }])
        } else {
            indexed_rings_to_surface(&plot.rings)
        };
        if !surface.parts.is_empty() {
            geometries.push(surface);

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


// ═══ 统一字段映射解析 ═══

/// 各字段对应的占位文字
fn field_placeholder(field_key: &str) -> &'static str {
    match field_key {
        "name" => "DKMC",
        "id" => "DKBH",
        "area" => "MJ",
        "use_field" => "DKYT",
        "tfh" => "TFH",
        "dlbm" => "DLBM",
        _ => "",
    }
}

/// 解析字段值（SHP 版，index 查找）
fn resolve_value(
    mapping: &str,
    field_key: &str,
    field_names: &[String],
    record: &[String],
) -> String {
    match mapping {
        "" => String::new(),
        "__placeholder__" => field_placeholder(field_key).to_string(),
        other => {
            if let Some(pos) = field_names.iter().position(|n| n == other) {
                if pos < record.len() {
                    return record[pos].clone();
                }
            }
            String::new()
        }
    }
}

/// 解析字段值（GDB 版，HashMap 查找）
fn resolve_value_map(
    mapping: &str,
    field_key: &str,
    attrs: &HashMap<String, String>,
) -> String {
    match mapping {
        "" => String::new(),
        "__placeholder__" => field_placeholder(field_key).to_string(),
        other => attrs.get(other).cloned().unwrap_or_default(),
    }
}

/// 从 SurfaceGeometry 计算多边形面积（外环面积 - 孔面积），单位：平方米
fn calculate_area_from_surface(surface: &SurfaceGeometry) -> f64 {
    let mut total = 0.0f64;
    for part in &surface.parts {
        total += crate::geometry::signed_area(&part.exterior).abs();
        for hole in &part.holes {
            total -= crate::geometry::signed_area(hole).abs();
        }
    }
    total.abs()
}

/// 解析面积值（SHP 版，含自动计算）
fn resolve_area(
    mapping: &str,
    surface: &SurfaceGeometry,
    field_names: &[String],
    record: &[String],
) -> String {
    match mapping {
        "" => String::new(),
        "__placeholder__" => "MJ".to_string(),
        "__area_sqm__" => format!("{:.2}", calculate_area_from_surface(surface)),
        "__area_ha__" => format!("{:.4}", calculate_area_from_surface(surface) / 10000.0),
        other => resolve_value(other, "area", field_names, record),
    }
}

/// 解析面积值（GDB 版，含自动计算）
fn resolve_area_map(
    mapping: &str,
    surface: &SurfaceGeometry,
    attrs: &HashMap<String, String>,
) -> String {
    match mapping {
        "" => String::new(),
        "__placeholder__" => "MJ".to_string(),
        "__area_sqm__" => format!("{:.2}", calculate_area_from_surface(surface)),
        "__area_ha__" => format!("{:.4}", calculate_area_from_surface(surface) / 10000.0),
        other => resolve_value_map(other, "area", attrs),
    }
}
/// 从坐标点列表中提取高斯-克吕格带号。
/// 扫描所有地块的所有点，找第一个整数部分为 8 位的 X 值（东坐标），
/// 取其前两位作为带号，若前两位落在 13-45 区间则返回，否则继续扫描。
/// 全部不合法返回 None，交由调用方回退。
fn extract_zone_from_coords(plots: &[txt::PlotData]) -> Option<i32> {
    for plot in plots {
        // coords 存储为 (y, x) = (northing, easting)，x 在第二位
        for &(_, x) in &plot.coords {
            let abs_x = x.abs();
            if abs_x >= 10_000_000.0 && abs_x < 100_000_000.0 {
                let prefix = (abs_x / 1_000_000.0) as i32;
                if (13..=45).contains(&prefix) {
                    return Some(prefix);
                }
            }
            // 同时检查 rings 里的坐标（多部件情况）
        }
        // rings 与 coords 内容重叠（parse 时同时填入），上面已遍历 coords 即可
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::txt::PlotData;

    fn plot_with_coords(coords: Vec<(f64, f64)>) -> PlotData {
        PlotData {
            point_count: coords.len() as u32,
            area: String::new(),
            fid: String::new(),
            name: String::new(),
            geom_type: "面".to_string(),
            tfh: String::new(),
            use_field: String::new(),
            dlbm: String::new(),
            coords,
            rings: Vec::new(),
        }
    }

    #[test]
    fn extract_zone_8digit_prefix() {
        // 38378508 -> 前两位 38
        let p = plot_with_coords(vec![(2585776.157, 38378508.034)]);
        assert_eq!(extract_zone_from_coords(&[p]), Some(38));
    }

    #[test]
    fn extract_zone_6digit_natural_value() {
        // 6位自然值，无带号前缀 -> None
        let p = plot_with_coords(vec![(3000000.0, 450000.123)]);
        assert_eq!(extract_zone_from_coords(&[p]), None);
    }

    #[test]
    fn extract_zone_out_of_range_prefix() {
        // 05xxxxxx -> 前两位 05，不在 13-45 范围 -> None
        let p = plot_with_coords(vec![(1000000.0, 5000000.0)]);
        assert_eq!(extract_zone_from_coords(&[p]), None);
    }

    #[test]
    fn extract_zone_empty_coords() {
        let p = plot_with_coords(vec![]);
        assert_eq!(extract_zone_from_coords(&[p]), None);
    }

    #[test]
    fn extract_zone_skips_invalid_finds_valid() {
        // 第一个点 6 位不合法，第二个点 8 位合法 -> 返回第二个的带号
        let p = plot_with_coords(vec![(3000000.0, 450000.0), (2585776.0, 39378508.0)]);
        assert_eq!(extract_zone_from_coords(&[p]), Some(39));
    }

    #[test]
    fn extract_zone_negative_x() {
        // 负坐标取绝对值后判断（理论上东坐标不应为负，但防御性测试）
        let p = plot_with_coords(vec![(-1000000.0, -38378508.0)]);
        assert_eq!(extract_zone_from_coords(&[p]), Some(38));
    }
}
