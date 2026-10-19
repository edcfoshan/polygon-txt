use jisig_bpoint_converter_lib::{
    config::Config,
    convert,
    geometry::{PolygonPart, SurfaceGeometry},
    output, shp,
};
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path, process::Command};

fn cli(args: &[&str]) -> (bool, Value) {
    let result = Command::new(env!("CARGO_BIN_EXE_jisigg-cli"))
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    let value = serde_json::from_slice(&result.stdout).unwrap_or_else(|e| {
        panic!(
            "{}: stdout={}, stderr={}",
            e,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        )
    });
    (result.status.success(), value)
}

fn fixture(root: &Path) -> std::path::PathBuf {
    let ring = vec![
        (38500000., 2500000.),
        (38500100., 2500000.),
        (38500100., 2500100.),
        (38500000., 2500100.),
        (38500000., 2500000.),
    ];
    let geometry = SurfaceGeometry {
        parts: vec![PolygonPart {
            exterior: ring,
            holes: vec![],
        }],
    };
    let attrs = HashMap::from([("DKMC".into(), "测试地块".into())]);
    shp::write_shapefile_structured(
        root,
        "地块",
        &[geometry],
        &[attrs],
        "2000国家大地坐标系",
        "3",
        "38",
    )
    .unwrap();
    root.join("地块.shp")
}

fn config(root: &Path, mode: &str) -> (Config, std::path::PathBuf) {
    let mut c = Config::default();
    c.shp_to_txt.options.output_mode = mode.into();
    c.shp_to_txt.fields.name = "DKMC".into();
    c.shp_to_txt
        .header
        .attrs
        .iter_mut()
        .find(|a| a.k == "带号")
        .unwrap()
        .v = "38".into();
    c.txt_to_shp.output_mode = mode.into();
    let path = root.join("配置.json");
    std::fs::write(&path, serde_json::to_vec(&c).unwrap()).unwrap();
    (c, path)
}

#[test]
fn config_roundtrip_and_validation() {
    let mut c = Config::default();
    c.shp_to_txt.editor.rows = Some(vec![]);
    c.txt_to_shp.keep_lujin = true;
    c.active_direction = "txt-to-shp".into();
    let text = serde_json::to_string(&c).unwrap();
    assert_eq!(
        serde_json::to_value(Config::parse(&text).unwrap()).unwrap(),
        serde_json::to_value(&c).unwrap()
    );
    let mut value = serde_json::to_value(&c).unwrap();
    value["shp_to_txt"]["options"]["output_dir"] = json!("private/path");
    assert!(Config::parse(&value.to_string()).is_err());
    c.shp_to_txt.fields.area_decimals = Some(7);
    assert!(c.validate().is_err());
    c.shp_to_txt.fields.area_decimals = Some(4);
    c.schema_version = 2;
    assert!(c.validate().is_err());
}

