// GDB 读写模块 — 纯 Rust 实现
// 读取: geonative-filegdb (优先) + 手动回退
// 写入: 最小化 OpenFileGDB (仅 Polygon)

use geonative_core::{Geometry as GeoGeom, Value};
use geonative_filegdb as fgdb;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

// ─── 手动回退用图层条目 ───

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct LayerEntry {
    name: String,
    fid: i64,
    physical_filename: String,
}

impl LayerEntry {
    fn table_path(&self, gdb_dir: &Path) -> PathBuf {
        gdb_dir.join(&self.physical_filename)
    }
    fn tablx_path(&self, gdb_dir: &Path) -> PathBuf {
        let mut p = self.table_path(gdb_dir);
        p.set_extension("gdbtablx");
        p
    }
}

// ─── 读取 ───

/// 打开并读取 GDB（双层策略：库优先 → 手动回退）
pub fn read_gdb(path: &Path) -> Result<GdbFileInfo, String> {
    if !path.exists() {
        return Err(format!("GDB 路径不存在: {}", path.display()));
    }
    if !path.join("a00000001.gdbtable").exists() {
        return Err(format!(
            "不是有效的 FileGDB 目录（缺少 a00000001.gdbtable）: {}",
            path.display()
        ));
    }

    // 快速路径：优先使用 geonative-filegdb 库
    match fgdb::open(path) {
        Ok(gdb) => read_gdb_via_library(gdb, path),
        Err(e) => {
            let err_msg = e.to_string();
            // 若因 version / malformed 错误失败，回退到手动解析
            if err_msg.contains("version") || err_msg.contains("malformed") {
                eprintln!(
                    "GDB 库打开失败({})，切换到手动解析回退方案…",
                    err_msg
                );
                read_gdb_fallback(path)
            } else {
                Err(format!(
                    "打开 GDB 失败: {}。\n请确认 .gdb 目录完整且版本受支持（FGDB 10.x / ArcGIS Pro）。",
                    err_msg
                ))
            }
        }
    }
}

/// 库路径：通过 geonative-filegdb 正常读取
fn read_gdb_via_library(
    gdb: fgdb::Geodatabase,
    path: &Path,
) -> Result<GdbFileInfo, String> {
    let layer_infos = gdb.layers();

    if layer_infos.is_empty() {
        return Err("GDB 中未找到可用图层".to_string());
    }

    let mut layers = Vec::new();
    let mut all_features = Vec::new();
    let mut all_field_names = Vec::new();

    for info in layer_infos.iter() {
        match gdb.layer(&info.name) {
            Ok(layer) => {
                let schema = layer.schema();
                let field_names: Vec<String> =
                    schema.fields.iter().map(|f| f.name.clone()).collect();

                let mut features = Vec::new();
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

                    if let Some(ref geom) = feature.geometry {
                        extract_coords(geom, &mut gdb_feat.points);
                    }

                    for (i, val) in feature.attributes.iter().enumerate() {
                        if i < field_names.len() {
                            let attr_str = value_to_string(val);
                            gdb_feat
                                .attributes
                                .insert(field_names[i].clone(), attr_str);
                        }
                    }
                    features.push(gdb_feat);
                }

                let geom_type = infer_geom_type(&features);

                layers.push(GdbLayerInfo {
                    name: info.name.clone(),
                    field_names: field_names.clone(),
                    num_features: features.len(),
                    geometry_type: geom_type.to_string(),
                });
                all_features.push(features);
                all_field_names.push(field_names);
            }
            Err(e) => {
                eprintln!("跳过图层 {}: {}", info.name, e);
            }
        }
    }

    if layers.is_empty() {
        return Err("GDB 中所有图层均无法读取".to_string());
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

// ─── 手动回退路径 ───

/// 回退路径：手动解析 catalog + 逐层安全打开
fn read_gdb_fallback(path: &Path) -> Result<GdbFileInfo, String> {
    let entries = parse_catalog_manual(path)?;

    if entries.is_empty() {
        return Err("GDB 中未找到可用图层（手动解析）".to_string());
    }

    let mut layers = Vec::new();
    let mut all_features = Vec::new();
    let mut all_field_names = Vec::new();

    for entry in &entries {
        match read_layer_manual(path, entry) {
            Ok((layer_info, features, field_names)) => {
                layers.push(layer_info);
                all_features.push(features);
                all_field_names.push(field_names);
            }
            Err(e) => {
                eprintln!("跳过图层 {}: {}", entry.name, e);
            }
        }
    }

    if layers.is_empty() {
        return Err("GDB 中所有图层均无法读取（手动解析）".to_string());
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

/// Task 1: 手动解析 GDB 系统目录（a00000001.gdbtable）
fn parse_catalog_manual(dir: &Path) -> Result<Vec<LayerEntry>, String> {
    let cat_bytes = std::fs::read(dir.join("a00000001.gdbtable"))
        .map_err(|e| format!("读取 catalog table 失败: {}", e))?;
    let tx_bytes = std::fs::read(dir.join("a00000001.gdbtablx"))
        .map_err(|e| format!("读取 catalog index 失败: {}", e))?;

    // 使用库的 read_catalog 公开 API 解析目录（a00000001 版本应为 3，库能正常处理）
    let layer_infos = fgdb::read_catalog(&cat_bytes, &tx_bytes)
        .map_err(|e| format!("解析 GDB 系统目录失败: {}", e))?;

    Ok(layer_infos
        .into_iter()
        .map(|li| LayerEntry {
            name: li.name,
            fid: li.fid,
            physical_filename: li.physical_filename,
        })
        .collect())
}

/// Task 2: 逐层安全打开 — 使用库底层 API，失败不阻塞其他图层
fn read_layer_manual(
    dir: &Path,
    entry: &LayerEntry,
) -> Result<(GdbLayerInfo, Vec<GdbFeature>, Vec<String>), String> {
    let table_bytes = std::fs::read(entry.table_path(dir))
        .map_err(|e| format!("读取图层文件失败: {}", e))?;
    let tx_bytes = std::fs::read(entry.tablx_path(dir))
        .map_err(|e| format!("读取图层索引失败: {}", e))?;

    let table = fgdb::Table::parse(&table_bytes)
        .map_err(|e| format!("解析图层表结构失败: {}", e))?;
    let tablx = fgdb::Tablx::parse(&tx_bytes)
        .map_err(|e| format!("解析图层索引失败: {}", e))?;

    let schema = &table.field_section;

    // 用户字段名（排除 OBJECTID 和 Geometry）
    let field_names: Vec<String> = schema
        .fields
        .iter()
        .filter(|f| {
            f.ty != fgdb::FieldTypeCode::ObjectId && f.ty != fgdb::FieldTypeCode::Geometry
        })
        .map(|f| f.name.clone())
        .collect();

    let geom_field_idx = schema.geometry_field_index();

    let mut features = Vec::new();
    for (row_idx, offset) in tablx.iter_present() {
        let fid = (row_idx as i64) + 1;

        let blob = match fgdb::slice_row_blob(&table_bytes, offset) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("跳过图层 {} 行 {}: {}", entry.name, row_idx, e);
                continue;
            }
        };

        let row = match fgdb::decode_row_blob(blob, fid, schema) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("跳过图层 {} 行 {}: {}", entry.name, row_idx, e);
                continue;
            }
        };

        let mut gdb_feat = GdbFeature {
            points: Vec::new(),
            attributes: HashMap::new(),
        };

        // 解码几何
        if let (Some(geom_blob), Some(gidx)) = (&row.geometry_blob, geom_field_idx) {
            if let Some(meta) = schema.fields[gidx].geometry.as_ref() {
                if let Ok(geom) = fgdb::decode_shape_buffer(geom_blob, meta) {
                    extract_coords(&geom, &mut gdb_feat.points);
                }
            }
        }

        // 解码属性
        for (i, val) in row.values.iter().enumerate() {
            if Some(i) == geom_field_idx
                || schema.fields[i].ty == fgdb::FieldTypeCode::ObjectId
            {
                continue;
            }
            let field_name = &schema.fields[i].name;
            gdb_feat
                .attributes
                .insert(field_name.clone(), value_to_string(val));
        }

        features.push(gdb_feat);
    }

    let geom_type = infer_geom_type(&features);

    let layer_info = GdbLayerInfo {
        name: entry.name.clone(),
        field_names: field_names.clone(),
        num_features: features.len(),
        geometry_type: geom_type.to_string(),
    };

    Ok((layer_info, features, field_names))
}

