// SHP / DBF / PRJ 文件读写模块
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// 从 SHP 读取的要素信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShpFeature {
    pub points: Vec<(f64, f64)>, // (x, y) = (easting, northing)
}

/// SHP 文件的摘要信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShpFileInfo {
    pub name: String,
    pub shp_path: String,
    pub dbf_path: Option<String>,
    pub prj_path: Option<String>,
    pub field_names: Vec<String>,
    pub field_records: Vec<Vec<String>>,
    pub num_features: usize,
    pub shape_type: String,
    pub prj_text: Option<String>,
    pub crs_info: HashMap<String, String>,
}

/// 解析 .shp 文件，返回所有多边形
pub fn read_shp(path: &Path) -> Result<Vec<ShpFeature>, String> {
    use shapefile::ShapeReader;

    let mut reader =
        ShapeReader::from_path(path).map_err(|e| format!("打开 SHP 失败: {}", e))?;

    let mut features = Vec::new();
    for result in reader.iter_shapes() {
        let shape = result.map_err(|e| format!("读取 SHP 图形: {}", e))?;
        match shape {
            shapefile::Shape::Polygon(poly) => {
                for ring in poly.rings() {
                    // Only extract outer rings; inner rings (holes) are skipped
                    // because the TXT format uses single-ring plots
                    if let shapefile::PolygonRing::Outer(pts) = ring {
                        let points: Vec<(f64, f64)> =
                            pts.iter().map(|p| (p.x, p.y)).collect();
                        if !points.is_empty() {
                            features.push(ShpFeature { points });
                        }
                    }
                }
            }
            shapefile::Shape::Polyline(pl) => {
                for part in pl.parts() {
                    let pts: Vec<(f64, f64)> = part.iter().map(|p| (p.x, p.y)).collect();
                    if !pts.is_empty() {
                        features.push(ShpFeature { points: pts });
                    }
                }
            }
            shapefile::Shape::Point(p) => {
                features.push(ShpFeature {
                    points: vec![(p.x, p.y)],
                });
            }
            _ => {}
        }
    }
    Ok(features)
}

/// 解析 .dbf 文件，返回字段名列表和所有记录
pub fn read_dbf(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    use dbase::FieldValue;

    let records =
        dbase::read(path).map_err(|e| format!("打开 DBF 失败: {}", e))?;

    if records.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    // Collect field names from the first record via iteration
    let mut field_names: Vec<String> = Vec::new();
    // Record implements IntoIterator, producing (FieldName, FieldValue) pairs
    let field_iter: Vec<(String, dbase::FieldValue)> = records[0].clone().into_iter().collect();
    for (name, _val) in &field_iter {
        if name != "DeletionFlag"
            && !name.starts_with("FID")
            && !name.eq_ignore_ascii_case("SHAPE")
            && !name.starts_with("SHAPE_LENG")
            && !name.starts_with("SHAPE_AREA")
            && !name.starts_with("OBJECTID")
        {
            field_names.push(name.clone());
        }
    }

    let mut string_records = Vec::new();
    for record in &records {
        let mut row = Vec::new();
        for name in &field_names {
            let val = record.get(name.as_str());
            match val {
                Some(FieldValue::Character(Some(s))) => row.push(s.clone()),
                Some(FieldValue::Numeric(Some(n))) => row.push(n.to_string()),
                Some(FieldValue::Float(Some(f))) => row.push(f.to_string()),
                Some(FieldValue::Integer(i)) => row.push(i.to_string()),
                Some(FieldValue::Double(d)) => row.push(d.to_string()),
                Some(FieldValue::Logical(Some(b))) => {
                    row.push(if *b { "是".to_string() } else { "否".to_string() })
                }
                Some(FieldValue::Date(Some(d))) => row.push(d.to_string()),
                _ => row.push(String::new()),
            }
        }
        string_records.push(row);
    }

    Ok((field_names, string_records))
}

/// 大小写不敏感的字符串包含
fn icontains(text: &str, pattern: &str) -> bool {
    text.to_lowercase().contains(&pattern.to_lowercase())
}

