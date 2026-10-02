use jisig_bpoint_converter_lib::{
    convert,
    geometry::{IndexedRing, PolygonPart, SurfaceGeometry},
    shp, txt,
};
use shapefile::{Point, Polygon, PolygonRing, ShapeWriter};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

fn mapping(source: &str, decimals: Option<u32>) -> convert::FieldMapping {
    serde_json::from_value(serde_json::json!({
        "name":"", "id":"", "area":source, "area_decimals":decimals,
        "use_field":"", "tfh":"", "dlbm":""
    }))
    .unwrap()
}

fn options(mode: &str, encoding: &str) -> convert::ShpToTxtOptions {
    serde_json::from_value(serde_json::json!({
        "ox":false, "oj":true, "on":false, "oo":true,
        "output_mode":mode, "txt_encoding":encoding
    }))
    .unwrap()
}

fn header() -> convert::HeaderConfig {
    convert::HeaderConfig {
        project_info: "地块面积测试".into(),
        attrs: vec![
            convert::AttrRow {
                k: "坐标系".into(),
                v: "2000国家大地坐标系".into(),
            },
            convert::AttrRow {
                k: "精度".into(),
                v: "0.001".into(),
            },
        ],
    }
}

fn ring(x: f64, y: f64, width: f64, height: f64) -> Vec<Point> {
    vec![
        Point::new(x, y),
        Point::new(x + width, y),
        Point::new(x + width, y + height),
        Point::new(x, y + height),
        Point::new(x, y),
    ]
}

fn write_shp(dir: &Path, stem: &str, count: usize) -> PathBuf {
    let path = dir.join(format!("{}.shp", stem));
    let mut writer = ShapeWriter::from_path(&path).unwrap();
    for _ in 0..count {
        // 外环10000平方米，孔500平方米，净面积9500平方米。
        writer
            .write_shape(&Polygon::with_rings(vec![
                PolygonRing::Outer(ring(500000.0, 3000000.0, 100.0, 100.0)),
                PolygonRing::Inner(ring(500010.0, 3000010.0, 20.0, 25.0)),
            ]))
            .unwrap();
    }
    drop(writer);
    path
}

fn export(
    paths: &[PathBuf],
    map: &convert::FieldMapping,
    opt: &convert::ShpToTxtOptions,
    hdr: &convert::HeaderConfig,
    dir: &Path,
) -> Result<convert::ConvertResult, String> {
    convert::convert_shp_to_txt(paths, None, None, hdr, map, opt, dir, None)
}

fn metadata(text: &str) -> Vec<&str> {
    text.lines()
        .find(|l| l.ends_with(",@"))
        .unwrap()
        .split(',')
        .collect()
}

#[test]
fn area_units_and_every_precision_in_simple_mode() {
    let input = tempfile::tempdir().unwrap();
    let shp = write_shp(input.path(), "area", 1);
    let expectations = [
        (
            "__area_sqm__",
            [
                "9500",
                "9500.0",
                "9500.00",
                "9500.000",
                "9500.0000",
                "9500.00000",
                "9500.000000",
            ],
        ),
        (
            "__area_mu__",
            [
                "14",
                "14.3",
                "14.25",
                "14.250",
                "14.2500",
                "14.25000",
                "14.250000",
            ],
        ),
        (
            "__area_ha__",
            ["1", "1.0", "0.95", "0.950", "0.9500", "0.95000", "0.950000"],
        ),
        (
            "__area_km2__",
            ["0", "0.0", "0.01", "0.010", "0.0095", "0.00950", "0.009500"],
        ),
    ];
    for (unit, values) in expectations {
        for (decimals, expected) in values.iter().enumerate() {
            let out = tempfile::tempdir().unwrap();
            let result = export(
                &[shp.clone()],
                &mapping(unit, Some(decimals as u32)),
                &options("one_to_one", "utf8"),
                &header(),
                out.path(),
            )
            .unwrap();
            let text = std::fs::read_to_string(&result.output_files[0]).unwrap();
            assert_eq!(metadata(&text)[1], *expected, "{} / {}位", unit, decimals);
        }
    }
}