// ─── 通用辅助 ───

/// 将 geonative_core::Value 转为字符串
fn value_to_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Int32(n) => n.to_string(),
        Value::Int64(n) => n.to_string(),
        Value::Int16(n) => n.to_string(),
        Value::Float32(f) => f.to_string(),
        Value::Float64(f) => f.to_string(),
        Value::Bool(b) => {
            if *b {
                "是".to_string()
            } else {
                "否".to_string()
            }
        }
        Value::DateTime(d) => d.to_string(),
        _ => String::new(),
    }
}

/// 根据要素列表推断几何类型
fn infer_geom_type(features: &[GdbFeature]) -> &str {
    if features.is_empty() {
        return "Unknown";
    }
    match features[0].points.len() {
        0 => "Unknown",
        1 => "Point",
        n if n > 2 => "Polygon",
        _ => "PolyLine",
    }
}

/// 从 Geometry 树中提取坐标（仅外环）
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
        }
        GeoGeom::MultiPolygon(mp) => {
            for poly in mp {
                for c in &poly.exterior.coords {
                    out.push((c.x, c.y));
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

// ─── 编码工具 ───

/// LEB128 无符号变长整数
fn enc_varuint(buf: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        buf.push(((v & 0x7F) as u8) | 0x80);
        v >>= 7;
    }
    buf.push(v as u8);
}

/// FileGDB 有符号变长整数（首字节 bit6 = 符号位）
fn enc_varint(buf: &mut Vec<u8>, v: i64) {
    let (mag, sign_bit) = if v < 0 {
        ((-v) as u64, 0x40u8)
    } else {
        (v as u64, 0u8)
    };
    let lo6 = (mag & 0x3F) as u8;
    let rest = mag >> 6;
    if rest == 0 {
        buf.push(lo6 | sign_bit);
    } else {
        buf.push(lo6 | sign_bit | 0x80);
        let mut x = rest;
        while x >= 0x80 {
            buf.push(((x & 0x7F) as u8) | 0x80);
            x >>= 7;
        }
        buf.push(x as u8);
    }
}

fn w_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn w_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn w_i16(buf: &mut Vec<u8>, v: i16) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn w_f64(buf: &mut Vec<u8>, v: f64) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn w_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

/// 写 u32 LE 到 buf 指定偏移
#[allow(dead_code)]
fn patch_u32(buf: &mut [u8], offset: usize, v: u32) {
    buf[offset..offset + 4].copy_from_slice(&v.to_le_bytes());
}
fn patch_i64(buf: &mut [u8], offset: usize, v: i64) {
    buf[offset..offset + 8].copy_from_slice(&v.to_le_bytes());
}

/// UTF-16LE 编码，返回字节数
fn enc_utf16le(buf: &mut Vec<u8>, s: &str) -> usize {
    let u16s: Vec<u16> = s.encode_utf16().collect();
    for c in &u16s {
        buf.extend_from_slice(&c.to_le_bytes());
    }
    u16s.len() * 2
}

// ─── Polygon Shape Buffer 编码 ───

/// 编码单个 Polygon 的 shape buffer（Esri CW 外环约定）
fn encode_polygon_shape(coords: &[(f64, f64)], xyscale: f64, xorigin: f64, yorigin: f64) -> Vec<u8> {
    // 去掉闭合点（与首点相同）
    let pts: Vec<(f64, f64)> = if coords.len() >= 2
        && (coords[0].0 - coords.last().unwrap().0).abs() < 1e-12
        && (coords[0].1 - coords.last().unwrap().1).abs() < 1e-12
    {
        coords[..coords.len() - 1].to_vec()
    } else {
        coords.to_vec()
    };

    if pts.len() < 3 {
        return Vec::new();
    }

    // 确保 Esri 外环 = CW（有向面积 < 0）
    let area = signed_area_2d(&pts);
    let ring = if area >= 0.0 {
        pts.iter().rev().copied().collect::<Vec<_>>()
    } else {
        pts
    };

    let n = ring.len();
    let mut buf = Vec::with_capacity(n * 6);

    enc_varuint(&mut buf, 5); // Polygon type
    enc_varuint(&mut buf, n as u64); // nPoints
    enc_varuint(&mut buf, 1); // nParts = 1

    // BBox (4 × varuint): xmin_q, ymin_q, dx_q, dy_q
    let qx: Vec<i64> = ring.iter().map(|p| ((p.0 - xorigin) * xyscale).round() as i64).collect();
    let qy: Vec<i64> = ring.iter().map(|p| ((p.1 - yorigin) * xyscale).round() as i64).collect();
    let (xmin_q, xmax_q) = qx.iter().copied().fold((i64::MAX, i64::MIN), |a, v| (a.0.min(v), a.1.max(v)));
    let (ymin_q, ymax_q) = qy.iter().copied().fold((i64::MAX, i64::MIN), |a, v| (a.0.min(v), a.1.max(v)));
    enc_varuint(&mut buf, xmin_q.max(0) as u64);
    enc_varuint(&mut buf, ymin_q.max(0) as u64);
    enc_varuint(&mut buf, (xmax_q - xmin_q) as u64);
    enc_varuint(&mut buf, (ymax_q - ymin_q) as u64);

    // 单 part → 无 part count 字段（nParts-1 = 0）
    // Delta 编码坐标
    let mut px = 0i64;
    let mut py = 0i64;
    for i in 0..n {
        let dx = qx[i] - px;
        let dy = qy[i] - py;
        enc_varint(&mut buf, dx);
        enc_varint(&mut buf, dy);
        px = qx[i];
        py = qy[i];
    }
    buf
}

fn signed_area_2d(pts: &[(f64, f64)]) -> f64 {
    let n = pts.len();
    if n < 3 { return 0.0; }
    let mut s = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        s += pts[i].0 * pts[j].1 - pts[j].0 * pts[i].1;
    }
    s * 0.5
}

// ─── 字段描述符写入 ───

/// 写 ObjectId 字段描述
fn write_fd_objectid(buf: &mut Vec<u8>, name: &str) {
    let n16 = enc_utf16le(&mut Vec::new(), name);
    w_u8(buf, (n16 / 2) as u8);
    enc_utf16le(buf, name);
    w_u8(buf, 0); // alias len
    w_u8(buf, 6); // type = ObjectId
    w_u8(buf, 4); // width = 4 (32-bit)
    w_u8(buf, 2); // flag = constant 2
}

/// 写 Geometry 字段描述（Polygon，无 Z/M）
fn write_fd_geometry(buf: &mut Vec<u8>, name: &str, wkt: &str, xorigin: f64, yorigin: f64, xyscale: f64) {
    let n16 = enc_utf16le(&mut Vec::new(), name);
    w_u8(buf, (n16 / 2) as u8);
    enc_utf16le(buf, name);
    w_u8(buf, 0);
    w_u8(buf, 7); // type = Geometry
    // GeomFieldMeta
    w_u8(buf, 0); // zero byte
    w_u8(buf, 7); // flag
    let wkt_buf: Vec<u16> = wkt.encode_utf16().collect();
    let wkt_byte_len = wkt_buf.len() * 2;
    buf.extend_from_slice(&(wkt_byte_len as u16).to_le_bytes());
    for c in &wkt_buf {
        buf.extend_from_slice(&c.to_le_bytes());
    }
    w_u8(buf, 0x01); // sub-flags (bit0 set)
    w_f64(buf, xorigin);
    w_f64(buf, yorigin);
    w_f64(buf, xyscale);
    w_f64(buf, 0.001); // xytolerance
    // extent xy (4 × f64)
    for _ in 0..4 { w_f64(buf, 0.0); }
    // grid resolutions
    w_u8(buf, 0); // zero byte
    w_u32(buf, 1); // 1 grid
    w_f64(buf, 1.0); // resolution
}

/// 写 String 字段描述
fn write_fd_string(buf: &mut Vec<u8>, name: &str, max_len: u32) {
    let n16 = enc_utf16le(&mut Vec::new(), name);
    w_u8(buf, (n16 / 2) as u8);
    enc_utf16le(buf, name);
    w_u8(buf, 0);
    w_u8(buf, 4); // type = String
    w_u32(buf, max_len);
    w_u8(buf, 0x01); // flag = nullable
    w_u8(buf, 0); // default len = 0
}

/// 写 Float64 字段描述
fn write_fd_float64(buf: &mut Vec<u8>, name: &str) {
    let n16 = enc_utf16le(&mut Vec::new(), name);
    w_u8(buf, (n16 / 2) as u8);
    enc_utf16le(buf, name);
    w_u8(buf, 0);
    w_u8(buf, 3); // type = Float64
    w_u8(buf, 8); // width
    w_u8(buf, 0x01); // nullable
    w_u8(buf, 0); // no default
}

// ─── null bitmap 辅助 ───

#[allow(dead_code)]
fn test_bit(bitmap: &[u8], idx: usize) -> bool {
    bitmap[idx / 8] & (1u8 << (idx % 8)) != 0
}

// ─── gdbtablx 索引写入 ───

fn build_gdbtablx(row_offsets: &[u64]) -> Vec<u8> {
    let n = row_offsets.len();
    let osz = 4u32;
    let mut buf = Vec::new();
    w_u32(&mut buf, 3); // version
    w_u32(&mut buf, 1); // n_1024_blocks
    w_i32(&mut buf, n as i32); // total_records
    w_u32(&mut buf, osz); // offset_size
    for i in 0..1024 {
        let v = if i < n { row_offsets[i] } else { 0 };
        buf.extend_from_slice(&(v as u32).to_le_bytes());
    }
    // trailer
    w_u32(&mut buf, 0); // nBitmapInt32Words
    w_u32(&mut buf, 1); // nBitsForBlockMap
    w_u32(&mut buf, 1); // n1024BlocksBis
    w_u32(&mut buf, 0); // nLeadingNonZero
    buf
}

// ─── 系统目录写入 ───

fn write_system_catalog(gdb_path: &Path, fc_name: &str) -> Result<(), String> {
    let items = vec![
        (1i16, "GDB_SystemCatalog".to_string()),
        (2i16, fc_name.to_string()),
        (3i16, "spTimestamps".to_string()),
        (4i16, "GDB_Items".to_string()),
        (5i16, "GDB_ItemTypes".to_string()),
    ];

    let _strings_utf8 = true;
    let flags = 0x100u32; // UTF-8 strings, no geometry

    // Field descriptors (OBJECTID + Name)
    let mut field_sec = Vec::new();
    w_u32(&mut field_sec, 4); // format_version
    w_u32(&mut field_sec, flags);
    w_i16(&mut field_sec, 2); // 2 fields
    write_fd_objectid(&mut field_sec, "OBJECTID");
    write_fd_string(&mut field_sec, "Name", 256);

    let section_size = field_sec.len() as i32;
    let mut fs_with_size = Vec::new();
    w_i32(&mut fs_with_size, section_size);
    fs_with_size.extend_from_slice(&field_sec);

    // Encode rows
    let n_nullable = 1usize; // Name is nullable
    let bitmap_size = (n_nullable + 7) / 8;

    let mut rows_data = Vec::new();
    let mut offsets: Vec<u64> = Vec::new();

    for (_id, name) in &items {
        offsets.push((40 + fs_with_size.len() + rows_data.len()) as u64);
        let mut rb = Vec::new();
        rb.extend_from_slice(&vec![0u8; bitmap_size]); // null bitmap (all present)
        // Name: varuint byte_len + UTF-8
        let nb = name.as_bytes();
        enc_varuint(&mut rb, nb.len() as u64);
        rb.extend_from_slice(nb);

        w_i32(&mut rows_data, rb.len() as i32);
        rows_data.extend_from_slice(&rb);
    }

    // Assemble table
    let mut table = Vec::new();
    w_i32(&mut table, 3); // version
    w_i32(&mut table, items.len() as i32); // valid_record_count
    w_i32(&mut table, 1024); // max_row_size
    w_i32(&mut table, 5); // const_5
    table.extend_from_slice(&0i64.to_le_bytes()); // unused
    let file_size_off = table.len();
    table.extend_from_slice(&0i64.to_le_bytes()); // file_size (patch later)
    table.extend_from_slice(&40i64.to_le_bytes()); // field_desc_offset
    table.extend_from_slice(&fs_with_size);
    table.extend_from_slice(&rows_data);

    let fsz = table.len() as i64;
    patch_i64(&mut table, file_size_off, fsz);

    // Write files
    let tbl_path = gdb_path.join("a00000001.gdbtable");
    std::fs::write(&tbl_path, &table).map_err(|e| format!("写 catalog table 失败: {}", e))?;

    let tx = build_gdbtablx(&offsets);
    let tx_path = gdb_path.join("a00000001.gdbtablx");
    std::fs::write(&tx_path, &tx).map_err(|e| format!("写 catalog index 失败: {}", e))?;

    Ok(())
}

// ─── CRS WKT 构建 ───

/// 根据 CRS 参数动态构建 PROJCS WKT 字符串
/// 返回 (projcs_wkt, geogcs_wkt)
fn build_crs_wkt(crs_name: &str, band: &str, zone: &str) -> (String, String) {
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
        if crs_name.contains("2000") || crs_name.contains("CGCS") {
            ("GCS_China_Geodetic_Coordinate_System_2000", "D_China_2000",
             "CGCS2000", 6378137.0, 298.257222101)
        } else if crs_name.contains("西安") || crs_name.contains("Xian") || crs_name.contains("1980") {
            ("GCS_Xian_1980", "D_Xian_1980",
             "Xian_1980", 6378140.0, 298.257)
        } else if crs_name.contains("北京") || crs_name.contains("Beijing") || crs_name.contains("1954") {
            ("GCS_Beijing_1954", "D_Beijing_1954",
             "Krasovsky_1940", 6378245.0, 298.3)
        } else {
            // WGS84 or default (assumed geographic if no projection)
            ("GCS_WGS_1984", "D_WGS_1984",
             "WGS_1984", 6378137.0, 298.257223563)
        };

    let band_label = if (band_val - 3.0).abs() < 0.1 { "3_Degree" } else { "6_Degree" };
    let zone_int = zval as i32;

    // Use CGCS2000 naming convention for all CRS types (保持一致)
    let projcs = format!(
        "PROJCS[\"CGCS2000_{}_GK_Zone_{}\",GEOGCS[\"{}\",DATUM[\"{}\",SPHEROID[\"{}\",{},{}]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]],PROJECTION[\"Gauss_Kruger\"],PARAMETER[\"False_Easting\",{}],PARAMETER[\"False_Northing\",0.0],PARAMETER[\"Central_Meridian\",{}],PARAMETER[\"Scale_Factor\",1.0],PARAMETER[\"Latitude_Of_Origin\",0.0],UNIT[\"Meter\",1.0]]",
        band_label, zone_int,
        geogcs_name, datum_name, spheroid_name, semi_major, inv_flattening,
        false_easting, central_meridian
    );

    let geogcs = format!(
        "GEOGCS[\"{}\",DATUM[\"{}\",SPHEROID[\"{}\",{},{}]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]]",
        geogcs_name, datum_name, spheroid_name, semi_major, inv_flattening
    );

    (projcs, geogcs)
}

// ─── spTimestamps 系统表 (a00000003) ───

/// 写入 spTimestamps 系统表 (a00000003)
fn write_sp_timestamps(gdb_path: &Path) -> Result<(), String> {
    let items = vec![
        ("a00000001".to_string(), "GDB_SystemCatalog".to_string()),
        ("a00000002".to_string(), "GDB_FeatureClass".to_string()),
        ("a00000003".to_string(), "spTimestamps".to_string()),
        ("a00000004".to_string(), "GDB_Items".to_string()),
        ("a00000005".to_string(), "GDB_ItemTypes".to_string()),
    ];

    let flags = 0x100u32;
    let n_nullable = 3usize;
    let bitmap_size = (n_nullable + 7) / 8;

    // Field descriptors: OBJECTID + TableName + CreationTime + LastModifiedTime
    let mut field_sec = Vec::new();
    w_u32(&mut field_sec, 4);
    w_u32(&mut field_sec, flags);
    w_i16(&mut field_sec, 4);
    write_fd_objectid(&mut field_sec, "OBJECTID");
    write_fd_string(&mut field_sec, "TableName", 160);
    // CreationTime (Float64 for OLE Automation date)
    let n16 = enc_utf16le(&mut Vec::new(), "CreationTime");
    w_u8(&mut field_sec, (n16 / 2) as u8);
    enc_utf16le(&mut field_sec, "CreationTime");
    w_u8(&mut field_sec, 0);
    w_u8(&mut field_sec, 3); // Float64
    w_u8(&mut field_sec, 8);
    w_u8(&mut field_sec, 0x01);
    // LastModifiedTime (Float64)
    let n16 = enc_utf16le(&mut Vec::new(), "LastModifiedTime");
    w_u8(&mut field_sec, (n16 / 2) as u8);
    enc_utf16le(&mut field_sec, "LastModifiedTime");
    w_u8(&mut field_sec, 0);
    w_u8(&mut field_sec, 3);
    w_u8(&mut field_sec, 8);
    w_u8(&mut field_sec, 0x01);

    let section_size = field_sec.len() as i32;
    let mut fs_with_size = Vec::new();
    w_i32(&mut fs_with_size, section_size);
    fs_with_size.extend_from_slice(&field_sec);

    // OLE Automation date: 2026-06-10 ≈ 46180.0
    let now_ole: f64 = 46180.0;

    let mut rows_data = Vec::new();
    let mut offsets: Vec<u64> = Vec::new();

    for (_id, name) in &items {
        offsets.push((40 + fs_with_size.len() + rows_data.len()) as u64);
        let mut rb = Vec::new();
        rb.extend_from_slice(&vec![0u8; bitmap_size]);
        let nb = name.as_bytes();
        enc_varuint(&mut rb, nb.len() as u64);
        rb.extend_from_slice(nb);
        w_f64(&mut rb, now_ole);
        w_f64(&mut rb, now_ole);

        w_i32(&mut rows_data, rb.len() as i32);
        rows_data.extend_from_slice(&rb);
    }

    let mut table = Vec::new();
    w_i32(&mut table, 3);
    w_i32(&mut table, items.len() as i32);
    w_i32(&mut table, 1024);
    w_i32(&mut table, 5);
    table.extend_from_slice(&0i64.to_le_bytes());
    let file_size_off = table.len();
    table.extend_from_slice(&0i64.to_le_bytes());
    table.extend_from_slice(&40i64.to_le_bytes());
    table.extend_from_slice(&fs_with_size);
    table.extend_from_slice(&rows_data);
    let fsz = table.len() as i64;
    patch_i64(&mut table, file_size_off, fsz);

    let tbl_path = gdb_path.join("a00000003.gdbtable");
    std::fs::write(&tbl_path, &table)
        .map_err(|e| format!("写 spTimestamps table 失败: {}", e))?;
    let tx = build_gdbtablx(&offsets);
    let tx_path = gdb_path.join("a00000003.gdbtablx");
    std::fs::write(&tx_path, &tx)
        .map_err(|e| format!("写 spTimestamps index 失败: {}", e))?;

    Ok(())
}

// ─── GDB_Items 系统表 (a00000004) ───

/// 从字符串生成确定性 UUID
fn simple_uuid_from(s: &str) -> [u8; 16] {
    let mut buf = [0u8; 16];
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        buf[i % 16] ^= b;
    }
    // Set version 4 UUID bits
    buf[6] = (buf[6] & 0x0F) | 0x40;
    buf[8] = (buf[8] & 0x3F) | 0x80;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for i in 0..8 {
        buf[8 + i] ^= ((ts >> (i * 8)) & 0xFF) as u8;
    }
    buf
}

