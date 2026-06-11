//! GeoPackage (.gpkg) 读写模块 — 纯 Rust 实现
//! 基于 OGC GeoPackage 标准，使用 SQLite 存储矢量面数据

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// 从 GeoPackage 读取的要素信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpkgFeature {
    pub points: Vec<(f64, f64)>,
    pub attributes: HashMap<String, String>,
}

/// GeoPackage 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpkgFileInfo {
    pub path: String,
    pub name: String,
    pub layers: Vec<GpkgLayerInfo>,
    pub all_features: Vec<Vec<GpkgFeature>>,
    pub all_field_names: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpkgLayerInfo {
    pub name: String,
    pub field_names: Vec<String>,
    pub num_features: usize,
}

/// CGCS2000 3-degree Gauss-Kruger zone 38 SRID
const CGCS2000_SRID: i32 = 4490;

// ─── WKB 编码/解码 ───

/// 将坐标点编码为 WKB Polygon（EPSG:4326 经纬度或投影坐标）
fn encode_wkb_polygon(coords: &[(f64, f64)]) -> Vec<u8> {
    if coords.len() < 3 {
        return Vec::new();
    }

    // 确保闭合
    let ring: Vec<(f64, f64)> = if coords.len() >= 2
        && (coords[0].0 - coords.last().unwrap().0).abs() < 1e-12
        && (coords[0].1 - coords.last().unwrap().1).abs() < 1e-12
    {
        coords.to_vec()
    } else {
        let mut c = coords.to_vec();
        c.push(coords[0]);
        c
    };

    let n = ring.len();
    let mut buf = Vec::with_capacity(9 + n * 16);

    // WKB header: LittleEndian(1) + Polygon(3) + nRings(1) + nPoints + coords
    buf.push(0x01); // byte order = Little Endian
    buf.extend_from_slice(&3u32.to_le_bytes()); // geometry type = Polygon
    buf.extend_from_slice(&1u32.to_le_bytes()); // 1 ring (exterior)
    buf.extend_from_slice(&(n as u32).to_le_bytes()); // number of points
    for &(x, y) in &ring {
        buf.extend_from_slice(&x.to_le_bytes()); // X (easting)
        buf.extend_from_slice(&y.to_le_bytes()); // Y (northing)
    }
    buf
}

/// GeoPackage 几何编码：GPKG header + WKB
fn encode_gpkg_geom(coords: &[(f64, f64)]) -> Vec<u8> {
    if coords.len() < 3 {
        return Vec::new();
    }
    let wkb = encode_wkb_polygon(coords);
    if wkb.is_empty() {
        return Vec::new();
    }

    // GPKG BLOB header: magic(2) + version(1) + flags(1) + srid(4) + [empty_envelope]
    // flags: bit0=0(no envelope), bit1=1(exterior), bit2-3=0(no crs), bit4-7=0
    let flags: u8 = 0x02; // exterior envelope empty
    let mut buf = Vec::with_capacity(8 + wkb.len());
    buf.extend_from_slice(b"GP"); // magic
    buf.push(0x00); // version
    buf.push(flags);
    buf.extend_from_slice(&CGCS2000_SRID.to_le_bytes());
    // No envelope for simplicity
    buf.extend_from_slice(&wkb);
    buf
}

/// 解码 WKB 几何体为坐标点列表（仅支持 Polygon）
fn decode_wkb_polygon(data: &[u8]) -> Option<Vec<(f64, f64)>> {
    if data.len() < 9 {
        return None;
    }
    let _byte_order = data[0];
    let geom_type = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    if geom_type != 3 {
        // Only Polygon supported
        return None;
    }
    let n_rings = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);
    if n_rings < 1 {
        return None;
    }
    let mut pos = 9;
    if pos + 4 > data.len() {
        return None;
    }
    let n_points = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
    pos += 4;

    let mut coords = Vec::with_capacity(n_points as usize);
    for _ in 0..n_points {
        if pos + 16 > data.len() {
            break;
        }
        let x = f64::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3],
                                     data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
        let y = f64::from_le_bytes([data[pos+8], data[pos+9], data[pos+10], data[pos+11],
                                     data[pos+12], data[pos+13], data[pos+14], data[pos+15]]);
        coords.push((x, y));
        pos += 16;
    }
    Some(coords)
}

/// 从 GPKG BLOB 中提取坐标点（支持有/无 GPKG header）
fn decode_gpkg_geom(data: &[u8]) -> Option<Vec<(f64, f64)>> {
    if data.len() < 8 {
        return None;
    }
    // Check GPKG magic header
    if data[0] == b'G' && data[1] == b'P' {
        // Has GPKG header: skip 8 bytes (magic2 + ver1 + flags1 + srid4)
        decode_wkb_polygon(&data[8..])
    } else {
        // No GPKG header, assume raw WKB
        decode_wkb_polygon(data)
    }
}

// ─── 读取 ───

