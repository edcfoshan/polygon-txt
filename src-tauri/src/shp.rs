// SHP / DBF / PRJ 文件读写模块
use crate::geometry::{PolygonPart, SurfaceGeometry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

/// 从 SHP 读取的要素信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShpFeature {
    pub points: Vec<(f64, f64)>, // (x, y) = (easting, northing)
    pub surface: SurfaceGeometry,
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
                if let Some(f) = polygon_rings_to_feature(poly.rings(), |p| (p.x, p.y)) {
                    features.push(f);
                }
            }
            shapefile::Shape::PolygonZ(polyz) => {
                if let Some(f) = polygon_rings_to_feature(polyz.rings(), |p| (p.x, p.y)) {
                    features.push(f);
                }
            }
            shapefile::Shape::Polyline(pl) => {
                for part in pl.parts() {
                    let pts: Vec<(f64, f64)> = part.iter().map(|p| (p.x, p.y)).collect();
                    if !pts.is_empty() {
                        let surface = SurfaceGeometry {
                            parts: vec![PolygonPart {
                                exterior: pts.clone(),
                                holes: Vec::new(),
                            }],
                        };
                        features.push(ShpFeature { points: pts, surface });
                    }
                }
            }
            shapefile::Shape::Point(p) => {
                let pts = vec![(p.x, p.y)];
                features.push(ShpFeature {
                    points: pts.clone(),
                    surface: SurfaceGeometry {
                        parts: vec![PolygonPart {
                            exterior: pts,
                            holes: Vec::new(),
                        }],
                    },
                });
            }
            _ => {}
        }
    }
    Ok(features)
}

/// 从 GenericPolygon 的环构建单个 ShpFeature（Point / PointZ 通用）。
/// 洞（Inner ring）挂在最近的外环上；points 取首个外环（与原 Polygon 分支一致）。
fn polygon_rings_to_feature<P>(
    rings: &[shapefile::PolygonRing<P>],
    xy: impl Fn(&P) -> (f64, f64),
) -> Option<ShpFeature> {
    let mut parts = Vec::new();
    let mut current_exterior: Option<Vec<(f64, f64)>> = None;
    let mut current_holes: Vec<Vec<(f64, f64)>> = Vec::new();

    for ring in rings {
        match ring {
            shapefile::PolygonRing::Outer(pts) => {
                if let Some(exterior) = current_exterior.take() {
                    parts.push(PolygonPart {
                        exterior,
                        holes: std::mem::take(&mut current_holes),
                    });
                }
                current_exterior = Some(pts.iter().map(|p| xy(p)).collect());
            }
            shapefile::PolygonRing::Inner(pts) => {
                current_holes.push(pts.iter().map(|p| xy(p)).collect());
            }
        }
    }

    if let Some(exterior) = current_exterior.take() {
        parts.push(PolygonPart {
            exterior,
            holes: current_holes,
        });
    }

    if parts.is_empty() {
        None
    } else {
        let points = parts[0].exterior.clone();
        Some(ShpFeature {
            points,
            surface: SurfaceGeometry { parts },
        })
    }
}

/// 解析 .dbf 文件，返回字段名列表和所有记录
///
/// dbase 0.3.0 crate 硬编码用 ASCII 解码字段名，遇到非 ASCII 字段名（如 ArcGIS 导出的
/// 中文 UTF-8 字段名）会在 `dbase::read()` 内部 `.unwrap()` 处 panic，进而终止进程。
/// 此处用 `catch_unwind` 包裹（依赖 unwind；当前 release profile 未设 panic=abort），
/// panic 或 Err 时回退到自写的手动 DBF 解析器（`read_dbf_manual`）。
pub fn read_dbf(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    // 非 UTF-8（如 GBK 无 .cpg）直接手动解析，避免 dbase 用 UTF-8 误解码 GBK 字段值
    if detect_dbf_encoding(path) != encoding_rs::UTF_8 {
        return read_dbf_manual(path);
    }

    use dbase::FieldValue;

    let dbase_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dbase::read(path).map_err(|e| format!("打开 DBF 失败: {}", e))
    }));

    match dbase_result {
        Ok(Ok(records)) => {
            if records.is_empty() {
                return Ok((Vec::new(), Vec::new()));
            }

            // Collect field names from the first record via iteration
            let mut field_names: Vec<String> = Vec::new();
            // Record implements IntoIterator, producing (FieldName, FieldValue) pairs
            let field_iter: Vec<(String, dbase::FieldValue)> =
                records[0].clone().into_iter().collect();
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
        Ok(Err(e)) => {
            eprintln!("dbase::read 返回错误，回退手动解析: {}", e);
            read_dbf_manual(path)
        }
        Err(_) => {
            eprintln!("dbase::read panic，回退手动解析");
            read_dbf_manual(path)
        }
    }
}