/// 写入 GDB_Items 系统表 (a00000004) -- 最小化实现
fn write_gdb_items(gdb_path: &Path, fc_name: &str, wkt: &str) -> Result<(), String> {
    // Esri FileGDB Type UUIDs
    let workspace_type_uuid: [u8; 16] = [
        0x59, 0x96, 0x3A, 0x71, 0xEF, 0x0D, 0x9A, 0x41,
        0xA7, 0xBB, 0x3B, 0x3E, 0xF8, 0x30, 0x99, 0x55,
    ];
    let fc_type_uuid: [u8; 16] = [
        0xF0, 0xA0, 0x0E, 0xE2, 0x1C, 0x9B, 0x1D, 0x4C,
        0xA4, 0xDB, 0xB2, 0x2A, 0x73, 0xBD, 0xC1, 0x92,
    ];

    let root_uuid = simple_uuid_from("ROOT");
    let fc_uuid = simple_uuid_from(fc_name);
    let zero_uuid = [0u8; 16];

    let flags = 0x104u32;
    let n_nullable = 16usize;
    let bitmap_size = (n_nullable + 7) / 8;

    // Field descriptors: 17 fields for GDB_Items
    let mut field_sec = Vec::new();
    w_u32(&mut field_sec, 4);
    w_u32(&mut field_sec, flags);
    w_i16(&mut field_sec, 17);

    write_fd_objectid(&mut field_sec, "OBJECTID");

    // UUID (Guid type)
    {
        let n16 = enc_utf16le(&mut Vec::new(), "UUID");
        w_u8(&mut field_sec, (n16 / 2) as u8);
        enc_utf16le(&mut field_sec, "UUID");
        w_u8(&mut field_sec, 0);
        w_u8(&mut field_sec, 8); // Guid
        w_u8(&mut field_sec, 16);
        w_u8(&mut field_sec, 0x01);
    }
    // ParentId (Guid)
    {
        let n16 = enc_utf16le(&mut Vec::new(), "ParentId");
        w_u8(&mut field_sec, (n16 / 2) as u8);
        enc_utf16le(&mut field_sec, "ParentId");
        w_u8(&mut field_sec, 0);
        w_u8(&mut field_sec, 8);
        w_u8(&mut field_sec, 16);
        w_u8(&mut field_sec, 0x01);
    }
    // Type (UUID)
    {
        let n16 = enc_utf16le(&mut Vec::new(), "Type");
        w_u8(&mut field_sec, (n16 / 2) as u8);
        enc_utf16le(&mut field_sec, "Type");
        w_u8(&mut field_sec, 0);
        w_u8(&mut field_sec, 8);
        w_u8(&mut field_sec, 16);
        w_u8(&mut field_sec, 0x01);
    }

    // Name, PhysicalName, Path (String)
    for fname in &["Name", "PhysicalName", "Path"] {
        write_fd_string(&mut field_sec, fname, 512);
    }

    // DatasetSubtype1, DatasetSubtype2 (Int32)
    for fname in &["DatasetSubtype1", "DatasetSubtype2"] {
        let n16 = enc_utf16le(&mut Vec::new(), fname);
        w_u8(&mut field_sec, (n16 / 2) as u8);
        enc_utf16le(&mut field_sec, fname);
        w_u8(&mut field_sec, 0);
        w_u8(&mut field_sec, 1); // Int32
        w_u8(&mut field_sec, 4);
        w_u8(&mut field_sec, 0x01);
    }

    // String fields (DatasetInfo1, DatasetInfo2, URL, Definition, Documentation, ItemInfo)
    for fname in &["DatasetInfo1", "DatasetInfo2", "URL", "Definition", "Documentation", "ItemInfo"] {
        write_fd_string(&mut field_sec, fname, 4096);
    }

    // Shape (Geometry)
    write_fd_geometry(&mut field_sec, "Shape", wkt, -400.0, -400.0, 10000.0);

    let section_size = field_sec.len() as i32;
    let mut fs_with_size = Vec::new();
    w_i32(&mut fs_with_size, section_size);
    fs_with_size.extend_from_slice(&field_sec);

    let items: Vec<(Vec<u8>, Vec<u8>, Vec<u8>, String, String, String)> = vec![
        (
            root_uuid.to_vec(),
            zero_uuid.to_vec(),
            workspace_type_uuid.to_vec(),
            "ROOT".to_string(),
            "ROOT".to_string(),
            "\\".to_string(),
        ),
        (
            fc_uuid.to_vec(),
            root_uuid.to_vec(),
            fc_type_uuid.to_vec(),
            fc_name.to_string(),
            fc_name.to_string(),
            format!("\\{}", fc_name),
        ),
    ];

    // We need to write 2 rows but the spTimestamps has 5 entries now
    // The GDB_Items table only needs workspace + feature class entries

    let mut rows_data = Vec::new();
    let mut offsets: Vec<u64> = Vec::new();

    for (uuid, parent_id, typ, name, phys_name, path) in &items {
        offsets.push((40 + fs_with_size.len() + rows_data.len()) as u64);
        let mut rb = Vec::new();
        rb.extend_from_slice(&vec![0u8; bitmap_size]);

        rb.extend_from_slice(uuid);
        rb.extend_from_slice(parent_id);
        rb.extend_from_slice(typ);
        for s in &[name, phys_name, path] {
            let nb = s.as_bytes();
            enc_varuint(&mut rb, nb.len() as u64);
            rb.extend_from_slice(nb);
        }
        // DatasetSubtype1, DatasetSubtype2 = 0
        for _ in 0..2 {
            w_i32(&mut rb, 0);
        }
        // Empty strings for DatasetInfo1, DatasetInfo2, URL, Definition, Documentation, ItemInfo
        for _ in 0..6 {
            enc_varuint(&mut rb, 0);
        }
        // Empty shape
        enc_varuint(&mut rb, 0);

        w_i32(&mut rows_data, rb.len() as i32);
        rows_data.extend_from_slice(&rb);
    }

    let mut table = Vec::new();
    w_i32(&mut table, 3);
    w_i32(&mut table, items.len() as i32);
    w_i32(&mut table, 65536);
    w_i32(&mut table, 5);
    table.extend_from_slice(&0i64.to_le_bytes());
    let file_size_off = table.len();
    table.extend_from_slice(&0i64.to_le_bytes());
    table.extend_from_slice(&40i64.to_le_bytes());
    table.extend_from_slice(&fs_with_size);
    table.extend_from_slice(&rows_data);
    let fsz = table.len() as i64;
    patch_i64(&mut table, file_size_off, fsz);

    let tbl_path = gdb_path.join("a00000004.gdbtable");
    std::fs::write(&tbl_path, &table)
        .map_err(|e| format!("写 GDB_Items table 失败: {}", e))?;
    let tx = build_gdbtablx(&offsets);
    let tx_path = gdb_path.join("a00000004.gdbtablx");
    std::fs::write(&tx_path, &tx)
        .map_err(|e| format!("写 GDB_Items index 失败: {}", e))?;

    Ok(())
}