/// 打开并读取 GeoPackage 文件
pub fn read_gpkg(path: &Path) -> Result<GpkgFileInfo, String> {
    if !path.exists() {
        return Err(format!("GeoPackage 路径不存在: {}", path.display()));
    }

    let conn = Connection::open(path)
        .map_err(|e| format!("打开 GeoPackage 失败: {}", e))?;

    // 查询 feature 表
    let mut stmt = conn.prepare(
        "SELECT table_name, geometry_column_name, srs_id FROM gpkg_geometry_columns"
    ).map_err(|e| format!("查询 gpkg_geometry_columns 失败: {}", e))?;

    let mut layers = Vec::new();
    let mut all_features = Vec::new();
    let mut all_field_names = Vec::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,     // table_name
            row.get::<_, String>(1)?,     // geometry_column_name
            row.get::<_, i32>(2)?,        // srs_id
        ))
    }).map_err(|e| format!("读取 geometry_columns 失败: {}", e))?;

    for row_result in rows {
        let (table_name, geom_col, _srs_id) = row_result
            .map_err(|e| format!("读取行失败: {}", e))?;

        // 获取字段名（排除 geometry 列）
        let mut col_stmt = conn.prepare(&format!("PRAGMA table_info(\"{}\")", table_name))
            .map_err(|e| format!("获取字段信息失败: {}", e))?;
        let field_names: Vec<String> = col_stmt.query_map([], |row| {
            row.get::<_, String>(1)
        }).map_err(|e| format!("读字段名失败: {}", e))?
        .filter_map(|r| r.ok())
        .filter(|n| n != &geom_col)
        .collect();

        // 读取要素
        let query = format!("SELECT rowid, \"{}\", * FROM \"{}\"", geom_col, table_name);
        let mut feat_stmt = conn.prepare(&query)
            .map_err(|e| format!("查询 {} 失败: {}", table_name, e))?;

        let mut features = Vec::new();
        let feat_rows = feat_stmt.query_map([], |row| {
            let geom_blob: Option<Vec<u8>> = row.get(1).ok();
            let mut attrs = HashMap::new();
            // 从第 2 列开始是用户属性（rowid + geom 后）
            for (i, name) in field_names.iter().enumerate() {
                let val: String = row.get::<_, String>(i + 2).unwrap_or_default();
                attrs.insert(name.clone(), val);
            }
            Ok((geom_blob, attrs))
        }).map_err(|e| format!("读取要素失败: {}", e))?;

        for feat in feat_rows {
            let (geom_blob, attrs) = feat.map_err(|e| format!("要素行错误: {}", e))?;
            if let Some(blob) = geom_blob {
                if let Some(coords) = decode_gpkg_geom(&blob) {
                    features.push(GpkgFeature { points: coords, attributes: attrs });
                }
            }
        }

        let layer_info = GpkgLayerInfo {
            name: table_name.clone(),
            field_names: field_names.clone(),
            num_features: features.len(),
        };
        layers.push(layer_info);
        all_features.push(features);
        all_field_names.push(field_names);
    }

    if layers.is_empty() {
        return Err("GeoPackage 中未找到要素图层".to_string());
    }

    let name = path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(GpkgFileInfo {
        path: path.to_string_lossy().to_string(),
        name,
        layers,
        all_features,
        all_field_names,
    })
}

// ─── 写入 ───

