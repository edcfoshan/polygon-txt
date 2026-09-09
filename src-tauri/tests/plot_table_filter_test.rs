// 属性表/地图视图结构化数据（plot_table_geo）+ plot_filter 地块级筛选 集成测试
// 自建临时 SHP（无 DBF/PRJ），不依赖 test_arcpy 夹具

use std::path::PathBuf;

extern crate jisig_bpoint_converter_lib;

use jisig_bpoint_converter_lib::convert;
use shapefile::{Point, Polygon, PolygonRing, ShapeWriter};

fn write_polygon_shp(dir: &std::path::Path, name: &str, rings: &[Vec<(f64, f64)>]) -> PathBuf {
    let shp_path = dir.join(format!("{}.shp", name));
    let mut w = ShapeWriter::from_path(&shp_path).expect("创建 ShapeWriter");
    for ring in rings {
        let mut pts: Vec<Point> = ring.iter().map(|&(x, y)| Point::new(x, y)).collect();
        let first = pts[0];
        if let Some(last) = pts.last() {
            if (last.x, last.y) != (first.x, first.y) {
                pts.push(first);
            }
        }
        w.write_shape(&Polygon::with_rings(vec![PolygonRing::Outer(pts)]))
            .expect("写 Polygon");
    }
    drop(w);
    shp_path
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<(f64, f64)> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
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

fn opts(output_mode: &str, filter: Option<Vec<[usize; 2]>>) -> convert::ShpToTxtOptions {
    convert::ShpToTxtOptions {
        ox: false, oj: true, on: false, oo: false, oc: false,
        output_mode: output_mode.into(),
        filename_field: String::new(),
        og: false, zone_type: 3,
        proj_mode: "keep".into(),
        proj_zone: None,
        proj_no_prefix: false,
        plot_filter: filter,
    }
}

/// TXT 里每个地块恰有一条以 @ 结尾的元数据行
fn meta_line_count(txt: &str) -> usize {
    txt.lines().filter(|l| l.trim_end().ends_with('@')).count()
}

fn three_feature_shp(dir: &std::path::Path) -> PathBuf {
    write_polygon_shp(
        dir,
        "flt",
        &[square(0.0, 0.0, 10.0, 10.0), square(20.0, 0.0, 30.0, 10.0), square(40.0, 0.0, 50.0, 10.0)],
    )
}

// ─── plot_filter × 三种输出模式 ───

#[test]
fn plot_filter_merge_all() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = three_feature_shp(tmp.path());
    let out_dir = tempfile::tempdir().unwrap();
    let header = make_header();

    let result = convert::convert_shp_to_txt(
        &[shp.clone()], None, None, &header, &empty_mapping(),
        &opts("merge_all", Some(vec![[0, 0], [0, 2]])),
        out_dir.path(), None,
    )
    .expect("merge_all 筛选转换失败");
    assert!(result.success);
    let merged = std::fs::read_dir(out_dir.path()).unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|x| x == "txt").unwrap_or(false))
        .expect("应有输出 txt");
    let txt = std::fs::read_to_string(merged).unwrap();
    assert_eq!(meta_line_count(&txt), 2, "筛选后应只输出 2 个地块");

    // 不过滤 → 3 个
    let out_all = tempfile::tempdir().unwrap();
    convert::convert_shp_to_txt(
        &[shp], None, None, &header, &empty_mapping(), &opts("merge_all", None),
        out_all.path(), None,
    )
    .unwrap();
    let merged_all = std::fs::read_dir(out_all.path()).unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|x| x == "txt").unwrap_or(false))
        .unwrap();
    assert_eq!(meta_line_count(&std::fs::read_to_string(merged_all).unwrap()), 3);
}

#[test]
fn plot_filter_one_to_one() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = three_feature_shp(tmp.path());
    let out_dir = tempfile::tempdir().unwrap();

    let result = convert::convert_shp_to_txt(
        &[shp], None, None, &make_header(), &empty_mapping(),
        &opts("one_to_one", Some(vec![[0, 1]])),
        out_dir.path(), None,
    )
    .expect("one_to_one 筛选转换失败");
    assert_eq!(result.output_files.len(), 1);
    let txt = std::fs::read_to_string(&result.output_files[0]).unwrap();
    assert_eq!(meta_line_count(&txt), 1, "一对一模式筛选后文件内应只有 1 个地块");
}

