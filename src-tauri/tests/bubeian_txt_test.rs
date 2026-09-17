// 部备案模板 TXT 导出测试
// 坐标行 6 列：点号,环号,Y,X,到下一点距离,界址点类型（埋桩）
// 距离规则：本点到下一点直线距离；末点连回首点——闭合点距离 0，开口环算到首点的闭合边长。

use std::collections::HashMap;
use std::path::PathBuf;

extern crate jisig_bpoint_converter_lib;

use jisig_bpoint_converter_lib::{convert, txt};
use jisig_bpoint_converter_lib::geometry::IndexedRing;
use jisig_bpoint_converter_lib::txt::{PlotData, PointColumn, PointLayout};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn test_shp_stem() -> PathBuf {
    repo_root().join("test_arcpy").join("std_shp").join("plot_000.shp")
}

fn column(kind: &str, source: &str, value: &str) -> PointColumn {
    PointColumn {
        kind: kind.to_string(),
        source: source.to_string(),
        value: value.to_string(),
        distance_unit: None,
        distance_decimals: None,
    }
}

fn distance_column(unit: &str, decimals: u32) -> PointColumn {
    let mut col = column("distance", "", "");
    col.distance_unit = Some(unit.to_string());
    col.distance_decimals = Some(decimals);
    col
}

fn layout(columns: &[PointColumn], unit: &str, decimals: u32) -> PointLayout {
    PointLayout {
        columns: columns.to_vec(),
        stake_field: String::new(),
        stake_default: String::new(),
        distance_unit: unit.to_string(),
        distance_decimals: decimals,
    }
}

fn bubeian_layout(unit: &str, decimals: u32) -> PointLayout {
    layout(
        &["point", "ring", "y", "x", "distance", "stake"]
            .iter()
            .map(|kind| column(kind, "", ""))
            .collect::<Vec<_>>(),
        unit,
        decimals,
    )
}

fn square_plot(stake: &str) -> PlotData {
    PlotData {
        point_count: 4,
        area: String::new(),
        fid: String::new(),
        name: String::new(),
        geom_type: "面".to_string(),
        tfh: String::new(),
        use_field: String::new(),
        dlbm: String::new(),
        coords: vec![],
        rings: vec![IndexedRing {
            part_index: 1,
            coords: vec![(10.0, 10.0), (10.0, 20.0), (20.0, 20.0), (10.0, 10.0)],
        }],
        fields: vec![],
        stake: stake.to_string(),
        custom_values: HashMap::new(),
    }
}

fn coord_lines(out: &str) -> Vec<String> {
    out.lines()
        .filter(|l| l.contains(',') && !l.starts_with('[') && !l.ends_with('@'))
        .map(|s| s.to_string())
        .collect()
}

fn attrs() -> Vec<convert::AttrRow> {
    vec![convert::AttrRow { k: "精度".into(), v: "0.001".into() }]
}

// ─── 1. 6 列输出：距离、闭合 0、埋桩列 ───

#[test]
fn test_bubeian_six_columns_closing_zero_stake() {
    let plot = square_plot("埋桩");
    let out = txt::generate_txt_ex("", &attrs(), &[plot], true, false, Some(&bubeian_layout("m", 3)));
    let lines = coord_lines(&out);

    assert_eq!(
        lines,
        vec![
            "J1,1,10.000,10.000,10.000,埋桩".to_string(),
            "J2,1,10.000,20.000,10.000,埋桩".to_string(),
            "J3,1,20.000,20.000,14.142,埋桩".to_string(),
            "J1,1,10.000,10.000,0.000,埋桩".to_string(),
        ],
        "6 列坐标行：闭合边 14.142（√200），闭合点距离 0:\n{}",
        out
    );
}

