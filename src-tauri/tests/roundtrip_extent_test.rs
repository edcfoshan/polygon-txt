// 37 带东侧溢出数据（x 以 38 开头）× 位置保持类转换：
// 转出 TXT → 回转 SHP（自动写 PRJ）→ WGS84 范围必须与原始一致（用户要求的核心测试）

use std::path::PathBuf;

extern crate jisig_bpoint_converter_lib;

use jisig_bpoint_converter_lib::convert;
use shapefile::{Point, Polygon, PolygonRing, ShapeWriter};

const WKT_ZONE37: &str = r#"PROJCS["CGCS2000_3_Degree_GK_Zone_37",GEOGCS["GCS_China_Geodetic_Coordinate_System_2000",DATUM["D_China_2000",SPHEROID["CGCS2000",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Gauss_Kruger"],PARAMETER["False_Easting",37500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",111.0],PARAMETER["Scale_Factor",1.0],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

fn write_spill_shp(dir: &std::path::Path) -> PathBuf {
    let shp_path = dir.join("z37spill.shp");
    let pts: Vec<Point> = [
        (38_057_383.0, 2_612_015.0),
        (38_058_383.0, 2_612_015.0),
        (38_058_383.0, 2_613_015.0),
        (38_057_383.0, 2_613_015.0),
        (38_057_383.0, 2_612_015.0),
    ]
    .iter()
    .map(|&(x, y)| Point::new(x, y))
    .collect();
    let mut w = ShapeWriter::from_path(&shp_path).expect("创建 ShapeWriter");
    w.write_shape(&Polygon::with_rings(vec![PolygonRing::Outer(pts)]))
        .expect("写 Polygon");
    drop(w);
    std::fs::write(dir.join("z37spill.prj"), WKT_ZONE37).unwrap();
    shp_path
}

fn make_header() -> convert::HeaderConfig {
    convert::HeaderConfig {
        attrs: vec![
            convert::AttrRow { k: "坐标系".into(), v: "2000国家大地坐标系".into() },
            convert::AttrRow { k: "几度分带".into(), v: "3".into() },
            convert::AttrRow { k: "投影类型".into(), v: "高斯克吕格".into() },
            convert::AttrRow { k: "计量单位".into(), v: "米".into() },
            convert::AttrRow { k: "带号".into(), v: "37".into() },
            convert::AttrRow { k: "精度".into(), v: "0.001".into() },
            convert::AttrRow { k: "转换参数".into(), v: ",,,,,,".into() },
        ],
        project_info: String::new(),
    }
}

fn empty_mapping() -> convert::FieldMapping {
    convert::FieldMapping {
        name: String::new(),
        id: String::new(),
        area: String::new(),
        use_field: String::new(),
        tfh: String::new(),
        dlbm: String::new(),
        columns: Vec::new(),
    }
}

fn opts_with(proj_mode: &str, proj_zone: Option<u32>, no_prefix: bool) -> convert::ShpToTxtOptions {
    convert::ShpToTxtOptions {
        ox: false, oj: true, on: false, oo: false, oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
        og: false, zone_type: 3,
        proj_mode: proj_mode.into(),
        proj_zone,
        proj_no_prefix: no_prefix,
        plot_filter: None,
    }
}

fn txt_to_shp_opts(out_dir: &std::path::Path) -> convert::TxtToShpOptions {
    convert::TxtToShpOptions {
        output_shp: true,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
        output_dir: out_dir.to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    }
}

/// WGS84 范围 [minLon, minLat, maxLon, maxLat]
fn wgs84_extent(geo: &convert::PlotTableGeo) -> Option<[f64; 4]> {
    let mut it = geo
        .plots
        .iter()
        .flat_map(|p| p.rings.iter().flat_map(|r| r.iter()));
    let first = it.next()?;
    let mut min_lon = first[0];
    let mut min_lat = first[1];
    let mut max_lon = first[0];
    let mut max_lat = first[1];
    for pt in it {
        min_lon = min_lon.min(pt[0]);
        min_lat = min_lat.min(pt[1]);
        max_lon = max_lon.max(pt[0]);
        max_lat = max_lat.max(pt[1]);
    }
    Some([min_lon, min_lat, max_lon, max_lat])
}

/// 原始 SHP / 回转 SHP → WGS84 范围
fn geo_extent(shp: &PathBuf) -> [f64; 4] {
    let header = make_header();
    let geo = convert::plot_table_geo(
        std::slice::from_ref(shp),
        None,
        None,
        &header,
        &empty_mapping(),
        &opts_with("keep", None, false),
        None,
    )
    .expect("plot_table_geo 失败");
    assert!(!geo.sources[0].degraded, "不应降级");
    wgs84_extent(&geo).expect("应有范围")
}