#[test]
fn cli_matches_core_bytes_for_all_output_modes() {
    for mode in ["one_to_one", "split_by_plot", "merge_all"] {
        let temp = tempfile::tempdir().unwrap();
        let source = fixture(temp.path());
        let (c, cfg) = config(temp.path(), mode);
        let actual = temp.path().join("cli");
        let expected = temp.path().join("core");
        let (ok, result) = cli(&[
            "shp-to-txt",
            "--input",
            source.to_str().unwrap(),
            "--config",
            cfg.to_str().unwrap(),
            "--output-dir",
            actual.to_str().unwrap(),
        ]);
        assert!(ok, "{}", result);
        let core = convert::convert_shp_to_txt(
            &[source],
            None,
            None,
            &c.shp_to_txt.header,
            &c.shp_to_txt.fields,
            &c.shp_to_txt.options,
            &expected,
            None,
        )
        .unwrap();
        for (a, b) in result["output_files"]
            .as_array()
            .unwrap()
            .iter()
            .zip(core.output_files)
        {
            assert_eq!(
                std::fs::read(a.as_str().unwrap()).unwrap(),
                std::fs::read(b).unwrap()
            );
        }
        let txt = result["output_files"][0].as_str().unwrap();
        let shp_dir = temp.path().join("reverse");
        let (ok, reverse) = cli(&[
            "txt-to-shp",
            "--input",
            txt,
            "--config",
            cfg.to_str().unwrap(),
            "--output-dir",
            shp_dir.to_str().unwrap(),
        ]);
        assert!(ok, "{}", reverse);
        assert!(reverse["output_files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().unwrap().ends_with(".shx")));
        let output = reverse["output_files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f.as_str().unwrap().ends_with(".shp"))
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(shp::read_shp(Path::new(output)).unwrap().len(), 1);
    }
}

#[test]
fn dry_run_writes_nothing_and_conflicts_preserve_all_files() {
    let temp = tempfile::tempdir().unwrap();
    let source = fixture(temp.path());
    let (_, cfg) = config(temp.path(), "one_to_one");
    let out = temp.path().join("not-created");
    let args = [
        "shp-to-txt",
        "--input",
        source.to_str().unwrap(),
        "--config",
        cfg.to_str().unwrap(),
        "--output-dir",
        out.to_str().unwrap(),
    ];
    let mut dry = args.to_vec();
    dry.push("--dry-run");
    let (ok, result) = cli(&dry);
    assert!(ok, "{}", result);
    assert!(!out.exists());
    assert!(cli(&args).0);
    let target = out.join("地块.txt");
    std::fs::write(&target, "保留原文件").unwrap();
    assert!(!cli(&args).0);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "保留原文件");
    let mut overwrite = args.to_vec();
    overwrite.push("--overwrite");
    assert!(cli(&overwrite).0);
    let reverse = temp.path().join("reverse");
    std::fs::create_dir(&reverse).unwrap();
    std::fs::write(reverse.join("地块.shx"), "existing index").unwrap();
    let (ok, _) = cli(&[
        "txt-to-shp",
        "--input",
        target.to_str().unwrap(),
        "--config",
        cfg.to_str().unwrap(),
        "--output-dir",
        reverse.to_str().unwrap(),
    ]);
    assert!(!ok);
    assert!(!reverse.join("地块.shp").exists());
    assert_eq!(
        std::fs::read_to_string(reverse.join("地块.shx")).unwrap(),
        "existing index"
    );
}

#[test]
fn invalid_mapping_and_invalid_args_are_json_errors_without_outputs() {
    let temp = tempfile::tempdir().unwrap();
    let source = fixture(temp.path());
    let (mut c, cfg) = config(temp.path(), "one_to_one");
    c.shp_to_txt.fields.name = "不存在的字段".into();
    std::fs::write(&cfg, serde_json::to_vec(&c).unwrap()).unwrap();
    let out = temp.path().join("out");
    let (ok, result) = cli(&[
        "shp-to-txt",
        "--input",
        source.to_str().unwrap(),
        "--config",
        cfg.to_str().unwrap(),
        "--output-dir",
        out.to_str().unwrap(),
    ]);
    assert!(!ok);
    assert!(result["error"].as_str().unwrap().contains("不存在的字段"));
    assert!(!out.exists());
    let (ok, error) = cli(&["shp-to-txt", "--unknown"]);
    assert!(!ok);
    assert_eq!(error["success"], false);
}

#[test]
fn capture_error_is_scoped_and_commit_checks_whole_batch() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a.txt");
    let b = temp.path().join("b.txt");
    let failed: Result<((), Vec<output::PlannedFile>), String> = output::capture(|| {
        output::write(&a, "must not exist").unwrap();
        Err("invalid input".into())
    });
    assert!(failed.is_err());
    assert!(!a.exists());
    std::fs::write(&b, "old").unwrap();
    let files = vec![
        (a.clone(), b"new a".to_vec()),
        (b.clone(), b"new b".to_vec()),
    ];
    assert!(output::commit(&files, false).is_err());
    assert!(!a.exists());
    assert_eq!(std::fs::read(&b).unwrap(), b"old");
}

#[test]
fn gbk_failure_and_invalid_plot_stop_before_writing() {
    let temp = tempfile::tempdir().unwrap();
    let source = fixture(temp.path());
    let (mut c, cfg) = config(temp.path(), "one_to_one");
    c.shp_to_txt.header.project_info = "无法编码😀".into();
    c.shp_to_txt.options.txt_encoding = "gbk".into();
    std::fs::write(&cfg, serde_json::to_vec(&c).unwrap()).unwrap();
    let out = temp.path().join("out");
    let args = [
        "shp-to-txt",
        "--input",
        source.to_str().unwrap(),
        "--config",
        cfg.to_str().unwrap(),
        "--output-dir",
        out.to_str().unwrap(),
    ];
    let (ok, error) = cli(&args);
    assert!(!ok, "{}", error);
    assert!(!out.exists());
    let mut utf8 = args.to_vec();
    utf8.extend(["--encoding", "utf8", "--dry-run"]);
    assert!(cli(&utf8).0);
    assert!(!out.exists());
    utf8.extend(["--plot", "0:0", "--plot", "0:99"]);
    let (ok, error) = cli(&utf8);
    assert!(!ok);
    assert!(error["error"].as_str().unwrap().contains("0:99"));
}

#[test]
fn gdb_inspection_and_conversion_use_same_core() {
    let temp = tempfile::tempdir().unwrap();
    let shp = fixture(temp.path());
    let features = shp::read_shp(&shp).unwrap();
    let ring = features[0].points.clone();
    jisig_bpoint_converter_lib::gdb::write_gdb_output(
        temp.path(),
        "sample",
        &[("DKMC".into(), "地块名称".into(), 4, 50)],
        &[HashMap::from([("DKMC".into(), "测试".into())])],
        &[ring],
        &HashMap::from([
            ("c".into(), "2000国家大地坐标系".into()),
            ("b".into(), "3".into()),
            ("z".into(), "38".into()),
        ]),
    )
    .unwrap();
    let source = temp.path().join("sample.gdb");
    let (ok, inspection) = cli(&["inspect", "--input", source.to_str().unwrap()]);
    assert!(ok, "{}", inspection);
    let out = temp.path().join("gdb-out");
    let (ok, result) = cli(&[
        "shp-to-txt",
        "--input",
        source.to_str().unwrap(),
        "--output-dir",
        out.to_str().unwrap(),
        "--dry-run",
    ]);
    assert!(ok, "{}", result);
    assert!(!out.exists());
    assert!(result["processed_count"].as_u64().unwrap() > 0);
}

#[test]
fn advanced_layout_roundtrip_and_nested_unknown_fields_rejected() {
    let mut value = serde_json::to_value(Config::default()).unwrap();
    let rows = json!([{"name":"面积","source":"__area_ha__","area_decimals":6}]);
    value["shp_to_txt"]["editor"]["advanced"] = json!(true);
    value["shp_to_txt"]["editor"]["rows"] = rows.clone();
    value["shp_to_txt"]["fields"]["columns"] = rows;
    value["shp_to_txt"]["options"]["point_layout"] = json!({"columns":[
        {"kind":"sequence"},{"kind":"point"},{"kind":"ring"},{"kind":"y"},{"kind":"x"},
        {"kind":"distance","distance_unit":"km","distance_decimals":6}, {"kind":"fixed","value":"X"}],
        "stake_field":"","stake_default":"木桩","distance_unit":"m","distance_decimals":3,"sequence_continuous":false});
    let config = Config::parse(&value.to_string()).unwrap();
    assert_eq!(
        Config::parse(&serde_json::to_string(&config).unwrap())
            .unwrap()
            .shp_to_txt
            .fields
            .columns[0]
            .area_decimals,
        Some(6)
    );
    value["shp_to_txt"]["options"]["point_layout"]["columns"][0]["private_path"] = json!("x");
    assert!(Config::parse(&value.to_string()).is_err());
}