#[test]
fn test_bubeian_unclosed_ring_distance_to_first() {
    // 开口环：末点不与首点重合 → 末点距离 = 到首点的闭合边长（30-40-50 直角三角形）
    let plot = PlotData {
        point_count: 3,
        area: String::new(),
        fid: String::new(),
        name: String::new(),
        geom_type: "面".to_string(),
        tfh: String::new(),
        use_field: String::new(),
        dlbm: String::new(),
        coords: vec![(0.0, 0.0), (0.0, 30.0), (40.0, 30.0)],
        rings: vec![],
        fields: vec![],
        stake: "钢钉".to_string(),
        custom_values: HashMap::new(),
    };
    let out = txt::generate_txt_ex("", &attrs(), &[plot], true, false, Some(&bubeian_layout("m", 3)));
    let lines = coord_lines(&out);
    assert_eq!(
        lines,
        vec![
            "J1,1,0.000,0.000,30.000,钢钉".to_string(),
            "J2,1,0.000,30.000,40.000,钢钉".to_string(),
            "J3,1,40.000,30.000,50.000,钢钉".to_string(),
        ],
        "开口环末点距离应为到首点的闭合边长 50:\n{}",
        out
    );
}

#[test]
fn test_bubeian_unit_conversion_and_decimals() {
    let plot = square_plot("埋桩");
    // 千米：10 m = 0.010 km
    let out = txt::generate_txt_ex("", &attrs(), &[plot.clone()], true, false, Some(&bubeian_layout("km", 3)));
    assert!(out.contains("J1,1,10.000,10.000,0.010,埋桩"), "千米换算:\n{}", out);
    // 厘米：10 m = 1000 cm
    let out = txt::generate_txt_ex("", &attrs(), &[plot.clone()], true, false, Some(&bubeian_layout("cm", 3)));
    assert!(out.contains("J1,1,10.000,10.000,1000.000,埋桩"), "厘米换算:\n{}", out);
    // 0 位小数：闭合边 14.142 → 14
    let out = txt::generate_txt_ex("", &attrs(), &[plot], true, false, Some(&bubeian_layout("m", 0)));
    assert!(out.contains("J3,1,20.000,20.000,14,埋桩"), "0 位小数:\n{}", out);
}

#[test]
fn test_bubeian_standard_output_unchanged_without_spec() {
    let plot = square_plot("埋桩");
    let out = txt::generate_txt("", &attrs(), &[plot], true, false);
    assert!(
        out.contains("J1,1,10.000,10.000\n"),
        "不带 bubeian 时保持标准 4 列输出:\n{}",
        out
    );
    assert!(!out.contains(",埋桩"), "标准输出不应出现埋桩列:\n{}", out);
}

#[test]
fn test_custom_layout_order_field_and_fixed() {
    let mut plot = square_plot("埋桩");
    plot.custom_values.insert("DKMC".to_string(), "测试地块".to_string());
    let custom = PointLayout {
        columns: vec![
            column("x", "", ""),
            column("field", "DKMC", ""),
            column("fixed", "", "手动列"),
            column("point", "", ""),
        ],
        stake_field: String::new(),
        stake_default: String::new(),
        distance_unit: "m".to_string(),
        distance_decimals: 3,
    };
    let out = txt::generate_txt_ex("", &attrs(), &[plot], true, false, Some(&custom));
    let line = coord_lines(&out).first().expect("至少一行坐标").clone();
    assert_eq!(line, "10.000,测试地块,手动列,J1");
}

#[test]
fn test_custom_layout_missing_field_outputs_empty() {
    let plot = square_plot("埋桩");
    let custom = PointLayout {
        columns: vec![column("point", "", ""), column("field", "NOT_EXIST", "")],
        stake_field: String::new(),
        stake_default: String::new(),
        distance_unit: "m".to_string(),
        distance_decimals: 3,
    };
    let out = txt::generate_txt_ex("", &attrs(), &[plot], true, false, Some(&custom));
    assert!(out.contains("\nJ1,\n"), "缺失字段列应输出空值:\n{}", out);
}

#[test]
fn test_each_distance_column_has_own_unit_and_precision() {
    let plot = square_plot("埋桩");
    let custom = PointLayout {
        columns: vec![
            column("point", "", ""),
            column("y", "", ""),
            distance_column("km", 1),
            distance_column("cm", 0),
        ],
        stake_field: String::new(),
        stake_default: String::new(),
        distance_unit: "m".to_string(),
        distance_decimals: 3,
    };
    let out = txt::generate_txt_ex("", &attrs(), &[plot], true, false, Some(&custom));
    let lines = coord_lines(&out);
    assert_eq!(
        lines,
        vec![
            "J1,10.000,0.0,1000".to_string(),
            "J2,10.000,0.0,1000".to_string(),
            "J3,20.000,0.0,1414".to_string(),
            "J1,10.000,0.0,0".to_string(),
        ],
        "两个点距离列应分别使用自身单位/精度:\n{}",
        out
    );
}