// ─── timestamps 文件 (400 bytes) ───

fn write_timestamps_file(gdb_path: &Path) -> Result<(), String> {
    let mut buf = vec![0u8; 400];
    // Header
    buf[0..4].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // sentinel
    buf[4..8].copy_from_slice(&20u32.to_le_bytes()); // version = 20
    buf[8..12].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // sentinel
    buf[12..16].copy_from_slice(&0u32.to_le_bytes()); // flags
    buf[16..20].copy_from_slice(&50u32.to_le_bytes()); // hash
    buf[20..24].copy_from_slice(&2u32.to_le_bytes()); // record_count
    buf[24..28].copy_from_slice(&14u32.to_le_bytes()); // count
    buf[28..32].copy_from_slice(&2u32.to_le_bytes()); // count
    // Rest is 0xff padding
    for i in 32..400 {
        buf[i] = 0xFF;
    }
    std::fs::write(gdb_path.join("timestamps"), &buf)
        .map_err(|e| format!("写 timestamps 失败: {}", e))
}

// ─── .gdbindexes 文件 ───

fn write_gdb_indexes_file(gdb_path: &Path, table_num: &str, indexes: &[(String, Vec<String>)]) -> Result<(), String> {
    let mut buf = Vec::new();
    w_u32(&mut buf, indexes.len() as u32);
    for (idx_name, field_names) in indexes {
        w_u32(&mut buf, idx_name.len() as u32 * 2);
        enc_utf16le(&mut buf, idx_name);
        w_u32(&mut buf, 0); // unknown
        w_u32(&mut buf, field_names.len() as u32);
        for fn_name in field_names {
            w_u32(&mut buf, fn_name.len() as u32 * 2);
            enc_utf16le(&mut buf, fn_name);
        }
    }
    let path = gdb_path.join(format!("{}.gdbindexes", table_num));
    std::fs::write(&path, &buf)
        .map_err(|e| format!("写 {}gdbindexes 失败: {}", table_num, e))
}

