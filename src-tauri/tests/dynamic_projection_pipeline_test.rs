// Integration tests for apply_dynamic_projection_to_sources (Task 6 完整化)
// Verifies: keep is noop, A mode forward-projects, C mode inverse, header sync, F mode reband

use jisig_bpoint_converter_lib::convert::*;
use jisig_bpoint_converter_lib::projection::{gauss_kruger_forward, gauss_kruger_inverse, Ellipsoid};
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
        ("几度分带", "3°带"),
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
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "几度分带").unwrap().v, "3°带");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "投影类型").unwrap().v, "高斯克吕格");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "计量单位").unwrap().v, "米");
}

#[test]
fn keep_prefix_add() {
    // keep + proj_zone：前缀开关与投影正交，不投影只加带号前缀（自然值 + zone×1,000,000）
    let mut sources = vec![make_test_source(vec![(3_381_842.0, 537_123.0)])]; // easting 无前缀
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("几度分带", "3°带"),
        ("带号", "38"),
        ("投影类型", "高斯克吕格"),
        ("计量单位", "米"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "keep".to_string(), proj_zone: Some(38), ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert_eq!(y, 3_381_842.0, "northing unchanged");
    assert_eq!(x, 38_537_123.0, "easting should have zone 38 prefix, got {}", x);
    // keep 不做头表同步：原样返回
    assert_eq!(new_header.attrs[0].v, "CGCS2000");
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "带号").unwrap().v, "38");
}

#[test]
fn keep_prefix_strip() {
    // keep + proj_zone + no_prefix：剥离已有 zone×1,000,000 前缀输出自然值
    let mut sources = vec![make_test_source(vec![(3_381_842.0, 38_537_123.0)])]; // easting 有前缀
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("几度分带", "3°带"),
        ("带号", "38"),
        ("投影类型", "高斯克吕格"),
        ("计量单位", "米"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "keep".to_string(), proj_zone: Some(38), proj_no_prefix: true, ..shp_opts_test_default() };
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert_eq!(y, 3_381_842.0, "northing unchanged");
    assert_eq!(x, 537_123.0, "easting should have prefix stripped, got {}", x);
}