#[test]
fn test_distance_is_per_ring_for_multipolygon_and_hole() {
    let plot = PlotData {
        point_count: 9,
        area: String::new(),
        fid: String::new(),
        name: String::new(),
        geom_type: "面".to_string(),
        tfh: String::new(),
        use_field: String::new(),
        dlbm: String::new(),
        coords: vec![],
        rings: vec![
            IndexedRing {
                part_index: 1,
                coords: vec![(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0), (0.0, 0.0)],
            },
            IndexedRing {
                part_index: 2,
                coords: vec![(0.0, 0.0), (0.0, 30.0), (40.0, 0.0), (0.0, 0.0)],
            },
        ],
        fields: vec![],
        stake: "埋桩".to_string(),
        custom_values: HashMap::new(),
    };
    let layout = PointLayout {
        columns: vec![column("point", "", ""), column("distance", "", "")],
        stake_field: String::new(),
        stake_default: String::new(),
        distance_unit: "m".to_string(),
        distance_decimals: 3,
    };
    let out = txt::generate_txt_ex("", &attrs(), &[plot], true, true, Some(&layout));
    let lines = coord_lines(&out);
    assert_eq!(
        lines,
        vec![
            "J1,10.000".to_string(),
            "J2,10.000".to_string(),
            "J3,10.000".to_string(),
            "J4,10.000".to_string(),
            "J5,0.000".to_string(),
            "J6,30.000".to_string(),
            "J7,50.000".to_string(),
            "J8,40.000".to_string(),
            "J9,0.000".to_string(),
        ],
        "多部件/内环应各自闭合，不跨环连边:\n{}",
        out
    );
}

// ─── 2. SHP 端到端：埋桩字段值优先 / 缺字段兜底 ───

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

fn field_mapping() -> convert::FieldMapping {
    convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "__area_ha__".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
        columns: Vec::new(),
    }
}

fn base_options() -> convert::ShpToTxtOptions {
    convert::ShpToTxtOptions {
        proj_mode: "keep".to_string(),
        proj_zone: None,
        ox: false,
        oj: true,
        on: false,
        oo: true,
        oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
        og: false,
        zone_type: 3,
        proj_no_prefix: false,
        plot_filter: None,
        point_layout: None,
    }
}