#[test]
fn advanced_area_columns_have_independent_units_and_precision() {
    let input = tempfile::tempdir().unwrap();
    let shp = write_shp(input.path(), "area", 1);
    let mut map = mapping("__area_ha__", Some(4));
    map.columns = serde_json::from_value(serde_json::json!([
        {"name":"平方米","source":"__area_sqm__","area_decimals":0},
        {"name":"亩","source":"__area_mu__","area_decimals":2},
        {"name":"平方千米","source":"__area_km2__","area_decimals":6}
    ]))
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    let result = export(
        &[shp],
        &map,
        &options("merge_all", "utf8"),
        &header(),
        out.path(),
    )
    .unwrap();
    let text = std::fs::read_to_string(&result.output_files[0]).unwrap();
    assert!(text.contains("9500,14.25,0.009500,@"));
}

fn point_layout(continuous: bool) -> txt::PointLayout {
    serde_json::from_value(serde_json::json!({
        "columns":[{"kind":"y"},{"kind":"sequence"},{"kind":"point"},{"kind":"ring"},{"kind":"x"}],
        "distance_unit":"m", "distance_decimals":3, "sequence_continuous":continuous
    }))
    .unwrap()
}

fn plot(two_rings: bool) -> txt::PlotData {
    let ring = IndexedRing {
        part_index: 1,
        coords: vec![(0.0, 0.0), (0.0, 10.0), (10.0, 0.0), (0.0, 0.0)],
    };
    let mut rings = vec![ring.clone()];
    if two_rings {
        rings.push(IndexedRing {
            part_index: 2,
            ..ring
        });
    }
    txt::PlotData {
        point_count: 0,
        area: String::new(),
        fid: String::new(),
        name: String::new(),
        geom_type: "面".into(),
        tfh: String::new(),
        use_field: String::new(),
        dlbm: String::new(),
        coords: vec![],
        rings,
        fields: vec![],
        stake: String::new(),
        custom_values: HashMap::new(),
    }
}

fn coord_sequences(text: &str) -> Vec<usize> {
    text.lines()
        .filter(|l| l.contains(',') && !l.ends_with('@') && !l.contains('='))
        .map(|l| l.split(',').nth(1).unwrap().parse().unwrap())
        .collect()
}

#[test]
fn sequence_spans_rings_and_optionally_spans_plots_including_closing_rows() {
    let plots = vec![plot(true), plot(false)];
    let continuous = point_layout(true);
    let restarted = point_layout(false);
    let text = txt::generate_txt_ex("", &[], &plots, true, false, Some(&continuous));
    assert_eq!(coord_sequences(&text), (1..=12).collect::<Vec<_>>());
    // 闭合行保留原点号J1，但独立序号占一号。
    assert!(text.contains("0.000,4,J1,1,0.000"));
    let text = txt::generate_txt_ex("", &[], &plots, true, false, Some(&restarted));
    assert_eq!(
        coord_sequences(&text),
        (1..=8).chain(1..=4).collect::<Vec<_>>()
    );
    let text = txt::generate_txt_ex("", &[], &[plot(false)], true, false, Some(&continuous));
    assert_eq!(coord_sequences(&text), vec![1, 2, 3, 4]);
}

#[test]
fn all_three_modes_write_correct_encoding_and_restart_sequence_per_file() {
    let input = tempfile::tempdir().unwrap();
    let paths = vec![
        write_shp(input.path(), "a", 2),
        write_shp(input.path(), "b", 1),
    ];
    for mode in ["one_to_one", "split_by_plot", "merge_all"] {
        for encoding in ["utf8", "gbk"] {
            let out = tempfile::tempdir().unwrap();
            let mut opt = options(mode, encoding);
            opt.point_layout = Some(point_layout(true));
            let result = export(
                &paths,
                &mapping("__area_ha__", Some(4)),
                &opt,
                &header(),
                out.path(),
            )
            .unwrap();
            assert_eq!(
                result.output_files.len(),
                match mode {
                    "one_to_one" => 2,
                    "split_by_plot" => 3,
                    _ => 1,
                }
            );
            for file in &result.output_files {
                let bytes = std::fs::read(file).unwrap();
                assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
                let text = txt::read_text_file(file).unwrap();
                assert!(text.contains("地块面积测试"));
                let expected = if encoding == "gbk" {
                    encoding_rs::GBK.encode(&text).0.into_owned()
                } else {
                    text.as_bytes().to_vec()
                };
                assert_eq!(bytes, expected);
                if encoding == "gbk" {
                    assert!(std::str::from_utf8(&bytes).is_err());
                }
                let seq = coord_sequences(&text);
                assert_eq!(seq, (1..=seq.len()).collect::<Vec<_>>());
            }
        }
    }
}