/// 解析 .prj 文件，提取坐标系信息
pub fn read_prj(path: &Path) -> Result<(String, HashMap<String, String>), String> {
    let mut buf = String::new();
    std::fs::File::open(path)
        .map_err(|e| format!("打开 PRJ 失败: {}", e))?
        .read_to_string(&mut buf)
        .map_err(|e| format!("读取 PRJ 失败: {}", e))?;
    let prj_text = buf.trim().to_string();
    let mut info = HashMap::new();

    if icontains(&prj_text, "CGCS2000")
        || icontains(&prj_text, "2000国家")
        || icontains(&prj_text, "China_2000")
    {
        info.insert("c".into(), "2000国家大地坐标系".into());
    } else if icontains(&prj_text, "Xian_1980") || icontains(&prj_text, "1980西安") {
        info.insert("c".into(), "1980西安坐标系".into());
    } else if icontains(&prj_text, "Beijing_1954") || icontains(&prj_text, "1954北京") {
        info.insert("c".into(), "1954北京坐标系".into());
    } else if icontains(&prj_text, "WGS84") || icontains(&prj_text, "WGS_84") {
        info.insert("c".into(), "WGS84坐标系".into());
    }

    if icontains(&prj_text, "Gauss_Kruger") || icontains(&prj_text, "Transverse_Mercator") {
        info.insert("j".into(), "高斯克吕格".into());
    } else if icontains(&prj_text, "Lambert") {
        info.insert("j".into(), "兰伯特".into());
    }

    if icontains(&prj_text, "UNIT[\"Meter") {
        info.insert("u".into(), "米".into());
    } else if icontains(&prj_text, "UNIT[\"Degree") {
        info.insert("u".into(), "度".into());
    }

    // 提取中央经线 → 带号
    for needle in &["Central_Meridian", "Longitude_Of_Origin"] {
        if let Some(pos) = prj_text.find(needle) {
            let after = &prj_text[pos + needle.len()..];
            if let Some(num_start) = after.find(|c: char| c.is_ascii_digit() || c == '-') {
                let num_str: String = after[num_start..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if let Ok(lon) = num_str.parse::<f64>() {
                    let zone = (lon / 3.0).round() as i32;
                    info.insert("z".into(), zone.to_string());
                    info.insert("cm".into(), lon.to_string());
                    info.insert("b".into(), "3".into());
                }
            }
            break;
        }
    }

    Ok((prj_text, info))
}

/// 根据路径基名查找配套的 .dbf、.prj 文件
pub fn find_companion_files(shp_path: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let stem = shp_path.file_stem().unwrap_or_default();
    let dir = shp_path.parent().unwrap_or(Path::new(""));
    let stem_str = stem.to_string_lossy();
    let dbf = dir.join(format!("{}.dbf", stem_str));
    let prj = dir.join(format!("{}.prj", stem_str));
    (
        if dbf.exists() { Some(dbf) } else { None },
        if prj.exists() { Some(prj) } else { None },
    )
}

/// 读取完整 SHP 文件组信息
pub fn read_shp_file_group(shp_path: &Path) -> Result<ShpFileInfo, String> {
    let name = shp_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let (dbf_path, prj_path) = find_companion_files(shp_path);

    // SHP header 校验
    let buf = std::fs::read(shp_path).map_err(|e| format!("读 SHP: {}", e))?;
    if buf.len() < 100 {
        return Err("文件太小，不是有效的 SHP".into());
    }
    let file_code = i32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if file_code != 9994 {
        return Err("不是有效的 SHP 文件".into());
    }
    let st = i32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);
    let shape_type_str = match st {
        1 => "Point",
        3 => "PolyLine",
        5 => "Polygon",
        8 => "MultiPoint",
        _ => "Other",
    }
    .to_string();

    let features = read_shp(shp_path)?;
    let num_features = features.len();

    let (field_names, field_records) = if let Some(ref dbf) = dbf_path {
        read_dbf(dbf).unwrap_or_default()
    } else {
        (Vec::new(), Vec::new())
    };

    let (prj_text, crs_info) = if let Some(ref prj) = prj_path {
        read_prj(prj).unwrap_or((String::new(), HashMap::new()))
    } else {
        (String::new(), HashMap::new())
    };
    let prj_text_opt = if prj_text.is_empty() { None } else { Some(prj_text) };

    Ok(ShpFileInfo {
        name,
        shp_path: shp_path.to_string_lossy().to_string(),
        dbf_path: dbf_path.map(|p| p.to_string_lossy().to_string()),
        prj_path: prj_path.map(|p| p.to_string_lossy().to_string()),
        field_names,
        field_records,
        num_features,
        shape_type: shape_type_str,
        prj_text: prj_text_opt,
        crs_info,
    })
}