/// 逐行校验 6 列坐标行：列数、距离可解析、各环末行（闭合点）距离为 0
fn assert_bubeian_rows(content: &str, expected_stake: &str) {
    let mut ring_last: Vec<String> = Vec::new(); // 每环最后一行的距离列
    let mut current_ring: u32 = 0;
    let mut coord_rows = 0;
    for line in content.lines() {
        // 坐标行：非段落标题、非 k=v 属性行、非 ,@ 结尾元数据行
        if line.starts_with('[') || line.contains('=') || line.ends_with('@') || !line.contains(',') {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        assert_eq!(parts.len(), 6, "部备案坐标行应为 6 列: {}", line);
        let ring: u32 = parts[1].parse().expect("环号列");
        parts[4].parse::<f64>().expect("距离列应为数字");
        if ring != current_ring {
            if coord_rows > 0 {
                assert_eq!(ring_last.last().unwrap(), "0.000", "环 {} 的闭合点距离应为 0", current_ring);
            }
            current_ring = ring;
        }
        ring_last.push(parts[4].to_string());
        assert_eq!(parts[5], expected_stake, "埋桩列应为「{}」: {}", expected_stake, line);
        coord_rows += 1;
    }
    assert_eq!(ring_last.last().unwrap(), "0.000", "最后一环的闭合点距离应为 0");
    assert!(coord_rows > 0, "应至少有一个坐标行");
}

#[test]
fn test_shp_to_txt_bubeian_stake_from_field() {
    if !repo_root().join("test_arcpy").exists() {
        return;
    }
    let out_dir = tempfile::tempdir().expect("temp dir");
    let mut options = base_options();
    let mut layout = bubeian_layout("m", 3);
    layout.stake_field = "DKMC".into();
    layout.stake_default = "埋桩".into();
    options.point_layout = Some(layout);

    let result = convert::convert_shp_to_txt(
        &[test_shp_stem()],
        None,
        None,
        &make_header(),
        &field_mapping(),
        &options,
        out_dir.path(),
        None,
    )
    .expect("部备案转换失败");

    assert!(result.success);
    let content = std::fs::read_to_string(&result.output_files[0]).unwrap();
    assert!(content.contains("[属性描述]") && content.contains("[地块坐标]"));

    // 埋桩列 = DKMC 字段值（std_shp 的 DKMC 非空）
    let dkmc = content
        .lines()
        .find(|l| l.ends_with(",@"))
        .and_then(|l| l.split(',').nth(3))
        .expect("元数据行地块名列")
        .to_string();
    assert!(!dkmc.is_empty(), "DKMC 应非空");
    assert_bubeian_rows(&content, &dkmc);
}

#[test]
fn test_shp_to_txt_bubeian_stake_fallback_and_standard_prefix() {
    if !repo_root().join("test_arcpy").exists() {
        return;
    }
    let out_dir = tempfile::tempdir().expect("temp dir");
    // 字段不存在 → 全部用兜底值
    let mut options = base_options();
    let mut layout = bubeian_layout("m", 3);
    layout.stake_field = "NOT_EXIST".into();
    layout.stake_default = "钢钉".into();
    options.point_layout = Some(layout);

    let header = make_header();
    let fm = field_mapping();
    let result = convert::convert_shp_to_txt(
        &[test_shp_stem()],
        None,
        None,
        &header,
        &fm,
        &options,
        out_dir.path(),
        None,
    )
    .expect("部备案转换失败");
    let bubeian_txt = std::fs::read_to_string(&result.output_files[0]).unwrap();
    assert_bubeian_rows(&bubeian_txt, "钢钉");

    // 与标准 TXT 对比：头部/元数据行一致；坐标行 = 标准行 + 距离、埋桩两列
    let std_dir = tempfile::tempdir().expect("temp dir");
    let std_result = convert::convert_shp_to_txt(
        &[test_shp_stem()],
        None,
        None,
        &header,
        &fm,
        &base_options(),
        std_dir.path(),
        None,
    )
    .expect("标准转换失败");
    let std_txt = std::fs::read_to_string(&std_result.output_files[0]).unwrap();

    let std_lines: Vec<&str> = std_txt.lines().collect();
    let bb_lines: Vec<&str> = bubeian_txt.lines().collect();
    assert_eq!(std_lines.len(), bb_lines.len(), "两种输出行数应一致");
    for (s, b) in std_lines.iter().zip(bb_lines.iter()) {
        if s.starts_with('[') || s.ends_with('@') || s.contains('=') {
            assert_eq!(s, b, "非坐标行应完全一致:\n标准: {}\n部备案: {}", s, b);
        } else if s.contains(',') {
            assert!(
                b.starts_with(&format!("{},", s)),
                "部备案坐标行应为标准行追加两列:\n标准: {}\n部备案: {}",
                s,
                b
            );
        }
    }
}

#[test]
fn test_point_layout_applies_to_all_output_modes() {
    if !repo_root().join("test_arcpy").exists() {
        return;
    }
    let layout = PointLayout {
        columns: vec![column("point", "", ""), column("fixed", "", "LAYOUT_OK")],
        stake_field: String::new(),
        stake_default: String::new(),
        distance_unit: "m".to_string(),
        distance_decimals: 3,
    };

    for mode in ["one_to_one", "split_by_plot", "merge_all"] {
        let out_dir = tempfile::tempdir().expect("temp dir");
        let mut options = base_options();
        options.output_mode = mode.to_string();
        options.point_layout = Some(layout.clone());

        let result = convert::convert_shp_to_txt(
            &[test_shp_stem()],
            None,
            None,
            &make_header(),
            &field_mapping(),
            &options,
            out_dir.path(),
            None,
        )
        .unwrap_or_else(|e| panic!("{mode} 转换失败: {e}"));

        assert!(result.success, "{mode} 应成功");
        let content = std::fs::read_to_string(&result.output_files[0]).unwrap();
        assert!(
            content.lines().any(|line| line.starts_with("J1,") && line.ends_with(",LAYOUT_OK")),
            "{mode} 应使用自定义列布局:\n{content}"
        );
    }
}