/// 将 TXT 解析结果写入 GeoPackage
pub fn write_gpkg_output(
    output_dir: &Path,
    base_name: &str,
    fields: &[(String, String, u8, u32)],
    attributes: &[HashMap<String, String>],
    geometries: &[Vec<(f64, f64)>],
    _crs_info: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let gpkg_path = output_dir.join(format!("{}.gpkg", base_name));

    // 清理旧文件
    if gpkg_path.exists() {
        std::fs::remove_file(&gpkg_path)
            .map_err(|e| format!("清理旧 GPKG 失败: {}", e))?;
    }

    let conn = Connection::open(&gpkg_path)
        .map_err(|e| format!("创建 GeoPackage 失败: {}", e))?;

    // 创建元数据表
    conn.execute_batch("
        CREATE TABLE gpkg_spatial_ref_sys (
            srs_id INTEGER PRIMARY KEY,
            srs_name TEXT NOT NULL,
            srs_type TEXT NOT NULL,
            organization TEXT NOT NULL,
            organization_coordsys_id INTEGER NOT NULL,
            definition TEXT NOT NULL,
            description TEXT
        );
        INSERT INTO gpkg_spatial_ref_sys VALUES (4490, 'CGCS2000', 'GEODETIC', 'EPSG', 4490,
            'GEOGCS[\"CGCS2000\",DATUM[\"China_2000\",SPHEROID[\"CGCS2000\",6378137,298.257222101]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433]]',
            'China Geodetic Coordinate System 2000');
        INSERT INTO gpkg_spatial_ref_sys VALUES (4326, 'WGS84', 'GEODETIC', 'EPSG', 4326,
            'GEOGCS[\"WGS84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433]]',
            'WGS 84');

        CREATE TABLE gpkg_contents (
            table_name TEXT NOT NULL PRIMARY KEY,
            data_type TEXT NOT NULL,
            identifier TEXT,
            description TEXT DEFAULT '',
            last_change DATETIME NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
            min_x DOUBLE, min_y DOUBLE, max_x DOUBLE, max_y DOUBLE,
            srs_id INTEGER REFERENCES gpkg_spatial_ref_sys(srs_id)
        );

        CREATE TABLE gpkg_geometry_columns (
            table_name TEXT NOT NULL,
            column_name TEXT NOT NULL,
            geometry_type_name TEXT NOT NULL,
            srs_id INTEGER NOT NULL,
            z TINYINT NOT NULL,
            m TINYINT NOT NULL,
            CONSTRAINT pk_geom_cols PRIMARY KEY (table_name, column_name)
        );
    ").map_err(|e| format!("创建元数据表失败: {}", e))?;

    // 构建用户字段的 SQL
    let field_defs: Vec<String> = fields.iter().map(|(name, _desc, type_code, width)| {
        let sql_type = match type_code {
            3 => "REAL".to_string(),  // Float64
            _ => format!("TEXT({})", width),
        };
        format!("\"{}\" {}", name, sql_type)
    }).collect();

    let all_cols = field_defs.join(", ");
    let create_sql = format!(
        "CREATE TABLE \"{}\" (\"geom\" BLOB NOT NULL, {}, CONSTRAINT pk_{}_rowid PRIMARY KEY (\"rowid\"))",
        base_name, all_cols, base_name
    );

    conn.execute("SELECT enable_extension('gpkg')", []).ok();
    conn.execute(&create_sql, [])
        .map_err(|e| format!("创建要素表失败: {}", e))?;

    // 注册到元数据
    conn.execute(
        "INSERT INTO gpkg_contents (table_name, data_type, identifier, srs_id) VALUES (?1, 'features', ?2, 4490)",
        rusqlite::params![base_name, base_name],
    ).map_err(|e| format!("注册 gpkg_contents 失败: {}", e))?;

    conn.execute(
        "INSERT INTO gpkg_geometry_columns (table_name, column_name, geometry_type_name, srs_id, z, m) VALUES (?1, 'geom', 'POLYGON', 4490, 0, 0)",
        rusqlite::params![base_name],
    ).map_err(|e| format!("注册 gpkg_geometry_columns 失败: {}", e))?;

    // 插入要素
    let val_placeholders: Vec<String> = fields.iter().enumerate()
        .map(|(i, _)| format!("?{}", i + 2))
        .collect();
    let insert_sql = format!(
        "INSERT INTO \"{}\" (\"geom\", {}) VALUES (?1, {})",
        base_name, all_cols, val_placeholders.join(", ")
    );

    let mut insert_stmt = conn.prepare(&insert_sql)
        .map_err(|e| format!("准备插入语句失败: {}", e))?;

    for (fi, coords) in geometries.iter().enumerate() {
        if coords.len() < 3 {
            continue;
        }

        let gpkg_geom = encode_gpkg_geom(coords);

        let mut field_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        field_values.push(Box::new(gpkg_geom)); // ?1 = geom

        for (name, _desc, type_code, _width) in fields {
            let str_val = attributes.get(fi)
                .and_then(|a| a.get(name.as_str()))
                .map(|s| s.as_str())
                .unwrap_or("");

            match type_code {
                3 => {
                    let fval: f64 = str_val.parse().unwrap_or(0.0);
                    field_values.push(Box::new(fval));
                }
                _ => {
                    field_values.push(Box::new(str_val.to_string()));
                }
            }
        }

        let params: Vec<&dyn rusqlite::types::ToSql> = field_values.iter()
            .map(|b| b.as_ref())
            .collect();

        insert_stmt.execute(params.as_slice())
            .map_err(|e| format!("插入要素失败: {}", e))?;
    }

    // 更新范围
    let mut min_x = f64::MAX; let mut min_y = f64::MAX;
    let mut max_x = f64::MIN; let mut max_y = f64::MIN;
    for coords in geometries {
        for &(x, y) in coords {
            if x < min_x { min_x = x; }
            if y < min_y { min_y = y; }
            if x > max_x { max_x = x; }
            if y > max_y { max_y = y; }
        }
    }
    if min_x != f64::MAX {
        conn.execute(
            "UPDATE gpkg_contents SET min_x = ?1, min_y = ?2, max_x = ?3, max_y = ?4 WHERE table_name = ?5",
            rusqlite::params![min_x, min_y, max_x, max_y, base_name],
        ).ok();
    }

    drop(insert_stmt);
    drop(conn);

    Ok(vec![gpkg_path.to_string_lossy().to_string()])
}

// ─── 预览用 ───

/// 读取 GeoPackage 信息，返回字段列表和要素数
pub fn read_gpkg_source_info(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>, usize), String> {
    let info = read_gpkg(path)?;
    let field_names = info.all_field_names.first().cloned().unwrap_or_default();
    let field_records: Vec<Vec<String>> = info.all_features.first()
        .map(|features| {
            features.iter().map(|f| {
                field_names.iter().map(|n| f.attributes.get(n).cloned().unwrap_or_default()).collect()
            }).collect()
        })
        .unwrap_or_default();
    let num_features = info.layers.first().map(|l| l.num_features).unwrap_or(0);
    Ok((field_names, field_records, num_features))
}
