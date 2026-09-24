// GDB 读→写往返、写出确定性、以及真实 ArcPy fixture 的元数据往返。
//
// 背景：GDB 写路径此前只有 debug_output_test 打印信息、没有任何断言；
// Batch 2 的 S1（坐标单次变换）与 S2（点号编号复用）会同时影响 SHP / GDB 两条源路径，
// 因此在动它们之前先把「写出 → 读回」的语义钉死。
//
// 三个已探明的事实（写进断言与注释，避免后人误判）：
//   1) 手写 writer 不输出字段别名（write_gdb_output 的 desc 参数被忽略）→ 别名不参与往返断言；
//   2) 写出是确定性的，唯一例外是 a00000004.gdbtable（GDB_Items）里 8 字节写入时间戳
//      （100ns FILETIME）→ GDB 不适合做整目录字节 golden；
//   3) 入库的 test_arcpy/test.gdb 五个图层全是 0 要素（extent 为 NaN），
//      故要素级覆盖由本文件的自造往返用例承担。

extern crate jisig_bpoint_converter_lib;

use jisig_bpoint_converter_lib::gdb;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn sample_fields() -> Vec<(String, String, u8, u32)> {
    vec![
        ("DKMC".to_string(), "地块名称".to_string(), 4u8, 50u32),
        ("DKBH".to_string(), "地块编号".to_string(), 4u8, 30u32),
        ("MJ".to_string(), "面积".to_string(), 3u8, 14u32),
    ]
}

fn sample_attrs() -> Vec<HashMap<String, String>> {
    let mut a = HashMap::new();
    a.insert("DKMC".to_string(), "测试地块".to_string());
    a.insert("DKBH".to_string(), "DKBH-001".to_string());
    a.insert("MJ".to_string(), "1234.56".to_string());
    // 空字符串也要能往返（前端允许字段留空）
    let mut b = HashMap::new();
    b.insert("DKMC".to_string(), "第二个地块".to_string());
    b.insert("DKBH".to_string(), String::new());
    vec![a, b]
}

fn sample_geoms() -> Vec<Vec<(f64, f64)>> {
    vec![
        vec![
            (38_383_243.971, 2_582_988.976),
            (38_383_261.067, 2_582_983.339),
            (38_383_048.719, 2_582_359.231),
            (38_383_061.719, 2_582_359.231),
            (38_383_243.971, 2_582_988.976),
        ],
        vec![
            (38_385_000.0, 2_583_000.0),
            (38_385_100.0, 2_583_000.0),
            (38_385_100.0, 2_583_100.0),
            (38_385_000.0, 2_583_100.0),
            (38_385_000.0, 2_583_000.0),
        ],
    ]
}

fn sample_crs() -> HashMap<String, String> {
    let mut c = HashMap::new();
    c.insert("c".to_string(), "2000国家大地坐标系".to_string());
    c.insert("b".to_string(), "3".to_string());
    c.insert("z".to_string(), "38".to_string());
    c
}

fn bbox(pts: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let mut b = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(x, y) in pts {
        b.0 = b.0.min(x);
        b.1 = b.1.min(y);
        b.2 = b.2.max(x);
        b.3 = b.3.max(y);
    }
    b
}

fn ring_of(f: &gdb::GdbFeature) -> Vec<(f64, f64)> {
    f.surface.parts.iter().flat_map(|p| p.exterior.clone()).collect()
}

/// 递归收集目录下的 (相对路径, 字节)，用于逐文件字节比较
fn dir_snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("读目录") {
            let path = entry.expect("目录项").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(root).expect("相对路径").to_string_lossy().to_string();
                out.push((rel, std::fs::read(&path).expect("读文件")));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().expect("repo root").to_path_buf()
}

// ─── 写出 → 读回：图层/字段/属性（含空值）/坐标范围 ───

#[test]
fn gdb_write_then_read_roundtrip() {
    let dir = tempfile::tempdir().expect("tmp");
    let files = gdb::write_gdb_output(
        dir.path(), "rt", &sample_fields(), &sample_attrs(), &sample_geoms(), &sample_crs(),
    ).expect("写 GDB 失败");
    assert!(!files.is_empty(), "写出应返回文件列表");

    let gdb_dir = dir.path().join("rt.gdb");
    assert!(gdb_dir.join("a00000001.gdbtable").exists(), "应写出系统表 a00000001.gdbtable");

    let info = gdb::read_gdb(&gdb_dir).expect("读回 GDB 失败");
    assert_eq!(info.layers.len(), 1, "单层写出应读回 1 个图层");
    let layer = &info.layers[0];
    assert!(!layer.name.is_empty(), "图层名不应为空");
    assert_eq!(layer.num_features, 2, "要素数应往返一致");
    assert!(
        layer.geometry_type.to_lowercase().contains("polygon"),
        "几何类型应为面状，实际 {}",
        layer.geometry_type
    );

    // 字段名往返（手写 writer 不输出字段别名，故此处不断言 field_aliases）
    for (name, _alias, _, _) in sample_fields() {
        assert!(
            layer.field_names.contains(&name),
            "字段 {} 应往返，实际 {:?}",
            name, layer.field_names
        );
    }

    let feats = &info.all_features[0];
    assert_eq!(feats.len(), 2, "要素数应与写出时一致");
    assert_eq!(feats[0].attributes.get("DKMC").map(|s| s.as_str()), Some("测试地块"));
    assert_eq!(feats[0].attributes.get("MJ").map(|s| s.as_str()), Some("1234.56"));
    assert_eq!(feats[1].attributes.get("DKMC").map(|s| s.as_str()), Some("第二个地块"));
    assert_eq!(
        feats[1].attributes.get("DKBH").map(|s| s.as_str()),
        Some(""),
        "空字符串属性值应原样往返"
    );

    let geoms = sample_geoms();
    for (i, f) in feats.iter().enumerate() {
        let pts = ring_of(f);
        assert!(!pts.is_empty(), "要素 {} 应读回环坐标", i);
        assert_eq!(bbox(&pts), bbox(&geoms[i]), "要素 {} 的坐标范围应往返一致", i);
    }
}

