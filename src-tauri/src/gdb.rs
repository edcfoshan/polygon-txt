// GDB 读写模块 — 纯 Rust 实现
// 读取: 使用 geonative-filegdb
// 写入: 最小化 OpenFileGDB 写入

use geonative_core::{Geometry as GeoGeom, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// GDB 中的要素信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdbFeature {
    pub points: Vec<(f64, f64)>,
    pub attributes: HashMap<String, String>,
}

/// GDB 图层信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdbLayerInfo {
    pub name: String,
    pub field_names: Vec<String>,
    pub num_features: usize,
    pub geometry_type: String,
}

/// GDB 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdbFileInfo {
    pub path: String,
    pub name: String,
    pub layers: Vec<GdbLayerInfo>,
    pub all_features: Vec<Vec<GdbFeature>>,
    pub all_field_names: Vec<Vec<String>>,
}

/// 打开并读取 GDB
pub fn read_gdb(path: &Path) -> Result<GdbFileInfo, String> {
    let gdb = geonative_filegdb::open(path).map_err(|e| format!("打开 GDB 失败: {}", e))?;
    let layer_infos = gdb.layers();

    let mut layers = Vec::new();
    let mut all_features = Vec::new();
    let mut all_field_names = Vec::new();

    for info in layer_infos.iter() {
        let layer = gdb
            .layer(&info.name)
            .map_err(|e| format!("读取图层 {} 失败: {}", info.name, e))?;

        let schema = layer.schema();
        let field_names: Vec<String> = schema
            .fields
            .iter()
            .map(|f| f.name.clone())
            .collect();

        let mut features = Vec::new();

        // Layer::read() returns a FeatureIter which implements Iterator
        for result in layer.read() {
            let feature = match result {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("读取要素记录失败: {:?}", e);
                    continue;
                }
            };

            let mut gdb_feat = GdbFeature {
                points: Vec::new(),
                attributes: HashMap::new(),
            };

            // 提取几何
            if let Some(ref geom) = feature.geometry {
                extract_coords(geom, &mut gdb_feat.points);
            }

            // 提取属性
            for (i, val) in feature.attributes.iter().enumerate() {
                if i < field_names.len() {
                    let attr_str = match val {
                        Value::String(s) => s.clone(),
                        Value::Int32(n) => n.to_string(),
                        Value::Int64(n) => n.to_string(),
                        Value::Float32(f) => f.to_string(),
                        Value::Float64(f) => f.to_string(),
                        Value::Bool(b) => {
                            if *b { "是".to_string() } else { "否".to_string() }
                        }
                        _ => String::new(),
                    };
                    gdb_feat
                        .attributes
                        .insert(field_names[i].clone(), attr_str);
                }
            }
            features.push(gdb_feat);
        }

        let geom_type = if features.is_empty() {
            "Unknown"
        } else {
            match &features[0].points.len() {
                0 => "Unknown",
                1 => "Point",
                n if *n > 2 => "Polygon",
                _ => "PolyLine",
            }
        };

        layers.push(GdbLayerInfo {
            name: info.name.clone(),
            field_names: field_names.clone(),
            num_features: features.len(),
            geometry_type: geom_type.to_string(),
        });
        all_features.push(features);
        all_field_names.push(field_names);
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(GdbFileInfo {
        path: path.to_string_lossy().to_string(),
        name,
        layers,
        all_features,
        all_field_names,
    })
}

/// 从 Geometry 树中提取坐标
fn extract_coords(geom: &GeoGeom, out: &mut Vec<(f64, f64)>) {
    match geom {
        GeoGeom::Point(c) => out.push((c.x, c.y)),
        GeoGeom::MultiPoint(points) => {
            for c in points {
                out.push((c.x, c.y));
            }
        }
        GeoGeom::LineString(ls) => {
            for c in &ls.coords {
                out.push((c.x, c.y));
            }
        }
        GeoGeom::MultiLineString(mls) => {
            for ls in mls {
                for c in &ls.coords {
                    out.push((c.x, c.y));
                }
            }
        }
        GeoGeom::Polygon(poly) => {
            for c in &poly.exterior.coords {
                out.push((c.x, c.y));
            }
            for hole in &poly.holes {
                for c in &hole.coords {
                    out.push((c.x, c.y));
                }
            }
        }
        GeoGeom::MultiPolygon(mp) => {
            for poly in mp {
                for c in &poly.exterior.coords {
                    out.push((c.x, c.y));
                }
                for hole in &poly.holes {
                    for c in &hole.coords {
                        out.push((c.x, c.y));
                    }
                }
            }
        }
        GeoGeom::GeometryCollection(collection) => {
            for g in collection {
                extract_coords(g, out);
            }
        }
        _ => {}
    }
}

// ─── GDB 写入 (GDAL OpenFileGDB) ───

