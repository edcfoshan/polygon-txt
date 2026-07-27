// Integration tests for apply_dynamic_projection_to_sources (Task 6 完整化)
// Verifies: keep is noop, A mode forward-projects, C mode inverse, header sync, F mode reband

use jisig_bpoint_converter_lib::convert::*;
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
        proj_qc: false,
    }
}

#[test]
fn dynamic_proj_mode_keep_is_noop() {
    let mut sources = vec![make_test_source(vec![(30.5, 114.5)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "大地（度）"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "keep".to_string(), proj_zone: None, proj_qc: false, ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options);
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
    let options = ShpToTxtOptions { proj_mode: "A".to_string(), proj_zone: Some(38), proj_qc: false, ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options);
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!(x > 38_000_000.0 && x < 39_000_000.0, "x should be 3°带 zone 38, got {}", x);
    assert!(y > 3_000_000.0 && y < 4_000_000.0, "y should be ~3.3M, got {}", y);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "形式").unwrap().v, "投影（米）");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "分带").unwrap().v, "3°带");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "投影类型").unwrap().v, "高斯克吕格");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "计量单位").unwrap().v, "米");
}

#[test]
fn dynamic_proj_mode_c_inverse() {
    let mut sources = vec![make_test_source(vec![(3_381_842.0, 38_537_123.0)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("分带", "3°带"),
        ("带号", "38"),
        ("投影类型", "高斯克吕格"),
        ("计量单位", "米"),
        ("投影类型", "高斯克吕格"),
        ("计量单位", "米"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "C".to_string(), proj_zone: None, proj_qc: false, ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options);
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!(x < 200.0, "x should be degrees (<200), got {}", x);
    assert!(y > 0.0 && y < 90.0, "y should be degrees (0-90), got {}", y);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "形式").unwrap().v, "大地（度）");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "分带").unwrap().v, "—");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "投影类型").unwrap().v, "—");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "计量单位").unwrap().v, "—");
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
    let options = ShpToTxtOptions { proj_mode: "F".to_string(), proj_zone: Some(38), proj_qc: false, ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options);
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!(x > 20_000_000.0 && x < 21_000_000.0, "x should be 6°带 zone ~20, got {}", x);
    assert!(y > 0.0 && y < 5_000_000.0, "y should be similar magnitude, got {}", y);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "分带").unwrap().v, "6°带");
}