/// 探测 DBF 字段名/值的编码：优先读同名 .cpg 文件；无 .cpg 时采样 Character 字段值
/// 字节探测（合法 UTF-8 → UTF-8，否则 → GBK）。中国大陆非 UTF-8 的 DBF 几乎都是 GBK。
fn detect_dbf_encoding(dbf_path: &Path) -> &'static encoding_rs::Encoding {
    let cpg = dbf_path.with_extension("cpg");
    if let Ok(text) = std::fs::read_to_string(&cpg) {
        let t = text.trim().to_ascii_uppercase();
        if t.contains("UTF-8") || t.contains("UTF8") || t == "65001" {
            return encoding_rs::UTF_8;
        }
        if t.contains("GBK") || t.contains("936") || t.contains("GB2312") {
            return encoding_rs::GBK;
        }
    }
    // 无 .cpg：采样 Character 字段值字节做严格 UTF-8 判定
    if let Ok(data) = std::fs::read(dbf_path) {
        let sample = collect_dbf_char_bytes(&data);
        if !sample.is_empty() {
            return if std::str::from_utf8(&sample).is_ok() {
                encoding_rs::UTF_8
            } else {
                encoding_rs::GBK
            };
        }
    }
    encoding_rs::UTF_8
}

/// 收集 DBF 所有 Character('C') 字段值字节（用于无 .cpg 时的编码探测）。
/// 解析逻辑与 read_dbf_manual 一致（dBase III+ 标准头）。
fn collect_dbf_char_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() < 32 {
        return Vec::new();
    }
    let num_records = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let header_len = u16::from_le_bytes([data[8], data[9]]) as usize;
    let record_len = u16::from_le_bytes([data[10], data[11]]) as usize;

    // 收集 Character 字段在记录中的偏移与长度
    let mut char_fields: Vec<(usize, usize)> = Vec::new(); // (offset_in_record, len)
    let mut off = 32usize;
    let mut field_off: usize = 1; // 跳过 deletion flag
    while off + 32 <= header_len && off < data.len() && data[off] != 0x0D {
        let ftype = data[off + 11];
        let flen = data[off + 16] as usize;
        if ftype == b'C' {
            char_fields.push((field_off, flen));
        }
        field_off = field_off.saturating_add(flen);
        off += 32;
    }

    let mut sample = Vec::new();
    let mut rec_off = header_len;
    for _ in 0..num_records {
        if rec_off + record_len > data.len() {
            break;
        }
        if data[rec_off] != b'*' {
            for &(foff, flen) in &char_fields {
                let start = rec_off + foff;
                let end = (start + flen).min(data.len());
                if start < end {
                    sample.extend_from_slice(&data[start..end]);
                }
            }
        }
        rec_off += record_len;
    }
    sample
}

