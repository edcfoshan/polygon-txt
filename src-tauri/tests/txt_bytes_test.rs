// TXT 输出字节基线 + 预览/导出同源 + 读取一致性。
//
// 目的：Batch 2 的「坐标单次变换」「点号编号复用」「输出免逐格分配」等重构
// 都必须保持 TXT 字节完全一致——本文件用自造 SHP（ShapeWriter，不依赖任何
// fixture）把标准 4 列输出的**完整文本**钉死，任何字节漂移都会失败。
//
// 另含两处此前无人覆盖的行为：
//   1) 预览 == 导出结果的前 2000 行（convert.rs 的 take(2000) 截断口径）
//   2) num_features 与 read_shp 的要素数一致 + 范围与 .shp 头部 bbox 一致

extern crate jisig_bpoint_converter_lib;

use jisig_bpoint_converter_lib::{convert, shp};
use shapefile::{Point, Polygon, PolygonRing, ShapeWriter};
use std::path::{Path, PathBuf};

fn make_header() -> convert::HeaderConfig {
    convert::HeaderConfig {
        attrs: vec![
            convert::AttrRow { k: "坐标系".into(), v: "2000国家大地坐标系".into() },
            convert::AttrRow { k: "几度分带".into(), v: "3".into() },
            convert::AttrRow { k: "投影类型".into(), v: "高斯克吕格".into() },
            convert::AttrRow { k: "计量单位".into(), v: "米".into() },
            convert::AttrRow { k: "带号".into(), v: "38".into() },
            convert::AttrRow { k: "精度".into(), v: "0.001".into() },
            convert::AttrRow { k: "转换参数".into(), v: ",,,,,,".into() },
        ],
        project_info: String::new(),
    }
}

fn make_mapping() -> convert::FieldMapping {
    convert::FieldMapping {
        name: String::new(), id: String::new(), area: String::new(),
        use_field: String::new(), tfh: String::new(), dlbm: String::new(),
        columns: Vec::new(),
    }
}

fn base_options() -> convert::ShpToTxtOptions {
    convert::ShpToTxtOptions {
        proj_mode: "keep".to_string(),
        proj_zone: None,
        ox: false, oj: true, on: false, oo: true, oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
        og: false, zone_type: 3, proj_no_prefix: false,
        plot_filter: None, point_layout: None,
    }
}

/// 以 (x0, y0) 为西南角写一个 100×100 的矩形环（闭合）。
/// extra: 沿上边界额外插入的点数（用于把输出撑到 2000 行以上）
fn write_rect_shp(dir: &Path, name: &str, x0: f64, y0: f64, extra: usize) -> PathBuf {
    let mut pts: Vec<Point> = vec![
        Point::new(x0, y0),
        Point::new(x0 + 100.0, y0),
        Point::new(x0 + 100.0, y0 + 100.0),
    ];
    for i in 0..extra {
        let t = (i + 1) as f64 / (extra + 1) as f64;
        pts.push(Point::new(x0 + 100.0 * (1.0 - t), y0 + 100.0));
    }
    pts.push(Point::new(x0, y0 + 100.0));
    pts.push(Point::new(x0, y0)); // 闭合

    let shp_path = dir.join(format!("{}.shp", name));
    let mut w = ShapeWriter::from_path(&shp_path).expect("创建 ShapeWriter");
    w.write_shape(&Polygon::with_rings(vec![PolygonRing::Outer(pts)]))
        .expect("写 Polygon");
    drop(w);
    shp_path
}

// ─── T3：标准 4 列输出的完整字节基线 ───

#[test]
fn one_to_one_txt_bytes_are_stable() {
    let shp_dir = tempfile::tempdir().expect("shp tmp");
    let out_dir = tempfile::tempdir().expect("out tmp");
    let shp_path = write_rect_shp(shp_dir.path(), "bytes_base", 500_000.0, 3_000_000.0, 0);

    let result = convert::convert_shp_to_txt(
        &[shp_path], None, None, &make_header(), &make_mapping(), &base_options(),
        out_dir.path(), None,
    ).expect("一对一转换失败");

    assert!(result.success, "转换应成功");
    assert_eq!(result.output_files.len(), 1, "一对一应只产出一个文件");
    let txt = std::fs::read_to_string(&result.output_files[0]).expect("读输出");

    // 基准文本在下方以字符串常量钉死；末尾换行单独断言，正文逐行对比
    assert!(txt.ends_with('\n'), "输出应以换行结尾");
    let body = &txt[..txt.len() - 1];
    assert!(!body.ends_with('\n'), "输出末尾不应有多余空行");

    eprintln!("__ACTUAL_BEGIN__\n{}\n__ACTUAL_END__", body);

    let expected = EXPECTED_ONE_TO_ONE;
    if body != expected {
        // 便于定位：逐行对比并打印首个不同行
        let (a, b): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), body.lines().collect());
        for i in 0..a.len().max(b.len()) {
            let (ea, eb) = (a.get(i).unwrap_or(&"<缺失>"), b.get(i).unwrap_or(&"<缺失>"));
            if ea != eb {
                panic!("TXT 字节漂移：第 {} 行\n  期望: {:?}\n  实际: {:?}\n完整实际输出:\n{}", i + 1, ea, eb, body);
            }
        }
        panic!("TXT 行数不一致：期望 {} 行，实际 {} 行", a.len(), b.len());
    }
}