// ─── .horizon 文件 (32 bytes) ───

fn write_horizon_file(gdb_path: &Path, table_num: &str) -> Result<(), String> {
    let mut buf = Vec::with_capacity(32);
    // 4 entries: xmin, ymin, xmax, ymax — each u32(2) + f64
    let bounds: [(u32, f64); 4] = [
        (2, -400.0),
        (2, -90.0),
        (2, 400.0),
        (2, 90.0),
    ];
    for (flag, val) in &bounds {
        w_u32(&mut buf, *flag);
        w_f64(&mut buf, *val);
    }
    let path = gdb_path.join(format!("{}.horizon", table_num));
    std::fs::write(&path, &buf)
        .map_err(|e| format!("写 {}horizon 失败: {}", table_num, e))
}

// ─── .spx 文件 (空空间索引) ───

fn write_spx_file(gdb_path: &Path, table_num: &str) -> Result<(), String> {
    // Empty spatial index: 4118 bytes of zeros (standard size)
    let buf = vec![0u8; 4118];
    let path = gdb_path.join(format!("{}.spx", table_num));
    std::fs::write(&path, &buf)
        .map_err(|e| format!("写 {}spx 失败: {}", table_num, e))
}

// ─── .atx 文件 (属性索引) ───

fn write_atx_file(gdb_path: &Path, table_num: &str, index_name: &str, num_records: u32) -> Result<(), String> {
    // B-tree attribute index: 4118 bytes
    let mut buf = vec![0u8; 4118];
    // Header: version=0, num_records, ...
    buf[0..4].copy_from_slice(&0u32.to_le_bytes()); // version
    buf[4..8].copy_from_slice(&num_records.to_le_bytes()); // record count
    // Fill with sorted record indices (1-based)
    for i in 0..num_records.min(1023) {
        let off = 8 + (i as usize) * 4;
        if off + 4 <= 4118 {
            buf[off..off + 4].copy_from_slice(&(i + 1).to_le_bytes());
        }
    }
    // Write attribute values (UTF-16LE padded to 256 bytes each)
    // For simplicity, write empty attribute blocks
    let path = gdb_path.join(format!("{}.{}.atx", table_num, index_name));
    std::fs::write(&path, &buf)
        .map_err(|e| format!("写 {}{}.atx 失败: {}", table_num, index_name, e))
}