/// 手动解析 dBase III+ DBF（作为 dbase::read panic/Err 时的回退）。
/// 仅覆盖 C/N/F/I/L/D 字段类型，字段名按 detect_dbf_encoding 解码。
fn read_dbf_manual(dbf_path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let data = std::fs::read(dbf_path).map_err(|e| format!("读 DBF 失败: {}", e))?;
    if data.len() < 32 {
        return Err("DBF 文件过小".into());
    }
    let num_records = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let header_len = u16::from_le_bytes([data[8], data[9]]) as usize;
    let record_len = u16::from_le_bytes([data[10], data[11]]) as usize;
    let encoding = detect_dbf_encoding(dbf_path);

    // 字段描述符：offset 32 起，每项 32 字节；name(11) / type(+11) / len(+16)；遇 0x0D 终止
    let mut fields: Vec<(String, u8, usize)> = Vec::new();
    let mut off = 32usize;
    while off + 32 <= header_len && off < data.len() && data[off] != 0x0D {
        let name_bytes = &data[off..off + 11];
        let name_trim = name_bytes.split(|&b| b == 0).next().unwrap_or(&[]);
        let name = encoding.decode(name_trim).0.into_owned();
        let ftype = data[off + 11];
        let flen = data[off + 16] as usize;
        fields.push((name, ftype, flen));
        off += 32;
    }

    // 字段名过滤条件与 read_dbf 的 dbase 分支完全一致
    let filtered: Vec<usize> = fields
        .iter()
        .enumerate()
        .filter(|(_, (name, _, _))| {
            name != "DeletionFlag"
                && !name.starts_with("FID")
                && !name.eq_ignore_ascii_case("SHAPE")
                && !name.starts_with("SHAPE_LENG")
                && !name.starts_with("SHAPE_AREA")
                && !name.starts_with("OBJECTID")
        })
        .map(|(i, _)| i)
        .collect();
    let field_names: Vec<String> = filtered.iter().map(|&i| fields[i].0.clone()).collect();

    let mut string_records = Vec::new();
    let mut rec_off = header_len;
    for _ in 0..num_records {
        if rec_off + record_len > data.len() {
            break;
        }
        let deletion = data[rec_off];
        if deletion != b'*' {
            let mut row = Vec::new();
            let mut field_off = rec_off + 1; // 跳过 deletion flag
            for (i, (_, ftype, flen)) in fields.iter().enumerate() {
                if field_off + flen > data.len() {
                    break;
                }
                let raw = &data[field_off..field_off + *flen];
                if filtered.contains(&i) {
                    row.push(decode_field_value(raw, *ftype, encoding));
                }
                field_off += flen;
            }
            string_records.push(row);
        }
        rec_off += record_len;
    }

    Ok((field_names, string_records))
}

/// 手动解析时解码单个 DBF 字段值（与 dbase crate 的 to_string 行为对齐）。
fn decode_field_value(
    raw: &[u8],
    ftype: u8,
    encoding: &'static encoding_rs::Encoding,
) -> String {
    match ftype {
        b'C' => {
            let (cow, _, _) = encoding.decode(raw);
            cow.into_owned().trim_end().trim_end_matches('\u{0}').to_string()
        }
        b'N' | b'F' => std::str::from_utf8(raw).unwrap_or("").trim().to_string(),
        b'I' => {
            if raw.len() >= 4 {
                i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]).to_string()
            } else {
                String::new()
            }
        }
        b'L' => match raw.first().copied().unwrap_or(b' ') {
            b'T' | b'Y' | b't' | b'y' => "是".to_string(),
            b'F' | b'N' | b'f' | b'n' => "否".to_string(),
            _ => String::new(),
        },
        b'D' => std::str::from_utf8(raw).unwrap_or("").trim().to_string(),
        _ => {
            let (cow, _, _) = encoding.decode(raw);
            cow.into_owned().trim_end().to_string()
        }
    }
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
    Ok((prj_text.clone(), parse_prj_text(&prj_text)))
}