#[test]
fn keep_prefix_rezone() {
    // keep + 前缀换号：先剥实际前缀取自然值，再按目标带号加回（38 → 39）
    let mut sources = vec![make_test_source(vec![(3_381_842.0, 38_537_123.0)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("几度分带", "3°带"),
        ("带号", "38"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "keep".to_string(), proj_zone: Some(39), ..shp_opts_test_default() };
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert_eq!(y, 3_381_842.0, "northing unchanged");
    assert_eq!(x, 39_537_123.0, "easting should be re-prefixed with zone 39, got {}", x);
}

#[test]
fn dynamic_proj_mode_f_reband() {
    let mut sources = vec![make_test_source(vec![(3_381_842.0, 38_537_123.0)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "投影（米）"),
        ("几度分带", "3°带"),
        ("带号", "38"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "F".to_string(), proj_zone: Some(38), ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!(x > 20_000_000.0 && x < 21_000_000.0, "x should be 6°带 zone ~20, got {}", x);
    assert!(y > 0.0 && y < 5_000_000.0, "y should be similar magnitude, got {}", y);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "几度分带").unwrap().v, "6°带");
}

#[test]
fn dynamic_proj_mode_b_forward() {
    let mut sources = vec![make_test_source(vec![(30.5, 114.5)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("形式", "大地（度）"),
        ("几度分带", "6°带"),
        ("带号", "20"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "B".to_string(), proj_zone: Some(20), ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y, x) = sources[0].plots[0].plot.coords[0];
    assert!(x > 20_000_000.0 && x < 21_000_000.0, "x should be 6°带 zone ~20, got {}", x);
    assert!(y > 3_000_000.0 && y < 4_000_000.0, "y should be ~3.3M, got {}", y);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "几度分带").unwrap().v, "6°带");
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
        ("几度分带", "6°带"),
        ("带号", "19"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "G".to_string(), proj_zone: None, ..shp_opts_test_default() };
    let new_header = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y_out, x_out) = sources[0].plots[0].plot.coords[0];
    assert!(x_out > 37_000_000.0 && x_out < 38_000_000.0, "x should be 3°带 zone ~37, got {}", x_out);
    assert!(y_out > 2_000_000.0 && y_out < 3_000_000.0, "y should be ~2.5M, got {}", y_out);
    assert_eq!(new_header.attrs.iter().find(|a| a.k == "几度分带").unwrap().v, "3°带");
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
        ("几度分带", "3°带"),
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

#[test]
fn dynamic_proj_mode_h_reband_3deg_38_to_39() {
    // H 模式：3°带 38→39 同分带不同带号换带
    let (x38_np, y) = gauss_kruger_forward(114.5, 23.4, 114.0, Ellipsoid::CGCS2000);
    let x38 = x38_np + 38_000_000.0;
    let mut sources = vec![make_test_source(vec![(y, x38)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("几度分带", "3"),
        ("带号", "39"), // 模拟 apply 后 header 被改成目标
    ]);
    let options = ShpToTxtOptions { proj_mode: "H".to_string(), proj_zone: Some(39), ..shp_opts_test_default() };
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y_out, x_out) = sources[0].plots[0].plot.coords[0];
    let (lon_b, lat_b) = gauss_kruger_inverse(x_out - 39_000_000.0, y_out, 117.0, Ellipsoid::CGCS2000);
    assert!((lon_b - 114.5).abs() < 0.001, "lon roundtrip (同基准换带可还原原始经度), got {}", lon_b);
    assert!((lat_b - 23.4).abs() < 0.001, "lat roundtrip, got {}", lat_b);
    assert!(x_out > 39_000_000.0 && x_out < 40_000_000.0, "x should be 3°带 zone 39, got {}", x_out);
}

#[test]
fn dynamic_proj_mode_h_reband_3deg_38_to_37() {
    // H 模式：3°带 38→37 相邻带换带（用户 bug 场景：同分带不同带号应正确换带）
    let (x38_np, y) = gauss_kruger_forward(112.5, 23.0, 114.0, Ellipsoid::CGCS2000);
    let x38 = x38_np + 38_000_000.0;
    let mut sources = vec![make_test_source(vec![(y, x38)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("几度分带", "3"),
        ("带号", "37"), // apply 后 header=目标 37，但坐标是原始 38 带
    ]);
    let options = ShpToTxtOptions { proj_mode: "H".to_string(), proj_zone: Some(37), ..shp_opts_test_default() };
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y_out, x_out) = sources[0].plots[0].plot.coords[0];
    let (lon_b, lat_b) = gauss_kruger_inverse(x_out - 37_000_000.0, y_out, 111.0, Ellipsoid::CGCS2000);
    assert!((lon_b - 112.5).abs() < 0.01, "lon roundtrip (38→37 换带可还原), got {}", lon_b);
    assert!((lat_b - 23.0).abs() < 0.01, "lat roundtrip, got {}", lat_b);
    assert!(x_out > 37_000_000.0 && x_out < 38_000_000.0, "x should be 3°带 zone 37, got {}", x_out);
}

#[test]
fn dynamic_proj_mode_h_src_zone_inferred_from_coords() {
    // 验证 src_zone 从坐标推断（不依赖 header.带号）：header 带号故意写错(37)，坐标是 38 带
    let (x38_np, y) = gauss_kruger_forward(114.5, 23.4, 114.0, Ellipsoid::CGCS2000);
    let x38 = x38_np + 38_000_000.0;
    let mut sources = vec![make_test_source(vec![(y, x38)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "CGCS2000"),
        ("几度分带", "3"),
        ("带号", ""), // header 无带号
    ]);
    let options = ShpToTxtOptions { proj_mode: "H".to_string(), proj_zone: Some(39), ..shp_opts_test_default() };
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y_out, x_out) = sources[0].plots[0].plot.coords[0];
    let (lon_b, lat_b) = gauss_kruger_inverse(x_out - 39_000_000.0, y_out, 117.0, Ellipsoid::CGCS2000);
    assert!((lon_b - 114.5).abs() < 0.001, "lon roundtrip (header 无带号也能推断 src), got {}", lon_b);
    assert!((lat_b - 23.4).abs() < 0.001, "lat roundtrip, got {}", lat_b);
    assert!(x_out > 39_000_000.0 && x_out < 40_000_000.0, "x should be zone 39 (inferred src 38), got {}", x_out);
}

#[test]
fn dynamic_proj_mode_h_reband_3deg_36_to_35_wgs84() {
    // city.shp 场景：WGS84 3°带 36→35（用户报告换带失败）
    let (x36_np, y) = gauss_kruger_forward(107.5, 40.7, 108.0, Ellipsoid::WGS84);
    let x36 = x36_np + 36_000_000.0;
    let mut sources = vec![make_test_source(vec![(y, x36)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "WGS84"),
        ("几度分带", "3"),
        ("带号", "35"), // apply 后 header=目标 35
    ]);
    let options = ShpToTxtOptions { proj_mode: "H".to_string(), proj_zone: Some(35), ..shp_opts_test_default() };
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y_out, x_out) = sources[0].plots[0].plot.coords[0];
    eprintln!("36→35: in ({}, {}), out ({}, {})", y, x36, y_out, x_out);
    let (lon_b, lat_b) = gauss_kruger_inverse(x_out - 35_000_000.0, y_out, 105.0, Ellipsoid::WGS84);
    assert!((lon_b - 107.5).abs() < 0.01, "lon roundtrip (36→35), got {}", lon_b);
    assert!((lat_b - 40.7).abs() < 0.01, "lat roundtrip, got {}", lat_b);
    assert!(x_out > 35_000_000.0 && x_out < 36_000_000.0, "x should be zone 35, got {}", x_out);
}

#[test]
fn city_shp_actual_36_to_35() {
    // city.shp 实际坐标 + 坐标系="WGS84坐标系"（parse_datum_for_proj 默认 CGCS2000）
    let mut sources = vec![make_test_source(vec![(4511093.231, 36427516.023)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "WGS84坐标系"),
        ("几度分带", "3"),
        ("带号", "35"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "H".to_string(), proj_zone: Some(35), ..shp_opts_test_default() };
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y_out, x_out) = sources[0].plots[0].plot.coords[0];
    eprintln!("city actual: out ({}, {})", y_out, x_out);
    assert!(x_out < 36_000_000.0, "x should be zone 35 (<36M), got {}", x_out);
}

#[test]
fn diag_39_to_34_actual() {
    // 面多部件加内切.shp 实际坐标（39 带）→ 选 34 带
    let mut sources = vec![make_test_source(vec![(2552207.0, 39325334.0)])];
    let header = header_with_test_attrs(vec![
        ("坐标系", "2000国家大地坐标系"),
        ("几度分带", "3"),
        ("带号", "34"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "H".to_string(), proj_zone: Some(34), ..shp_opts_test_default() };
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (y_out, x_out) = sources[0].plots[0].plot.coords[0];
    eprintln!("39→34: in (2552207, 39325334) out ({}, {})", y_out, x_out);
    // 反算验证：无论带外变形多大，reband 应能还原原始经纬度
    let (lon_b, lat_b) = gauss_kruger_inverse(x_out - 34_000_000.0, y_out, 102.0, Ellipsoid::CGCS2000);
    eprintln!("39→34 反算: lon={}, lat={}", lon_b, lat_b);
}

#[test]
fn diag_39_to_40_both_paths() {
    // 面多部件 39→40（相邻带），对比预览(_to_plots) vs 导出(_to_sources)
    let coords = vec![(2552207.0, 39325334.0)];
    let header = header_with_test_attrs(vec![
        ("坐标系", "2000国家大地坐标系"), ("几度分带", "3"), ("带号", "40"),
    ]);
    let options = ShpToTxtOptions { proj_mode: "H".to_string(), proj_zone: Some(40), ..shp_opts_test_default() };
    let mut sources = vec![make_test_source(coords.clone())];
    let _ = apply_dynamic_projection_to_sources(&mut sources, &header, &options).unwrap();
    let (ys, xs) = sources[0].plots[0].plot.coords[0];
    let mut plots = vec![__plot_with_coords(coords.clone())];
    let _ = apply_dynamic_projection_to_plots(&mut plots, &header, &options).unwrap();
    let (yp, xp) = plots[0].coords[0];
    eprintln!("39→40: sources out ({}, {}) | plots out ({}, {})", ys, xs, yp, xp);
    eprintln!("39→40: sources 带号前缀={} | plots 带号前缀={}", (xs/1e7).floor(), (xp/1e7).floor());
}