/// 写 .prj 文件
pub fn write_prj(path: &Path, crs: &str, band: &str, zone: &str) -> Result<(), String> {
    let zval: f64 = zone.parse().unwrap_or(38.0);
    let band_val: f64 = band.parse().unwrap_or(3.0);

    let (central_meridian, false_easting) = if (band_val - 3.0).abs() < 0.1 {
        // 3-degree Gauss-Kruger
        (zval * 3.0, zval * 1_000_000.0 + 500_000.0)
    } else {
        // 6-degree Gauss-Kruger
        (zval * 6.0 - 3.0, zval * 1_000_000.0 + 500_000.0)
    };

    let (geogcs_name, datum_name, spheroid_name, semi_major, inv_flattening) =
        if crs.contains("2000") || crs.contains("CGCS") {
            ("GCS_China_Geodetic_Coordinate_System_2000", "D_China_2000",
             "CGCS2000", 6378137.0, 298.257222101)
        } else if crs.contains("西安") || crs.contains("Xian") {
            ("GCS_Xian_1980", "D_Xian_1980",
             "Xian_1980", 6378140.0, 298.257)
        } else if crs.contains("北京") || crs.contains("Beijing") {
            ("GCS_Beijing_1954", "D_Beijing_1954",
             "Krasovsky_1940", 6378245.0, 298.3)
        } else {
            ("GCS_WGS_1984", "D_WGS_1984",
             "WGS_1984", 6378137.0, 298.257223563)
        };

    let band_label = if (band_val - 3.0).abs() < 0.1 { "3_Degree" } else { "6_Degree" };
    let zone_int = zval as i32;

    let wkt = format!(
        "PROJCS[\"CGCS2000_{}_GK_Zone_{}\",GEOGCS[\"{}\",DATUM[\"{}\",SPHEROID[\"{}\",{},{}]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]],PROJECTION[\"Gauss_Kruger\"],PARAMETER[\"False_Easting\",{}],PARAMETER[\"False_Northing\",0.0],PARAMETER[\"Central_Meridian\",{}],PARAMETER[\"Scale_Factor\",1.0],PARAMETER[\"Latitude_Of_Origin\",0.0],UNIT[\"Meter\",1.0]]",
        band_label, zone_int,
        geogcs_name, datum_name, spheroid_name, semi_major, inv_flattening,
        false_easting, central_meridian
    );
    std::fs::write(path, wkt).map_err(|e| format!("写 PRJ 失败: {}", e))
}

/// 写完整的 SHP 文件组
pub fn write_shapefile(
    output_dir: &Path,
    stem: &str,
    geometries: &[Vec<(f64, f64)>],
    attributes: &[std::collections::HashMap<String, String>],
    crs: &str,
    band: &str,
    zone: &str,
) -> Result<Vec<String>, String> {
    use shapefile::{ShapeWriter as ShpShapeWriter, PolygonRing, Point as ShpPoint, Polygon as ShpPolygon};

    let mut shp_paths = Vec::new();

    // 1. Write .shp + .shx
    let shp_path = output_dir.join(format!("{}.shp", stem));
    let mut swriter = ShpShapeWriter::from_path(&shp_path)
        .map_err(|e| format!("创建 SHP 写入器失败: {}", e))?;

    for geom in geometries {
        if geom.len() < 3 {
            continue;
        }
        let points: Vec<ShpPoint> = geom
            .iter()
            .map(|&(x, y)| ShpPoint::new(x, y))
            .collect();

        let ring_points = if points.first() != points.last() {
            let mut closed = points.clone();
            closed.push(closed[0]);
            closed
        } else {
            points.clone()
        };

        let ring = PolygonRing::Outer(ring_points);
        let poly = ShpPolygon::new(ring);
        swriter
            .write_shape(&poly)
            .map_err(|e| format!("写 SHP 图形失败: {}", e))?;
    }
    drop(swriter);
    shp_paths.push(shp_path.to_string_lossy().to_string());

    // 2. Write .dbf (manually)
    let dbf_path = output_dir.join(format!("{}.dbf", stem));
    write_dbf_manual(&dbf_path, attributes)?;
    shp_paths.push(dbf_path.to_string_lossy().to_string());

    // 3. Write .prj
    let prj_path = output_dir.join(format!("{}.prj", stem));
    write_prj(&prj_path, crs, band, zone)?;
    shp_paths.push(prj_path.to_string_lossy().to_string());

    Ok(shp_paths)
}

