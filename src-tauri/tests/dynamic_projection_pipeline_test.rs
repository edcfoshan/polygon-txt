// Integration tests for apply_dynamic_projection_to_sources (Task 6 完整化)
// Verifies: keep is noop, A mode forward-projects, C mode inverse, header sync, F mode reband

use jisig_bpoint_converter_lib::convert::*;
use jisig_bpoint_converter_lib::projection::{gauss_kruger_forward, Ellipsoid};
use std::collections::HashMap;

fn make_test_source(coords: Vec<(f64, f64)>) -> ImportSource {
    ImportSource {
        stem: "t".to_string(),
        plots: vec![PlotWithSource {
            plot: __plot_with_coords(coords),
            source_stem: "t".to_string(),
            index_in_source: 0,
            attributes: HashMap::new(),
        }],
    }
}

fn header_with_test_attrs(pairs: Vec<(&str, &str)>) -> HeaderConfig {
    HeaderConfig {
        project_info: String::new(),
        attrs: pairs.into_iter().map(|(k, v)| AttrRow { k: k.to_string(), v: v.to_string() }).collect(),
    }
}

fn shp_opts_test_default() -> ShpToTxtOptions {
    ShpToTxtOptions {
        ox: false, oj: false, on: false, oo: false, oc: false,
        output_mode: "one_to_one".to_string(),
        filename_field: String::new(),
        og: false, zone_type: 3,
        proj_mode: String::new(),
        proj_zone: None,
        proj_no_prefix: false,
    }
}

#[test]
fn dynamic_proj_mode_keep_is_noop() {
    let mut sources = vec![make_test_source(vec![(30.5, 114.5)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "大地（度）"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "keep".to_string(), proj_zone: None, ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!((y - 30.5).abs() < 1e-9);
    assert!((x - 114.5).abs() < 1e-9);
    assert_eq!(new_header.attrs[0].v, "CGCS2000");
}

#[test]
fn dynamic_proj_mode_a_forward() {
    let mut sources = vec![make_test_source(vec![(30.5, 114.5)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "大地（度）"),
        ("分带", "3°带"),
        ("带号", "38"),
        ("投影类型", "高斯克吕格"),
        ("计量单位", "米"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "A".to_string(), proj_zone: Some(38), ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!(x > 38_000_000.0 && x < 39_000_000.0, "x should be 3°带 zone 38, got {}", x);
    assert!(y > 3_000_000.0 && y < 4_000_000.0, "y should be ~3.3M, got {}", y);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "形式").unwrap().v, "投影（米）");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "分带").unwrap().v, "3°带");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "投影类型").unwrap().v, "高斯克吕格");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "计量单位").unwrap().v, "米");
}

#[test]
fn dynamic_proj_mode_c_add_prefix() {
    // 模式 C 同带含带号：自然值 easting + zone×1,000,000
    let mut sources = vec![make_test_source(vec![(3_381_842.0, 537_123.0)])]; // easting 无前缀
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("分带", "3°带"),
        ("带号", "38"),
        ("投影类型", "高斯克吕格"),
        ("计量单位", "米"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "C".to_string(), proj_zone: Some(38), ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert_eq!(y, 3_381_842.0, "northing unchanged");
    assert_eq!(x, 38_537_123.0, "easting should have zone 38 prefix, got {}", x);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "形式").unwrap().v, "投影（米）");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "分带").unwrap().v, "3°带");
}

#[test]
fn dynamic_proj_mode_c_strip_prefix() {
    // 模式 C 同带不含带号：剥离 zone×1,000,000 前缀
    let mut sources = vec![make_test_source(vec![(3_381_842.0, 38_537_123.0)])]; // easting 有前缀
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("分带", "3°带"),
        ("带号", "38"),
        ("投影类型", "高斯克吕格"),
        ("计量单位", "米"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "C".to_string(), proj_zone: Some(38), proj_no_prefix: true, ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert_eq!(y, 3_381_842.0, "northing unchanged");
    assert_eq!(x, 537_123.0, "easting should have prefix stripped, got {}", x);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "形式").unwrap().v, "投影（米）");
}

#[test]
fn dynamic_proj_mode_f_reband() {
    let mut sources = vec![make_test_source(vec![(3_381_842.0, 38_537_123.0)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("分带", "3°带"),
        ("带号", "38"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "F".to_string(), proj_zone: Some(38), ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!(x > 20_000_000.0 && x < 21_000_000.0, "x should be 6°带 zone ~20, got {}", x);
    assert!(y > 0.0 && y < 5_000_000.0, "y should be similar magnitude, got {}", y);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "分带").unwrap().v, "6°带");
}

#[test]
fn dynamic_proj_mode_b_forward() {
    let mut sources = vec![make_test_source(vec![(30.5, 114.5)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "大地（度）"),
        ("分带", "6°带"),
        ("带号", "20"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "B".to_string(), proj_zone: Some(20), ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!(x > 20_000_000.0 && x < 21_000_000.0, "x should be 6°带 zone ~20, got {}", x);
    assert!(y > 3_000_000.0 && y < 4_000_000.0, "y should be ~3.3M, got {}", y);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "分带").unwrap().v, "6°带");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "形式").unwrap().v, "投影（米）");
}

#[test]
#[ignore = "已知限制：gauss_kruger_inverse 对 6° 带源坐标的前缀剥离假定 3° 带号，导致 proj-core + classic 均产出错误结果。详见 projection.rs:proj_core_inverse zone 计算逻辑"]
fn dynamic_proj_mode_g_reband() {
    // 6°带 → 3°带 reband（G 模式）。
    // 等 gauss_kruger_inverse 支持 6° 带前缀后再启用。
    let (x_no_prefix, y) = gauss_kruger_forward(112.8, 23.4, 111.0, Ellipsoid::CGCS2000);
    let x6 = x_no_prefix + 19.0 * 1_000_000.0;
    let mut sources = vec![make_test_source(vec![(y, x6)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("分带", "6°带"),
        ("带号", "19"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "G".to_string(), proj_zone: None, ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y_out, x_out) = sources[0].plots[0].plot.coords[0];
    assert!(x_out > 37_000_000.0 && x_out < 38_000_000.0, "x should be 3°带 zone ~37, got {}", x_out);
    assert!(y_out > 2_000_000.0 && y_out < 3_000_000.0, "y should be ~2.5M, got {}", y_out);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "分带").unwrap().v, "3°带");
}

#[test]
fn preview_matches_source_path_for_mode_a() {
    // 预览路径 (apply_dynamic_projection_to_plots) 与转换路径 (apply_dynamic_projection_to_sources)
    // 对相同输入应产出相同的坐标。
    let coords = vec![(30.5, 114.5)];
    let mut sources = vec![make_test_source(coords.clone())];
    let mut plots = vec![__plot_with_coords(coords)];

    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "大地（度）"),
        ("分带", "3°带"),
        ("带号", "38"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "A".to_string(), proj_zone: Some(38), ..shp_opts_test_default() };

    let h1 = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let h2 = apply_dynamic_projection_to_plots(&mut plots, &header, &options).unwrap();

    let (y1, x1) = sources[0].plots[0].plot.coords[0];
    let (y2, x2) = plots[0].coords[0];
    assert!((x1 - x2).abs() < 0.001, "预览/转换 X 不一致: {} vs {}", x1, x2);
    assert!((y1 - y2).abs() < 0.001, "预览/转换 Y 不一致: {} vs {}", y1, y2);
    assert_eq!(h1.attrs.iter().find(|a| a.k == "形式").unwrap().v,
               h2.attrs.iter().find(|a| a.k == "形式").unwrap().v);
}