// ─── GDB_ItemTypes 系统表 (a00000005) ───

fn write_gdb_item_types(gdb_path: &Path) -> Result<(), String> {
    // Standard Esri type UUIDs
    let type_uuids: Vec<([u8; 16], String)> = vec![
        // Feature Class
        ([
            0xF0, 0xA0, 0x0E, 0xE2, 0x1C, 0x9B, 0x1D, 0x4C,
            0xA4, 0xDB, 0xB2, 0x2A, 0x73, 0xBD, 0xC1, 0x92,
        ], "Feature Class".to_string()),
        // Workspace
        ([
            0x59, 0x96, 0x3A, 0x71, 0xEF, 0x0D, 0x9A, 0x41,
            0xA7, 0xBB, 0x3B, 0x3E, 0xF8, 0x30, 0x99, 0x55,
        ], "Workspace".to_string()),
        // Feature Dataset
        ([
            0x69, 0x42, 0x47, 0x9E, 0x4A, 0x1E, 0x9D, 0xC3,
            0xA0, 0xF5, 0xC0, 0x5B, 0x1F, 0x49, 0x1C, 0xFB,
        ], "Feature Dataset".to_string()),
        // Topology
        ([
            0x53, 0x16, 0x98, 0xFA, 0x06, 0x4C, 0x4D, 0xD0,
            0xA0, 0x39, 0x60, 0xD3, 0x5F, 0x5B, 0x02, 0x3F,
        ], "Topology".to_string()),
        // Network Dataset
        ([
            0x4B, 0x06, 0x0B, 0x0E, 0x6D, 0x5F, 0x41, 0xE4,
            0xA0, 0x57, 0xE3, 0x2D, 0x7C, 0x2B, 0x4E, 0x0D,
        ], "Network Dataset".to_string()),
    ];

    let flags = 0x100u32;
    let n_nullable = 3usize;
    let bitmap_size = (n_nullable + 7) / 8;

    // Field descriptors: OBJECTID + UUID + ParentTypeID + Name
    let mut field_sec = Vec::new();
    w_u32(&mut field_sec, 4);
    w_u32(&mut field_sec, flags);
    w_i16(&mut field_sec, 4);
    write_fd_objectid(&mut field_sec, "OBJECTID");
    // UUID (Guid)
    {
        let n16 = enc_utf16le(&mut Vec::new(), "UUID");
        w_u8(&mut field_sec, (n16 / 2) as u8);
        enc_utf16le(&mut field_sec, "UUID");
        w_u8(&mut field_sec, 0);
        w_u8(&mut field_sec, 8); // Guid
        w_u8(&mut field_sec, 16);
        w_u8(&mut field_sec, 0x01);
    }
    // ParentTypeID (Guid)
    {
        let n16 = enc_utf16le(&mut Vec::new(), "ParentTypeID");
        w_u8(&mut field_sec, (n16 / 2) as u8);
        enc_utf16le(&mut field_sec, "ParentTypeID");
        w_u8(&mut field_sec, 0);
        w_u8(&mut field_sec, 8);
        w_u8(&mut field_sec, 16);
        w_u8(&mut field_sec, 0x01);
    }
    // Name
    write_fd_string(&mut field_sec, "Name", 64);

    let section_size = field_sec.len() as i32;
    let mut fs_with_size = Vec::new();
    w_i32(&mut fs_with_size, section_size);
    fs_with_size.extend_from_slice(&field_sec);

    // Encode rows
    let mut rows_data = Vec::new();
    let mut offsets: Vec<u64> = Vec::new();

    for (uuid, name) in &type_uuids {
        offsets.push((40 + fs_with_size.len() + rows_data.len()) as u64);
        let mut rb = Vec::new();
        rb.extend_from_slice(&vec![0u8; bitmap_size]);
        rb.extend_from_slice(uuid);
        rb.extend_from_slice(&[0u8; 16]); // ParentTypeID = zero
        let nb = name.as_bytes();
        enc_varuint(&mut rb, nb.len() as u64);
        rb.extend_from_slice(nb);
        w_i32(&mut rows_data, rb.len() as i32);
        rows_data.extend_from_slice(&rb);
    }

    let mut table = Vec::new();
    w_i32(&mut table, 3);
    w_i32(&mut table, type_uuids.len() as i32);
    w_i32(&mut table, 1024);
    w_i32(&mut table, 5);
    table.extend_from_slice(&0i64.to_le_bytes());
    let file_size_off = table.len();
    table.extend_from_slice(&0i64.to_le_bytes());
    table.extend_from_slice(&40i64.to_le_bytes());
    table.extend_from_slice(&fs_with_size);
    table.extend_from_slice(&rows_data);
    let fsz = table.len() as i64;
    patch_i64(&mut table, file_size_off, fsz);

    let tbl_path = gdb_path.join("a00000005.gdbtable");
    std::fs::write(&tbl_path, &table)
        .map_err(|e| format!("写 GDB_ItemTypes table 失败: {}", e))?;
    let tx = build_gdbtablx(&offsets);
    let tx_path = gdb_path.join("a00000005.gdbtablx");
    std::fs::write(&tx_path, &tx)
        .map_err(|e| format!("写 GDB_ItemTypes index 失败: {}", e))?;

    Ok(())
}

