use std::path::PathBuf;

#[test]
fn release_smoke_roundtrip_gpkg_preview() {
    let txt_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("test_arcpy")
        .join("txt_output")
        .join("plot_000.txt");

    if !txt_path.exists() {
        panic!("Smoke TXT 不存在: {}", txt_path.display());
    }

    let out_dir = std::env::temp_dir().join("jisig_arcpy_verify");
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir).ok();
    }
    std::fs::create_dir_all(&out_dir).expect("创建 smoke 临时目录失败");
    let report = jisig_bpoint_converter_lib::smoke::run_release_smoke(
        jisig_bpoint_converter_lib::SmokeTestConfig {
            txt_path: txt_path.clone(),
            output_dir: out_dir.clone(),
        },
    )
    .expect("release smoke 运行失败");

    println!("{}", report);
    assert!(report.contains("SMOKE_OK"));
    let gpkg_path = out_dir.join("plot_000.gpkg");
    assert!(gpkg_path.exists(), "应生成 GPKG");
    assert!(
        out_dir.join("plot_000_preview.txt").exists(),
        "应生成 preview txt"
    );

    let conn = rusqlite::Connection::open(&gpkg_path).expect("打开 GPKG 失败");
    let mut stmt = conn
        .prepare(r#"SELECT geom FROM "plot_000" LIMIT 1"#)
        .expect("读取几何失败");
    let blob: Vec<u8> = stmt
        .query_row([], |row| row.get(0))
        .expect("读取几何 blob 失败");
    assert!(blob.starts_with(b"GP"), "GeoPackage 几何头应以 GP 开头");
    assert_eq!(blob[2], 0, "GeoPackage version 应为 0");
    assert_eq!(blob[3], 1, "GeoPackage flags 应为 0x01");
}