#[test]
fn plot_filter_split_by_plot() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = three_feature_shp(tmp.path());
    let out_dir = tempfile::tempdir().unwrap();

    let result = convert::convert_shp_to_txt(
        &[shp], None, None, &make_header(), &empty_mapping(),
        &opts("split_by_plot", Some(vec![[0, 0], [0, 2]])),
        out_dir.path(), None,
    )
    .expect("split_by_plot 筛选转换失败");
    assert!(result.success);
    let subdir = out_dir.path().join("flt");
    let txts: Vec<_> = std::fs::read_dir(&subdir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "txt").unwrap_or(false))
        .collect();
    assert_eq!(txts.len(), 2, "拆分模式筛选后应只拆出 2 个 txt");
}

#[test]
fn plot_filter_empty_result_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = three_feature_shp(tmp.path());
    let out_dir = tempfile::tempdir().unwrap();

    let err = convert::convert_shp_to_txt(
        &[shp], None, None, &make_header(), &empty_mapping(),
        &opts("merge_all", Some(vec![[0, 99]])),
        out_dir.path(), None,
    )
    .expect_err("筛 0 条应报错");
    assert!(err.contains("筛选结果为空"), "错误文案不符: {}", err);
}

#[test]
fn plot_filter_applies_to_preview() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = three_feature_shp(tmp.path());
    let header = make_header();

    let all = convert::shp_to_txt_preview(
        &[shp.clone()], None, None, &header, &empty_mapping(), &opts("merge_all", None), None,
    )
    .unwrap();
    assert_eq!(meta_line_count(&all), 3, "无筛选预览应有 3 个地块");

    let filtered = convert::shp_to_txt_preview(
        &[shp], None, None, &header, &empty_mapping(),
        &opts("merge_all", Some(vec![[0, 1]])), None,
    )
    .unwrap();
    assert_eq!(meta_line_count(&filtered), 1, "预览应与导出同一筛选口径");
}

// ─── plot_table_geo：结构化地块 + WGS84 环坐标 ───

fn table_geo(shps: &[PathBuf]) -> convert::PlotTableGeo {
    convert::plot_table_geo(
        shps, None, None, &make_header(), &empty_mapping(),
        &opts("merge_all", None), None,
    )
    .expect("plot_table_geo 失败")
}

/// 3°带 38 带号带前缀源（CM 114°E）→ lon≈114
#[test]
fn table_geo_projected_zone38_prefix() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = write_polygon_shp(
        tmp.path(), "z38",
        &[square(38_495_000.0, 3_795_000.0, 38_505_000.0, 3_805_000.0)],
    );
    let geo = table_geo(&[shp.clone()]);

    assert_eq!(geo.sources.len(), 1);
    assert!(!geo.sources[0].geodetic, "38 带前缀源应为投影坐标");
    assert!(!geo.sources[0].degraded, "有带号前缀不应降级");
    assert_eq!(geo.plots.len(), 1);
    assert_eq!(geo.plots[0].si, 0);
    assert_eq!(geo.plots[0].pi, 0);
    let ring = &geo.plots[0].rings[0];
    assert!(ring.len() >= 3, "外环至少 3 点");
    let (lon, lat) = (ring[0][0], ring[0][1]);
    assert!((113.9..=114.1).contains(&lon), "3°带 38 带号 CM=114，实际 lon={}", lon);
    assert!((34.2..=34.4).contains(&lat), "北坐标 380km≈纬度 34.3，实际 lat={}", lat);
    // 界址点标签：与 TXT 输出同口径（oo=false 默认 → 闭合点被剥离，4 点 J1..J4）
    let pts = &geo.plots[0].points;
    assert_eq!(pts.len(), ring.len(), "点数应与环坐标数一致");
    assert_eq!(pts[0].label, "J1");
    assert_eq!(pts.last().unwrap().label, "J4", "oo=false 无闭合点");
    assert!((pts[0].xy[0] - ring[0][0]).abs() < 1e-6, "点坐标应与环坐标一致");
    assert!(geo.source_aliases.is_empty(), "SHP 源无别名");

    // oo=true：闭合点保留 → 末点回环首号 J1
    let geo_oo = convert::plot_table_geo(
        std::slice::from_ref(&shp), None, None, &make_header(), &empty_mapping(),
        &convert::ShpToTxtOptions { oo: true, ..opts("merge_all", None) }, None,
    )
    .unwrap();
    let pts_oo = &geo_oo.plots[0].points;
    assert_eq!(pts_oo.len(), 5, "oo=true 应含闭合点");
    assert_eq!(pts_oo.last().unwrap().label, "J1", "闭合点回环首号");
}