// ─── GDB 写入（纯 Rust OpenFileGDB，仅 Polygon）───

/// 写 .gdb 目录（纯 Rust 实现，不依赖 GDAL）
pub fn write_gdb_output(
    output_dir: &Path,
    base_name: &str,
    fields: &[(String, String, u8, u32)],
    attributes: &[HashMap<String, String>],
    geometries: &[Vec<(f64, f64)>],
    crs_info: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let gdb_name = format!("{}.gdb", base_name);
    let gdb_path = output_dir.join(&gdb_name);

    if gdb_path.exists() {
        std::fs::remove_dir_all(&gdb_path)
            .map_err(|e| format!("清理旧 GDB 失败: {}", e))?;
    }
    std::fs::create_dir_all(&gdb_path)
        .map_err(|e| format!("创建 GDB 目录失败: {}", e))?;

    // Geometry parameters
    let xorigin = -400.0;
    let yorigin = -400.0;
    let xyscale = 10000.0; // 0.1mm precision

    // 根据 CRS 参数动态生成 PROJCS WKT（修复：原来错误使用 GEOGCS）
    let crs_name = crs_info.get("c").map(|s| s.as_str()).unwrap_or("CGCS2000");
    let band = crs_info.get("b").map(|s| s.as_str()).unwrap_or("3");
    let zone = crs_info.get("z").map(|s| s.as_str()).unwrap_or("38");
    let (wkt, _geogcs_wkt) = build_crs_wkt(crs_name, band, zone);

    let layer_flags = 0x104u32; // polygon(4) + UTF-8(bit8)
    let _strings_utf8 = true;

    // Field descriptors
    let mut field_sec = Vec::new();
    w_u32(&mut field_sec, 4); // format_version
    w_u32(&mut field_sec, layer_flags);
    w_i16(&mut field_sec, (2 + fields.len()) as i16);
    write_fd_objectid(&mut field_sec, "OBJECTID");
    write_fd_geometry(&mut field_sec, "Shape", &wkt, xorigin, yorigin, xyscale);
    for (name, _desc, type_code, width) in fields {
        match type_code {
            3 => write_fd_float64(&mut field_sec, name),
            _ => write_fd_string(&mut field_sec, name, *width),
        }
    }

    let section_size = field_sec.len() as i32;
    let mut fs_with_size = Vec::new();
    w_i32(&mut fs_with_size, section_size);
    fs_with_size.extend_from_slice(&field_sec);

    // Null bitmap: 1(Shape) + user_field_count nullable fields
    let n_nullable = 1 + fields.len();
    let bitmap_size = (n_nullable + 7) / 8;

    // Encode rows
    let mut rows_data = Vec::new();
    let mut row_offsets = Vec::new();

    for (fi, coords) in geometries.iter().enumerate() {
        if coords.len() < 3 {
            continue;
        }

        let offset = 40 + fs_with_size.len() + rows_data.len();
        row_offsets.push(offset as u64);

        let mut rb = Vec::new();
        // null bitmap (all present)
        rb.extend_from_slice(&vec![0u8; bitmap_size]);

        // Shape: varuint(len) + shape_buffer
        let shape = encode_polygon_shape(coords, xyscale, xorigin, yorigin);
        if shape.is_empty() {
            continue;
        }
        enc_varuint(&mut rb, shape.len() as u64);
        rb.extend_from_slice(&shape);

        // User fields
        for (name, _desc, type_code, _width) in fields {
            let str_val = attributes
                .get(fi)
                .and_then(|a| a.get(name.as_str()))
                .map(|s| s.as_str())
                .unwrap_or("");

            match type_code {
                3 => {
                    // Float64
                    let fval = str_val.parse::<f64>().unwrap_or(0.0);
                    w_f64(&mut rb, fval);
                }
                _ => {
                    // String: varuint byte_len + UTF-8 bytes
                    let nb = str_val.as_bytes();
                    enc_varuint(&mut rb, nb.len() as u64);
                    rb.extend_from_slice(nb);
                }
            }
        }

        w_i32(&mut rows_data, rb.len() as i32);
        rows_data.extend_from_slice(&rb);
    }

    // Assemble feature class table
    let mut table = Vec::new();
    w_i32(&mut table, 3); // version
    w_i32(&mut table, row_offsets.len() as i32);
    w_i32(&mut table, 65536); // max_row_size
    w_i32(&mut table, 5);
    table.extend_from_slice(&0i64.to_le_bytes());
    let file_size_off = table.len();
    table.extend_from_slice(&0i64.to_le_bytes()); // file_size (patch later)
    table.extend_from_slice(&40i64.to_le_bytes());
    table.extend_from_slice(&fs_with_size);
    table.extend_from_slice(&rows_data);

    let fsz = table.len() as i64;
    patch_i64(&mut table, file_size_off, fsz);

    let tbl_path = gdb_path.join("a00000002.gdbtable");
    std::fs::write(&tbl_path, &table)
        .map_err(|e| format!("写 feature class table 失败: {}", e))?;

    let tx = build_gdbtablx(&row_offsets);
    let tx_path = gdb_path.join("a00000002.gdbtablx");
    std::fs::write(&tx_path, &tx)
        .map_err(|e| format!("写 feature class index 失败: {}", e))?;

    // System catalog
    write_system_catalog(&gdb_path, base_name)?;

    // spTimestamps
    write_sp_timestamps(&gdb_path)?;

    // GDB_Items
    write_gdb_items(&gdb_path, base_name, &wkt)?;

    // GDB_ItemTypes 系统表
    write_gdb_item_types(&gdb_path)?;

    // timestamps 文件
    write_timestamps_file(&gdb_path)?;

    // .gdbindexes 文件
    write_gdb_indexes_file(&gdb_path, "a00000001", &[
        ("FDO_ID".to_string(), vec!["OBJECTID".to_string()]),
        ("TablesByName".to_string(), vec!["Name".to_string()]),
    ])?;
    write_gdb_indexes_file(&gdb_path, "a00000003", &[
        ("FDO_ID".to_string(), vec!["OBJECTID".to_string()]),
    ])?;
    write_gdb_indexes_file(&gdb_path, "a00000004", &[
        ("FDO_ObjectID".to_string(), vec!["OBJECTID".to_string()]),
        ("ObjectID".to_string(), vec!["OBJECTID".to_string()]),
        ("FDO_Shape".to_string(), vec!["Shape".to_string()]),
        ("FDO_UUID".to_string(), vec!["UUID".to_string()]),
        ("CatItemsByType".to_string(), vec!["Type".to_string()]),
        ("CatItemsByPhysicalName".to_string(), vec!["PhysicalName".to_string()]),
    ])?;
    write_gdb_indexes_file(&gdb_path, "a00000005", &[
        ("FDO_ObjectID".to_string(), vec!["OBJECTID".to_string()]),
        ("CatItemTypesByUUID".to_string(), vec!["UUID".to_string()]),
        ("CatItemTypesByParentTypeID".to_string(), vec!["ParentTypeID".to_string()]),
        ("CatItemTypesByName".to_string(), vec!["Name".to_string()]),
    ])?;
    write_gdb_indexes_file(&gdb_path, "a00000002", &[
        ("FDO_ObjectID".to_string(), vec!["OBJECTID".to_string()]),
        ("FDO_Shape".to_string(), vec!["Shape".to_string()]),
    ])?;

    // .horizon 文件（空间表）
    write_horizon_file(&gdb_path, "a00000004")?;
    write_horizon_file(&gdb_path, "a00000002")?;

    // .spx 文件（空间索引）
    write_spx_file(&gdb_path, "a00000004")?;
    write_spx_file(&gdb_path, "a00000002")?;

    // .atx 文件（属性索引）
    write_atx_file(&gdb_path, "a00000001", "TablesByName", 2)?;
    write_atx_file(&gdb_path, "a00000004", "CatItemsByPhysicalName", 2)?;
    write_atx_file(&gdb_path, "a00000004", "CatItemsByType", 2)?;
    write_atx_file(&gdb_path, "a00000004", "FDO_UUID", 2)?;
    write_atx_file(&gdb_path, "a00000005", "CatItemTypesByName", 5)?;
    write_atx_file(&gdb_path, "a00000005", "CatItemTypesByUUID", 5)?;
    write_atx_file(&gdb_path, "a00000005", "CatItemTypesByParentTypeID", 5)?;

    // Marker file
    std::fs::write(gdb_path.join("gdb"), b"")
        .map_err(|e| format!("写 gdb marker 失败: {}", e))?;

    Ok(vec![gdb_path.to_string_lossy().to_string()])
}