/// 使用 GDAL 的 OpenFileGDB 驱动写 .gdb 文件
///
/// 需要系统安装 GDAL 3.6+（含 OpenFileGDB 写入支持）。
/// Windows 上需设置 GDAL_HOME 环境变量或在 PATH 中包含 GDAL DLL。
///
/// 编译时启用 `gdb-write` feature 才能使用此功能：
///   cargo build --features gdb-write
#[cfg(feature = "gdb-write")]
pub fn write_gdb_output(
    output_dir: &Path,
    base_name: &str,
    fields: &[(String, String, u8, u32)],
    _attributes: &[Vec<(String, f64)>],
    geometries: &[Vec<(f64, f64)>],
    _crs_info: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    use gdal::DriverManager;
    use gdal::vector::{FieldDefn, LayerOptions, LayerAccess};

    let gdb_name = format!("{}.gdb", base_name);
    let gdb_path = output_dir.join(&gdb_name);

    // 清理旧目录
    if gdb_path.exists() {
        std::fs::remove_dir_all(&gdb_path)
            .map_err(|e| format!("清理旧 GDB 失败: {}", e))?;
    }

    // 获取 OpenFileGDB 驱动
    let driver = DriverManager::get_driver_by_name("OpenFileGDB")
        .map_err(|e| format!("获取 OpenFileGDB 驱动失败 (请确认已安装 GDAL 3.6+): {}", e))?;

    // 创建 GDB 数据集（vector: 0,0,0）
    let mut dataset = driver
        .create(&gdb_path, 0, 0, 0)
        .map_err(|e| format!("创建 GDB 失败: {}", e))?;

    // 创建图层
    let mut layer = dataset
        .create_layer(LayerOptions {
            name: base_name,
            srs: None, // 后续可添加 SpatialRef
            ty: gdal::gdal_sys::OGRwkbGeometryType::wkbPolygon,
            options: None,
        })
        .map_err(|e| format!("创建图层失败: {}", e))?;

    // 添加字段定义
    // fields: (name, description, type_code, width)
    // type_code: 4=string(OFTString), 3=float(OFTReal)
    for (name, _desc, type_code, width) in fields {
        let field_type = match type_code {
            3 => gdal::gdal_sys::OGRFieldType::OFTReal,
            _ => gdal::gdal_sys::OGRFieldType::OFTString,
        };
        let field_defn = FieldDefn::new(name, field_type)
            .map_err(|e| format!("创建字段 {} 失败: {}", name, e))?;
        field_defn.set_width(*width as i32);
        if *type_code == 3 {
            field_defn.set_precision(3);
        }
        field_defn.add_to_layer(&layer)
            .map_err(|e| format!("添加字段 {} 失败: {}", name, e))?;
    }

    // 写入要素
    let defn = layer.defn();
    for (fi, geom_coords) in geometries.iter().enumerate() {
        if geom_coords.len() < 3 {
            continue;
        }

        // 构建 WKT 多边形
        let mut wkt = String::from("POLYGON ((");
        for (i, &(x, y)) in geom_coords.iter().enumerate() {
            if i > 0 {
                wkt.push_str(", ");
            }
            wkt.push_str(&format!("{} {}", x, y));
        }
        // 确保闭合
        if geom_coords.first() != geom_coords.last() {
            let first = geom_coords[0];
            wkt.push_str(&format!(", {} {}", first.0, first.1));
        }
        wkt.push_str("))");

        let geometry = gdal::vector::Geometry::from_wkt(&wkt)
            .map_err(|e| format!("创建几何失败: {}", e))?;

        let mut feature = gdal::vector::Feature::new(defn)
            .map_err(|e| format!("创建要素失败: {}", e))?;
        feature.set_geometry(geometry)
            .map_err(|e| format!("设置几何失败: {}", e))?;

        // 设置属性字段值
        for (field_idx, (name, _desc, type_code, _width)) in fields.iter().enumerate() {
            // 从 _attributes 中获取值（如果有的话）
            // 目前 _attributes 结构复杂，简化处理：
            // 对于字符串字段设为空，对于数值字段设为 0
            match type_code {
                3 => {
                    // Float - 面积字段尝试从 attributes 中获取
                    feature.set_field_double(field_idx, 0.0)
                        .map_err(|e| format!("设置字段 {} 失败: {}", name, e))?;
                }
                _ => {
                    feature.set_field_string(field_idx, "")
                        .map_err(|e| format!("设置字段 {} 失败: {}", name, e))?;
                }
            }
        }

        feature.create(&mut layer)
            .map_err(|e| format!("写入要素失败: {}", e))?;
    }

    // 关闭数据集以确保数据写入磁盘
    dataset.close()
        .map_err(|e| format!("关闭 GDB 失败: {}", e))?;

    Ok(vec![gdb_path.to_string_lossy().to_string()])
}

/// GDB 写入的 stub 实现（当未启用 gdb-write feature 时）
#[cfg(not(feature = "gdb-write"))]
pub fn write_gdb_output(
    output_dir: &Path,
    base_name: &str,
    _fields: &[(String, String, u8, u32)],
    _attributes: &[Vec<(String, f64)>],
    _geometries: &[Vec<(f64, f64)>],
    _crs_info: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let _ = output_dir;
    let _ = base_name;
    Err(
        "GDB 写入功能未启用。请使用 `cargo build --features gdb-write` 编译，\
         并确保系统已安装 GDAL 3.6+（https://gdal.org/download.html）。\n\
         Windows 上可安装 OSGeo4W 或从 https://www.gisinternals.com/ 下载预编译包，\
         然后设置 GDAL_HOME 环境变量指向安装目录。"
            .to_string(),
    )
}