/// 两源不同带号前缀（38/39）→ 各自落对应 3° 带经度区间（逐源推带号）
#[test]
fn table_geo_two_sources_different_zones() {
    let tmp = tempfile::tempdir().unwrap();
    let a = write_polygon_shp(
        tmp.path(), "z38",
        &[square(38_495_000.0, 3_795_000.0, 38_505_000.0, 3_805_000.0)],
    );
    let b = write_polygon_shp(
        tmp.path(), "z39",
        &[square(39_495_000.0, 3_795_000.0, 39_505_000.0, 3_805_000.0)],
    );
    let geo = table_geo(&[a, b]);

    assert_eq!(geo.sources.len(), 2);
    assert_eq!(geo.plots.len(), 2);
    let p38 = geo.plots.iter().find(|p| p.si == 0).unwrap();
    let p39 = geo.plots.iter().find(|p| p.si == 1).unwrap();
    let lon38 = p38.rings[0][0][0];
    let lon39 = p39.rings[0][0][0];
    assert!((113.9..=114.1).contains(&lon38), "源 0 应在 38 带区间，lon={}", lon38);
    assert!((116.9..=117.1).contains(&lon39), "源 1 应在 39 带区间，lon={}", lon39);
}

/// 大地坐标（度）源 → geodetic=true，坐标直用
#[test]
fn table_geo_geodetic_passthrough() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = write_polygon_shp(
        tmp.path(), "geo",
        &[square(114.0, 30.5, 114.01, 30.51)],
    );
    let geo = table_geo(&[shp]);

    assert!(geo.sources[0].geodetic);
    assert!(!geo.sources[0].degraded);
    let ring = &geo.plots[0].rings[0];
    assert!((113.99..=114.01).contains(&ring[0][0]), "lon 直用: {}", ring[0][0]);
    assert!((30.49..=30.51).contains(&ring[0][1]), "lat 直用: {}", ring[0][1]);
}

/// 无带号前缀且无 PRJ → degraded=true，plots 仍回传（rings 空，表格可用）
#[test]
fn table_geo_degraded_without_zone() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = write_polygon_shp(
        tmp.path(), "nopfx",
        &[square(500_000.0, 3_000_000.0, 510_000.0, 3_010_000.0)],
    );
    let geo = table_geo(&[shp]);

    assert!(geo.sources[0].degraded, "无前缀无 PRJ 应降级");
    assert_eq!(geo.plots.len(), 1, "降级源仍应回传地块行");
    assert!(geo.plots[0].rings.is_empty(), "降级源 rings 应为空");
}

/// 属性表 = 源字段视图：无 DBF 的 SHP attrs 为空（仅源/序号列）
#[test]
fn table_geo_source_attrs_without_dbf() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = three_feature_shp(tmp.path());
    let geo = table_geo(&[shp]);
    assert!(geo.plots[0].attrs.is_empty(), "无 DBF 时无源字段");
    assert!(geo.source_aliases.is_empty());
}

const WKT_ZONE37: &str = r#"PROJCS["CGCS2000_3_Degree_GK_Zone_37",GEOGCS["GCS_China_Geodetic_Coordinate_System_2000",DATUM["D_China_2000",SPHEROID["CGCS2000",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Gauss_Kruger"],PARAMETER["False_Easting",37500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",111.0],PARAMETER["Scale_Factor",1.0],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

const WKT_ZONE20_6DEG: &str = r#"PROJCS["CGCS2000_6_Degree_GK_Zone_20",GEOGCS["GCS_China_Geodetic_Coordinate_System_2000",DATUM["D_China_2000",SPHEROID["CGCS2000",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Gauss_Kruger"],PARAMETER["False_Easting",20500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",117.0],PARAMETER["Scale_Factor",1.0],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

/// 37 带东侧点东坐标溢出到 38M 块（真实揭阳数据形态：38_057_383 = 37_500_000 + 557_383）。
/// 带号推定必须以 PRJ 的 z=37 为权威（floor(x/1e6)=38 会误判），反算后应落在揭阳（~116.5°E）
#[test]
fn table_geo_zone37_spillover_easting() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = write_polygon_shp(
        tmp.path(), "z37spill",
        &[square(38_057_383.0, 2_612_015.0, 38_058_383.0, 2_613_015.0)],
    );
    std::fs::write(tmp.path().join("z37spill.prj"), WKT_ZONE37).unwrap();
    let geo = table_geo(&[shp]);

    assert!(!geo.sources[0].degraded, "有 PRJ 不应降级");
    let (lon, lat) = (geo.plots[0].rings[0][0][0], geo.plots[0].rings[0][0][1]);
    assert!((116.2..=116.8).contains(&lon), "37 带 CM111 东侧 557km ≈ 116.5°E（揭阳），实际 lon={}", lon);
    assert!((23.3..=23.9).contains(&lat), "北坐标 2612km ≈ 23.6°N，实际 lat={}", lat);
}