#[test]
fn gbk_and_utf8_exports_carry_identical_content() {
    let input = tempfile::tempdir().unwrap();
    let paths = vec![write_shp(input.path(), "a", 2)];
    let out_utf8 = tempfile::tempdir().unwrap();
    let out_gbk = tempfile::tempdir().unwrap();
    let r8 = export(
        &paths,
        &mapping("__area_ha__", Some(4)),
        &options("one_to_one", "utf8"),
        &header(),
        out_utf8.path(),
    )
    .unwrap();
    let rg = export(
        &paths,
        &mapping("__area_ha__", Some(4)),
        &options("one_to_one", "gbk"),
        &header(),
        out_gbk.path(),
    )
    .unwrap();
    assert_eq!(r8.output_files.len(), rg.output_files.len());
    for (f8, fg) in r8.output_files.iter().zip(rg.output_files.iter()) {
        assert_eq!(Path::new(f8).file_name(), Path::new(fg).file_name());
        let b8 = std::fs::read(f8).unwrap();
        let bg = std::fs::read(fg).unwrap();
        assert!(!b8.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(!bg.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert_ne!(b8, bg, "GBK 导出的字节不应与 UTF-8 相同");
        assert!(std::str::from_utf8(&bg).is_err(), "GBK 文件不应是合法 UTF-8");
        let t8 = std::fs::read_to_string(f8).unwrap();
        let tg = txt::read_text_file(fg).unwrap();
        assert_eq!(t8, tg, "两种编码导出的解码后内容必须逐字符一致");
        assert_eq!(t8.matches('\r').count(), 0);
    }
}

#[test]
fn gbk_rejection_preserves_existing_files_and_writes_no_partial_batch() {
    let input = tempfile::tempdir().unwrap();
    let first = write_shp(input.path(), "a", 1);
    // 第一源可以编码，第二源的DBF字段含emoji；不能先把第一源写出再报错。
    let geom = SurfaceGeometry {
        parts: vec![PolygonPart {
            exterior: vec![
                (500000.0, 3000000.0),
                (500100.0, 3000000.0),
                (500100.0, 3000100.0),
                (500000.0, 3000000.0),
            ],
            holes: vec![],
        }],
    };
    shp::write_shapefile_structured(
        input.path(),
        "b",
        &[geom],
        &[HashMap::from([("DKMC".into(), "名字含😀".into())])],
        "2000国家大地坐标系",
        "3",
        "38",
    )
    .unwrap();
    let paths = vec![first, input.path().join("b.shp")];
    let hdr = header();
    let mut map = mapping("", None);
    map.name = "DKMC".into();
    for mode in ["one_to_one", "split_by_plot", "merge_all"] {
        let out = tempfile::tempdir().unwrap();
        std::fs::write(out.path().join("a.txt"), b"original").unwrap();
        let err = export(&paths, &map, &options(mode, "gbk"), &hdr, out.path()).unwrap_err();
        assert!(err.contains("😀") && err.contains("U+1F600") && err.contains("UTF-8"));
        assert_eq!(
            std::fs::read(out.path().join("a.txt")).unwrap(),
            b"original"
        );
        assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 1);
        let utf8 = tempfile::tempdir().unwrap();
        assert!(export(&paths, &map, &options(mode, "utf8"), &hdr, utf8.path()).is_ok());
    }
}

#[test]
fn legacy_defaults_remain_compatible() {
    let input = tempfile::tempdir().unwrap();
    let shp = write_shp(input.path(), "legacy", 1);
    for (source, expected) in [("__area_sqm__", "9500.00"), ("__area_ha__", "0.9500")] {
        let out = tempfile::tempdir().unwrap();
        let result = export(
            &[shp.clone()],
            &mapping(source, None),
            &options("one_to_one", ""),
            &header(),
            out.path(),
        )
        .unwrap();
        assert_eq!(
            metadata(&std::fs::read_to_string(&result.output_files[0]).unwrap())[1],
            expected
        );
    }
    let legacy: txt::PointLayout = serde_json::from_value(serde_json::json!({
        "columns":[{"kind":"point"}],"distance_unit":"m","distance_decimals":3
    }))
    .unwrap();
    assert!(legacy.sequence_continuous);
}