// ─── 写出确定性：除 GDB_Items 的写入时间戳外，逐文件字节一致 ───

#[test]
fn gdb_write_is_deterministic_except_items_timestamp() {
    let d1 = tempfile::tempdir().expect("tmp1");
    let d2 = tempfile::tempdir().expect("tmp2");
    gdb::write_gdb_output(d1.path(), "det", &sample_fields(), &sample_attrs(), &sample_geoms(), &sample_crs())
        .expect("第一次写出失败");
    gdb::write_gdb_output(d2.path(), "det", &sample_fields(), &sample_attrs(), &sample_geoms(), &sample_crs())
        .expect("第二次写出失败");

    let s1 = dir_snapshot(&d1.path().join("det.gdb"));
    let s2 = dir_snapshot(&d2.path().join("det.gdb"));
    assert!(!s1.is_empty(), "应写出文件");
    assert_eq!(
        s1.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
        s2.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(),
        "两次写出的文件清单应一致"
    );

    let mut nondet = Vec::new();
    for ((path, a), (_, b)) in s1.iter().zip(s2.iter()) {
        assert_eq!(a.len(), b.len(), "文件 {} 字节数应一致", path);
        let diff: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
        if diff.is_empty() {
            continue;
        }
        // GDB_Items 记录写入时间（100ns FILETIME，创建/修改两处）→ 仅该表允许少量字节差异
        assert!(
            path.ends_with("a00000004.gdbtable") && diff.len() <= 32,
            "文件 {} 出现 {} 字节差异（首个偏移 {}）——除 GDB_Items 时间戳外，写出必须是确定性的",
            path, diff.len(), diff[0]
        );
        nondet.push(path.clone());
    }
    assert!(nondet.len() <= 1, "至多一个文件允许非确定，实际 {:?}", nondet);
}

// ─── 真实 ArcPy fixture：元数据往返（其图层为 0 要素，见文件头注释）───

#[test]
fn gdb_fixture_metadata_roundtrips() {
    let fixture = repo_root().join("test_arcpy").join("test.gdb");
    assert!(
        fixture.join("a00000001.gdbtable").exists(),
        "缺少入库 fixture：{}（test_arcpy 已入库，不应缺失）",
        fixture.display()
    );

    let src = gdb::read_gdb(&fixture).expect("读 fixture GDB 失败");
    assert!(!src.layers.is_empty(), "fixture 应至少有一个图层");

    // 真实 ArcGIS 产物：图层名与字段名必须被正确解析
    let names: Vec<&str> = src.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["plot_000", "plot_001", "plot_002", "plot_003", "plot_004"],
        "fixture 的 5 个图层名应被正确读出，实际 {:?}",
        names
    );
    for l in &src.layers {
        for expect in ["OBJECTID", "DKMC", "DKBH", "MJ", "DKYT", "TFH", "DLBM", "Shape_Area"] {
            assert!(
                l.field_names.iter().any(|f| f == expect),
                "图层 {} 应含字段 {}，实际 {:?}",
                l.name, expect, l.field_names
            );
        }
    }

    // 空图层也要能走完读→写→读
    let layer = &src.layers[0];
    let feats = &src.all_features[0];
    assert_eq!(feats.len(), layer.num_features, "要素数与 all_features 长度应一致");

    let out = tempfile::tempdir().expect("tmp");
    gdb::write_gdb_output(
        out.path(), "fixture_copy",
        &layer.field_names.iter().map(|n| (n.clone(), String::new(), 4u8, 50u32)).collect::<Vec<_>>(),
        &feats.iter().map(|f| f.attributes.clone()).collect::<Vec<_>>(),
        &feats.iter().map(ring_of).collect::<Vec<_>>(),
        &layer.crs_info,
    ).expect("把 fixture 图层写出失败");

    let back = gdb::read_gdb(&out.path().join("fixture_copy.gdb")).expect("读回写出结果失败");
    assert_eq!(back.layers.len(), 1, "应读回 1 个图层");
    assert_eq!(back.layers[0].num_features, layer.num_features, "要素数应在读→写→读后保持一致");
    // 不比较字段数：read_gdb 给出的 field_names 含 OBJECTID/Shape_Length 等系统字段，
    // 把它们当用户字段写回会与 writer 自建的系统字段重复（应用里不存在这条路径：
    // GDB 只作为输入格式，用户字段清单由字段映射决定）
}