/// 完整往返：SHP → TXT（按 opts 转换）→ SHP → WGS84 范围
fn roundtrip(shp: &PathBuf, opts: &convert::ShpToTxtOptions) -> [f64; 4] {
    let out_txt = tempfile::tempdir().unwrap();
    convert::convert_shp_to_txt(
        &[shp.clone()], None, None, &make_header(), &empty_mapping(),
        opts, out_txt.path(), None,
    )
    .expect("转出 TXT 失败");
    let txt_path = std::fs::read_dir(out_txt.path()).unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|x| x == "txt").unwrap_or(false))
        .expect("应有输出 txt");

    let out_shp_dir = tempfile::tempdir().unwrap();
    convert::convert_txt_to_shp(
        &[txt_path],
        &txt_to_shp_opts(out_shp_dir.path()),
        &make_header(),
    )
    .expect("回转 SHP 失败");
    let shp_path = std::fs::read_dir(out_shp_dir.path()).unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|x| x == "shp").unwrap_or(false))
        .expect("应有回转 shp");
    geo_extent(&shp_path)
}

fn assert_extent_close(a: &[f64; 4], b: &[f64; 4], label: &str) {
    for i in 0..4 {
        assert!(
            (a[i] - b[i]).abs() < 5e-5,
            "{}：WGS84 范围第 {} 项不一致 {} vs {}（约 {}m）",
            label, i, a[i], b[i], (a[i] - b[i]).abs() * 100_000.0
        );
    }
}

const CASES: &[(&str, &str, Option<u32>, bool)] = &[
    ("keep 不动", "keep", None, false),
    ("keep+前缀开(37)", "keep", Some(37), false),
    ("keep+前缀剥(37)", "keep", Some(37), true),
    ("F 换带 37→6°19", "F", None, false),
    ("H 换带 37→38", "H", Some(38), false),
];

/// 核心测试：每个位置保持类转换，TXT 回转 SHP 后 WGS84 范围与原始一致
#[test]
fn spill_data_roundtrip_extent_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = write_spill_shp(tmp.path());
    let original = geo_extent(&shp);

    for (label, mode, zone, no_prefix) in CASES {
        let opts = opts_with(mode, *zone, *no_prefix);
        let extent = roundtrip(&shp, &opts);
        assert_extent_close(&original, &extent, label);
    }
}

/// 前缀开关各形态的数字断言（换号 37→38：自然值 1_057_383 贴 38 带块 = 39_057_383）
#[test]
fn spill_data_prefix_forms() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = write_spill_shp(tmp.path());

    let first_x = |opts: &convert::ShpToTxtOptions| -> f64 {
        let out = tempfile::tempdir().unwrap();
        let result = convert::convert_shp_to_txt(
            &[shp.clone()], None, None, &make_header(), &empty_mapping(),
            opts, out.path(), None,
        )
        .expect("转换失败");
        let txt = std::fs::read_to_string(&result.output_files[0]).unwrap();
        // [地块坐标] 后第一条坐标行第 4 列 = x（东坐标）
        let coord_line = txt
            .lines()
            .skip_while(|l| !l.starts_with("J1,"))
            .next()
            .expect("应有坐标行");
        coord_line.split(',').nth(3).unwrap().parse::<f64>().unwrap()
    };

    let x_in = 38_057_383.0;
    let x_keep = first_x(&opts_with("keep", None, false));
    assert!((x_keep - x_in).abs() < 0.0011, "keep 原样: {}", x_keep);

    let x_on = first_x(&opts_with("keep", Some(37), false));
    assert!((x_on - x_in).abs() < 0.0011, "前缀开(37) 剥/加闭环: {}", x_on);

    let x_strip = first_x(&opts_with("keep", Some(37), true));
    assert!((x_strip - 1_057_383.0).abs() < 0.0011, "前缀剥: {}", x_strip);

    let x_rezone = first_x(&opts_with("keep", Some(38), false));
    assert!(
        (x_rezone - (1_057_383.0 + 38_000_000.0)).abs() < 0.0011,
        "换号 38: 自然值 1_057_383 贴 38 带块 = 39_057_383，实际 {}",
        x_rezone
    );

    let x_f = first_x(&opts_with("F", None, false));
    // F: 37 带 → 6° 带 19（CM111 不变），点在 CM 东侧 557km → 自然东坐标 1_057_383，
    // 贴 19 带块 = 20_057_383（19 带自身的东侧溢出现象，正常）
    assert!(
        (x_f - 20_057_383.0).abs() < 1_000.0,
        "F 换带 37→6°19 应落 19 带块东侧形态 20_057_383，实际 {}",
        x_f
    );
}