/// 手动写 DBF 文件（支持 GBK 中文编码）
fn write_dbf_manual(
    path: &Path,
    attributes: &[std::collections::HashMap<String, String>],
) -> Result<(), String> {
    let field_defs: Vec<(&str, u8, u8, u8)> = vec![
        ("DKMC", b'C', 50, 0),
        ("DKBH", b'C', 30, 0),
        ("MJ", b'N', 14, 3),
        ("DKYT", b'C', 50, 0),
        ("TFH", b'C', 20, 0),
        ("DLBM", b'C', 10, 0),
    ];

    let num_fields = field_defs.len();
    let header_len: u16 = 32 + (num_fields as u16 * 32) + 1;
    let records_len: u16 = field_defs.iter().map(|(_, _, len, _)| *len as u16).sum::<u16>() + 1;

    let mut buf = Vec::new();

    // Header (32 bytes)
    buf.push(0x03);         // version
    buf.push(26); buf.push(6); buf.push(10);  // date
    let num_records = attributes.len() as u32;
    buf.extend_from_slice(&num_records.to_le_bytes());
    buf.extend_from_slice(&header_len.to_le_bytes());
    buf.extend_from_slice(&records_len.to_le_bytes());
    buf.push(0x7C);         // language driver ID: GBK/Chinese Simplified
    buf.extend_from_slice(&[0u8; 19]);

    let mut offset: u16 = 1;
    for &(name, ftype, len, decimals) in &field_defs {
        let name_bytes = name.as_bytes();
        for j in 0..11 {
            buf.push(if j < name_bytes.len() { name_bytes[j] } else { 0 });
        }
        buf.push(ftype);
        // field offset in DBF is 4 bytes (LE), but only first 2 are used
        buf.extend_from_slice(&(offset as u32).to_le_bytes());
        buf.push(len);
        buf.push(decimals);
        buf.extend_from_slice(&[0u8; 14]);
        offset += len as u16;
    }
    buf.push(0x0D);

    for attr in attributes {
        buf.push(0x20);
        let vals = [
            attr.get("DKMC").map(|s| s.as_str()).unwrap_or(""),
            attr.get("DKBH").map(|s| s.as_str()).unwrap_or(""),
            attr.get("MJ").map(|s| s.as_str()).unwrap_or(""),
            attr.get("DKYT").map(|s| s.as_str()).unwrap_or(""),
            attr.get("TFH").map(|s| s.as_str()).unwrap_or(""),
            attr.get("DLBM").map(|s| s.as_str()).unwrap_or(""),
        ];
        for (i, &(_, _, len, _)) in field_defs.iter().enumerate() {
            // Encode string to GBK; fallback to UTF-8 if encoding fails
            let (encoded, _, had_errors) = encoding_rs::GBK.encode(vals[i]);
            let vb = if had_errors {
                vals[i].as_bytes().to_vec()
            } else {
                encoded.into_owned()
            };
            for j in 0..len as usize {
                buf.push(if j < vb.len() { vb[j] } else { b' ' });
            }
        }
    }
    buf.push(0x1A);
    std::fs::write(path, &buf).map_err(|e| format!("写 DBF 失败: {}", e))
}