/// 6° 带 20 带（CM 117°E）：前缀块 20M 与 round(cm/3)=39 的 3° 带块不同，重贴后应正确反算
#[test]
fn table_geo_band6_zone20() {
    let tmp = tempfile::tempdir().unwrap();
    let shp = write_polygon_shp(
        tmp.path(), "z20b6",
        &[square(20_500_000.0, 2_612_015.0, 20_501_000.0, 2_613_015.0)],
    );
    std::fs::write(tmp.path().join("z20b6.prj"), WKT_ZONE20_6DEG).unwrap();
    let geo = table_geo(&[shp]);

    assert!(!geo.sources[0].degraded);
    let (lon, lat) = (geo.plots[0].rings[0][0][0], geo.plots[0].rings[0][0][1]);
    assert!((116.8..=117.2).contains(&lon), "6°带 20 带 CM117 中央线上 ≈ 117°E，实际 lon={}", lon);
    assert!((23.3..=23.9).contains(&lat), "实际 lat={}", lat);
}


// ─── GDB 快速目录读取 + 图层过滤 ───

fn gdb_test_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("test_arcpy")
        .join("test.gdb")
}

/// read_gdb_catalog：层数/要素数/范围与全量读取一致（毫秒级，不解码要素）
#[test]
fn gdb_catalog_matches_full_read() {
    let dir = gdb_test_dir();
    if !dir.exists() {
        println!("test.gdb 不存在，跳过");
        return;
    }
    let catalog = jisig_bpoint_converter_lib::gdb::read_gdb_catalog(&dir).expect("catalog 读取失败");
    let full = jisig_bpoint_converter_lib::gdb::read_gdb(&dir).expect("全量读取失败");
    assert_eq!(catalog.len(), full.layers.len(), "层数应一致");
    for (c, f) in catalog.iter().zip(full.layers.iter()) {
        assert_eq!(c.name, f.name);
        assert_eq!(c.num_features as usize, f.num_features, "层 {} 要素数应一致", c.name);
        assert!(!c.field_names.is_empty(), "层 {} 应有字段", c.name);
    }
}

/// read_gdb_filtered：单层过滤后索引对齐，未选层 features 为空
#[test]
fn gdb_filtered_read_aligns_indices() {
    let dir = gdb_test_dir();
    if !dir.exists() {
        println!("test.gdb 不存在，跳过");
        return;
    }
    let full = jisig_bpoint_converter_lib::gdb::read_gdb(&dir).expect("全量读取失败");
    if full.layers.len() < 2 {
        println!("test.gdb 少于 2 层，过滤用例退化为单层校验");
    }
    // 只取第一层
    let first = full.layers[0].name.clone();
    let filtered = jisig_bpoint_converter_lib::gdb::read_gdb_filtered(
        &dir,
        Some(&[first.clone()]),
    )
    .expect("过滤读取失败");
    assert_eq!(filtered.layers.len(), full.layers.len(), "层索引必须对齐");
    for (i, layer) in filtered.layers.iter().enumerate() {
        if layer.name == first {
            assert_eq!(layer.num_features, full.layers[i].num_features, "选中层要素数一致");
            assert_eq!(filtered.all_features[i].len(), full.all_features[i].len(), "选中层要素已解码");
        } else {
            assert!(filtered.all_features[i].is_empty(), "未选层应跳过要素解码");
        }
    }
}

/// GDB 属性与字段表对齐：OBJECTID 存在且为整数（字段值错位时它会变成其他字段的小数）
#[test]
fn gdb_attrs_align_with_field_names() {
    let dir = gdb_test_dir();
    if !dir.exists() {
        println!("test.gdb 不存在，跳过");
        return;
    }
    let info = jisig_bpoint_converter_lib::gdb::read_gdb(&dir).expect("读取失败");
    for (li, feats) in info.all_features.iter().enumerate() {
        let names = &info.layers[li].field_names;
        for f in feats.iter().take(5) {
            for n in names {
                assert!(
                    f.attributes.contains_key(n),
                    "层 {} 属性缺失字段 {}（字段值与字段名错位）",
                    info.layers[li].name,
                    n
                );
            }
            if let Some(v) = f.attributes.get("OBJECTID") {
                assert!(
                    v.trim().parse::<i64>().is_ok(),
                    "OBJECTID 应为整数，实际 {}（疑似字段值错位）",
                    v
                );
            }
        }
    }
}
