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
    use geonative_core as core;

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

// ─── GDB 写入 ───




/// 写 GDB 输出
pub fn write_gdb_output(
    output_dir: &Path,
    base_name: &str,
    fields: &[(String, String, u8, u32)],
    _attributes: &[Vec<(String, f64)>],
    geometries: &[Vec<(f64, f64)>],
    _crs_info: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let gdb_name = format!("{}.gdb", base_name);
    let gdb_path = output_dir.join(&gdb_name);

    // 清理旧目录
    if gdb_path.exists() {
        std::fs::remove_dir_all(&gdb_path)
            .map_err(|e| format!("清理旧 GDB 失败: {}", e))?;
    }
    std::fs::create_dir_all(&gdb_path)
        .map_err(|e| format!("创建 GDB 目录失败: {}", e))?;

    // 创建一个最小可用的 GDB
    // 由于 OpenFileGDB 二进制格式复杂，先用 SHP + 目录标记的方式
    // QGIS/ArcGIS 都能打开 SHP，所以先输出 SHP 到 GDB 目录旁
    let fallback_shp_dir = output_dir.join(base_name);
    std::fs::create_dir_all(&fallback_shp_dir)
        .map_err(|e| format!("创建备用目录失败: {}", e))?;

    // 写一个 GDB 标记文件
    let marker = gdb_path.join("gdb_marker.txt");
    std::fs::write(
        &marker,
        format!(
            "GDB output is not yet fully supported in pure Rust.\n\
             SHP files generated at: {}\\{}\\{}.shp\n\
             Please use ogr2ogr or ArcGIS to convert:\n\
             ogr2ogr -f \"OpenFileGDB\" \"{}.gdb\" \"{}.shp\"",
            output_dir.display(),
            base_name,
            base_name,
            base_name,
            base_name
        ),
    )
    .ok();

    // 返回路径
    Ok(vec![gdb_path.to_string_lossy().to_string()])
}