/// 标准 4 列输出基准（oj=true 点号加 J，oo=true 首末点重合，on=false 不旋转起点）
/// 由「自造矩形环」在 HEAD 上的实际输出冻结而来；改动输出格式时必须同步更新这里。
const EXPECTED_ONE_TO_ONE: &str = r#"[属性描述]
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
带号=38
精度=0.001
转换参数=,,,,,,
[地块坐标]
5,,1,,面,,,,@
J1,1,3000000.000,500000.000
J2,1,3000100.000,500000.000
J3,1,3000100.000,500100.000
J4,1,3000000.000,500100.000
J1,1,3000000.000,500000.000"#;

// ─── T4：预览 == 导出结果的前 2000 行 ───

#[test]
fn preview_equals_first_2000_lines_of_export() {
    let shp_dir = tempfile::tempdir().expect("shp tmp");
    let out_dir = tempfile::tempdir().expect("out tmp");
    // 2100+ 点 → 输出行数超过 2000，触发预览截断
    let shp_path = write_rect_shp(shp_dir.path(), "long_ring", 500_000.0, 3_000_000.0, 2100);

    let full = convert::convert_shp_to_txt(
        &[shp_path.clone()], None, None, &make_header(), &make_mapping(), &base_options(),
        out_dir.path(), None,
    ).expect("导出失败");
    let full_txt = std::fs::read_to_string(&full.output_files[0]).expect("读导出");

    let preview = convert::shp_to_txt_preview(
        &[shp_path], None, None, &make_header(), &make_mapping(), &base_options(), None,
    ).expect("预览失败");

    let expected: String = full_txt.lines().take(2000).collect::<Vec<_>>().join("\n");
    assert!(full_txt.lines().count() > 2000, "本用例需要超过 2000 行输出，实际 {} 行",
        full_txt.lines().count());
    assert_eq!(preview.lines().count(), 2000, "预览应恰好截断到 2000 行");
    assert_eq!(preview, expected, "预览必须等于导出的前 2000 行（预览与导出同源）");
}

// ─── T5：读取一致性（保护 S4 的单次读取改造）───

#[test]
fn read_shp_consistency_and_extent_matches_header() {
    let shp_dir = tempfile::tempdir().expect("shp tmp");
    let shp_path = write_rect_shp(shp_dir.path(), "consistency", 500_000.0, 3_000_000.0, 7);

    let (info, _features) = shp::read_shp_file_group(&shp_path).expect("读 SHP 组");
    let features = shp::read_shp(&shp_path).expect("读 SHP 要素");

    // 同一文件的两种读法必须报告同样的要素数（S4 让二者共用同一次解析）
    assert_eq!(info.num_features, features.len(), "num_features 与要素数必须一致");
    assert_eq!(info.num_features, 1, "本用例只写了一个面要素");

    // 范围语义：由要素坐标算出的 bbox 必须等于 .shp 头部写的 bbox
    let bytes = std::fs::read(&shp_path).expect("读 SHP 字节");
    let f64_at = |off: usize| -> f64 {
        f64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
    };
    let (h_xmin, h_ymin, h_xmax, h_ymax) = (f64_at(36), f64_at(44), f64_at(52), f64_at(60));

    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for f in &features {
        for part in &f.surface.parts {
            for (x, y) in &part.exterior {
                xs.push(*x);
                ys.push(*y);
            }
        }
    }
    let min = |v: &[f64]| v.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = |v: &[f64]| v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let eps = 1e-6;
    assert!((min(&xs) - h_xmin).abs() < eps, "xmin: 要素 {} vs 头部 {}", min(&xs), h_xmin);
    assert!((min(&ys) - h_ymin).abs() < eps, "ymin: 要素 {} vs 头部 {}", min(&ys), h_ymin);
    assert!((max(&xs) - h_xmax).abs() < eps, "xmax: 要素 {} vs 头部 {}", max(&xs), h_xmax);
    assert!((max(&ys) - h_ymax).abs() < eps, "ymax: 要素 {} vs 头部 {}", max(&ys), h_ymax);
}