/// 解析 PRJ / WKT 文本 → crs_info（c 坐标系 / j 投影 / u 单位 / b 分带 / z 带号 / cm 中央经线）。
/// SHP 的 .prj 与 GDB 图层内嵌 srs_wkt 共用此解析。
pub fn parse_prj_text(prj_text: &str) -> HashMap<String, String> {
    let prj_text = prj_text.trim();
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
    } else if icontains(&prj_text, "WGS84") || icontains(&prj_text, "WGS_84") || icontains(&prj_text, "WGS_1984") {
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
    // ESRI 命名约定：PROJCS 名 "GK_Zone_39" = 带带号版（坐标含前缀）；"GK_CM_117E" = 仅中央经线版（无带号，
    // 坐标自然值）——CM 版不得写 z，否则前端「带号前缀」智能默认会把无带号坐标系误判为有带号。
    // 参数名大小写不敏感：ArcGIS 导出为 CamelCase（Central_Meridian），部分工具为全小写（central_meridian）
    let lower_prj = prj_text.to_lowercase();
    for needle in &["central_meridian", "longitude_of_origin"] {
        if let Some(pos) = lower_prj.find(needle) {
            let after = &prj_text[pos + needle.len()..];
            if let Some(num_start) = after.find(|c: char| c.is_ascii_digit() || c == '-') {
                let num_str: String = after[num_start..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if let Ok(lon) = num_str.parse::<f64>() {
                    info.insert("cm".into(), lon.to_string());
                    info.insert("b".into(), "3".into());
                    if !icontains(&prj_text, "GK_CM_") {
                        let zone = zone_number_from_prj_name(&prj_text)
                            .unwrap_or_else(|| (lon / 3.0).round() as i32);
                        info.insert("z".into(), zone.to_string());
                    }
                }
            }
            break;
        }
    }

    info
}

/// 从 PROJCS 名称提取显式带号（如 "GK_Zone_39" / "GK_3_Degree_Zone_39" → 39）；无则 None
fn zone_number_from_prj_name(prj_text: &str) -> Option<i32> {
    let lower = prj_text.to_lowercase();
    let pos = lower.find("zone_")?;
    let after = &prj_text[pos + "zone_".len()..];
    let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse::<i32>().ok()
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
        15 => "PolygonZ",
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

    // CRS label for PROJCS name (instead of hardcoded "CGCS2000_")
    let crs_label = if crs.contains("2000") || crs.contains("CGCS") {
        "CGCS2000"
    } else if crs.contains("西安") || crs.contains("Xian") {
        "Xian_1980"
    } else if crs.contains("北京") || crs.contains("Beijing") {
        "Beijing_1954"
    } else {
        "WGS_1984"
    };

    let wkt = format!(
        "PROJCS[\"{}_{}_GK_Zone_{}\",GEOGCS[\"{}\",DATUM[\"{}\",SPHEROID[\"{}\",{},{}]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]],PROJECTION[\"Gauss_Kruger\"],PARAMETER[\"False_Easting\",{}],PARAMETER[\"False_Northing\",0.0],PARAMETER[\"Central_Meridian\",{}],PARAMETER[\"Scale_Factor\",1.0],PARAMETER[\"Latitude_Of_Origin\",0.0],UNIT[\"Meter\",1.0]]",
        crs_label, band_label, zone_int,
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
    let surfaces: Vec<SurfaceGeometry> = geometries
        .iter()
        .map(|geom| SurfaceGeometry {
            parts: vec![PolygonPart {
                exterior: geom.clone(),
                holes: Vec::new(),
            }],
        })
        .collect();
    write_shapefile_structured(output_dir, stem, &surfaces, attributes, crs, band, zone)
}

pub fn write_shapefile_structured(
    output_dir: &Path,
    stem: &str,
    geometries: &[SurfaceGeometry],
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

    for surface in geometries {
        let mut rings = Vec::new();
        for part in &surface.parts {
            if part.exterior.len() < 3 {
                continue;
            }
            let exterior_points: Vec<ShpPoint> = part
                .exterior
                .iter()
                .map(|&(x, y)| ShpPoint::new(x, y))
                .collect();
            let exterior_points = if exterior_points.first() != exterior_points.last() {
                let mut closed = exterior_points.clone();
                closed.push(closed[0]);
                closed
            } else {
                exterior_points
            };
            rings.push(PolygonRing::Outer(exterior_points));

            for hole in &part.holes {
                if hole.len() < 3 {
                    continue;
                }
                let hole_points: Vec<ShpPoint> = hole
                    .iter()
                    .map(|&(x, y)| ShpPoint::new(x, y))
                    .collect();
                let hole_points = if hole_points.first() != hole_points.last() {
                    let mut closed = hole_points.clone();
                    closed.push(closed[0]);
                    closed
                } else {
                    hole_points
                };
                rings.push(PolygonRing::Inner(hole_points));
            }
        }
        if rings.is_empty() {
            continue;
        }
        let poly = ShpPolygon::with_rings(rings);
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

    // 4. Write .cpg (UTF-8，与 ArcPy 官方输出一致，ArcMap 10.x 兼容)
    let cpg_path = output_dir.join(format!("{}.cpg", stem));
    std::fs::write(&cpg_path, "UTF-8").ok();
    shp_paths.push(cpg_path.to_string_lossy().to_string());

    Ok(shp_paths)
}

/// 手动写 DBF 文件（UTF-8 编码，与 ArcPy 官方输出一致）
fn write_dbf_manual(
    path: &Path,
    attributes: &[std::collections::HashMap<String, String>],
) -> Result<(), String> {
    // 字段定义：出现 FIELD\d+ 键 → 动态模式（高级格式按元数据行顺序编号）；
    // 否则基础 6 字段。LUJIN/MINGC 仅在任一属性行含对应 key 时附加（勾选才有）
    let mut field_defs: Vec<(String, u8, u8, u8)> =
        if attributes.iter().any(|a| a.keys().any(|k| k.starts_with("FIELD"))) {
            // 动态模式：FIELD1~FIELDn（n = 所有记录中出现过的最大序号）
            let max_n = attributes
                .iter()
                .flat_map(|a| a.keys())
                .filter_map(|k| k.strip_prefix("FIELD").and_then(|s| s.parse::<u32>().ok()))
                .max()
                .unwrap_or(0);
            (1..=max_n)
                .map(|i| {
                    let name = format!("FIELD{}", i);
                    let vals: Vec<&str> = attributes
                        .iter()
                        .filter_map(|a| a.get(&name).map(|s| s.as_str()))
                        .collect();
                    // 类型推断：该列所有非空值均可解析为数字 → N(19, 观测最大小数位)，否则 C
                    let numeric = !vals.is_empty() && vals.iter().all(|v| v.parse::<f64>().is_ok());
                    if numeric {
                        let max_dec = vals
                            .iter()
                            .map(|v| v.find('.').map_or(0, |p| v.len() - p - 1))
                            .max()
                            .unwrap_or(0)
                            .min(10) as u8;
                        (name, b'N', 19, max_dec)
                    } else {
                        let max_len = vals
                            .iter()
                            .map(|v| v.as_bytes().len())
                            .max()
                            .unwrap_or(0)
                            .max(50)
                            .min(254) as u8;
                        (name, b'C', max_len, 0)
                    }
                })
                .collect()
        } else {
            vec![
                ("DKMC".to_string(), b'C', 50, 0),
                ("DKBH".to_string(), b'C', 30, 0),
                ("MJ".to_string(), b'N', 14, 3),
                ("DKYT".to_string(), b'C', 50, 0),
                ("TFH".to_string(), b'C', 20, 0),
                ("DLBM".to_string(), b'C', 10, 0),
            ]
        };
    if attributes.iter().any(|a| a.contains_key("LUJIN")) {
        field_defs.push(("LUJIN".to_string(), b'C', 254, 0));
    }
    if attributes.iter().any(|a| a.contains_key("MINGC")) {
        field_defs.push(("MINGC".to_string(), b'C', 100, 0));
    }

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
    // Bytes 12-28: reserved (17 bytes)
    buf.extend_from_slice(&[0u8; 17]);
    buf.push(0x00);         // byte 29: LDID=0 让 .cpg 主导，避免非标准值误导 ArcMap
    buf.extend_from_slice(&[0u8; 2]);  // bytes 30-31: reserved

    let mut offset: u16 = 1;
    for (name, ftype, len, decimals) in &field_defs {
        let name_bytes = name.as_bytes();
        for j in 0..11 {
            buf.push(if j < name_bytes.len() { name_bytes[j] } else { 0 });
        }
        buf.push(*ftype);
        // field offset in DBF is 4 bytes (LE), but only first 2 are used
        buf.extend_from_slice(&(offset as u32).to_le_bytes());
        buf.push(*len);
        buf.push(*decimals);
        buf.extend_from_slice(&[0u8; 14]);
        offset += *len as u16;
    }
    buf.push(0x0D);

    for attr in attributes {
        buf.push(0x20);
        // 按 field_defs 顺序取值（缺失 key 视为空串）；字段值直接写 UTF-8 字节
        for (field_name, _, len, _) in &field_defs {
            let val = attr.get(field_name.as_str()).map(|s| s.as_str()).unwrap_or("");
            let vb = val.as_bytes();
            for j in 0..*len as usize {
                buf.push(if j < vb.len() { vb[j] } else { b' ' });
            }
        }
    }
    buf.push(0x1A);
    std::fs::write(path, &buf).map_err(|e| format!("写 DBF 失败: {}", e))
}

#[cfg(test)]
mod prj_tests {
    use super::*;

    fn write_tmp_prj(content: &str, tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("jisig_prj_test_{}.prj", tag));
        std::fs::write(&p, content).unwrap();
        p
    }

    const PRJ_BODY: &str = "GEOGCS[\"GCS_China_2000\",DATUM[\"D_China_2000\",SPHEROID[\"CGCS2000\",6378137.0,298.257222101]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]],PROJECTION[\"Gauss_Kruger\"],PARAMETER[\"False_Easting\",500000.0],PARAMETER[\"False_Northing\",0.0],PARAMETER[\"Central_Meridian\",117.0],PARAMETER[\"Scale_Factor\",1.0],PARAMETER[\"Latitude_Of_Origin\",0.0],UNIT[\"Meter\",1.0]]";

    /// CM 版 PRJ（如 CGCS2000_3_Degree_GK_CM_117E）：无带号，z 必须缺失，
    /// 否则前端「带号前缀」智能默认会把无带号坐标系误判为有带号
    #[test]
    fn read_prj_cm_version_has_no_zone() {
        let p = write_tmp_prj(&format!("PROJCS[\"CGCS2000_3_Degree_GK_CM_117E\",{}]", PRJ_BODY), "cm");
        let (_, info) = read_prj(&p).unwrap();
        assert!(!info.contains_key("z"), "CM 版 PRJ 不应带带号: {:?}", info);
        assert_eq!(info.get("cm").map(|s| s.as_str()), Some("117"));
        assert_eq!(info.get("b").map(|s| s.as_str()), Some("3"));
        assert_eq!(info.get("u").map(|s| s.as_str()), Some("米"));
    }


    /// 小写参数名 PRJ（QGIS 等工具导出）：central_meridian 小写也要能提取带号/CM
    #[test]
    fn read_prj_lowercase_params() {
        let p = write_tmp_prj("PROJCS[\"CGCS2000_3_Degree_GK_Zone_38\",GEOGCS[\"GCS_China_Geodetic_Coordinate_System_2000\",DATUM[\"D_China_2000\",SPHEROID[\"CGCS2000\",6378137.0,298.257222101]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]],PROJECTION[\"Transverse_Mercator\"],PARAMETER[\"false_easting\",38500000.0],PARAMETER[\"false_northing\",0.0],PARAMETER[\"central_meridian\",114.0],PARAMETER[\"scale_factor\",1.0],PARAMETER[\"latitude_of_origin\",0.0],UNIT[\"Meter\",1.0]]", "lowercase");
        let (_, info) = read_prj(&p).unwrap();
        assert_eq!(info.get("c").map(|s| s.as_str()), Some("2000国家大地坐标系"));
        assert_eq!(info.get("z").map(|s| s.as_str()), Some("38"), "小写 central_meridian 也要能推出带号: {:?}", info);
        assert_eq!(info.get("cm").map(|s| s.as_str()), Some("114"));
        assert_eq!(info.get("b").map(|s| s.as_str()), Some("3"));
        assert_eq!(info.get("u").map(|s| s.as_str()), Some("米"));
    }

    /// Zone 版 PRJ（如 CGCS2000_3_Degree_GK_Zone_39）：z 取名称中的显式带号
    #[test]
    fn read_prj_zone_version_reads_zone_from_name() {
        let p = write_tmp_prj(&format!("PROJCS[\"CGCS2000_3_Degree_GK_Zone_39\",{}]", PRJ_BODY), "zone");
        let (_, info) = read_prj(&p).unwrap();
        assert_eq!(info.get("z").map(|s| s.as_str()), Some("39"));
        assert_eq!(info.get("cm").map(|s| s.as_str()), Some("117"));
    }
}
