// 一次性诊断：用真实测试 SHP 转换，打印 ox=false / ox=true 时第一行坐标
// 验证默认坐标顺序到底是 (X,Y) 还是 (Y,X)
use std::fs;
use std::path::PathBuf;

extern crate jisig_bpoint_converter_lib;
use jisig_bpoint_converter_lib::convert;

#[test]
fn diag_xy_order() {
    let shp = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("test_arcpy")
        .join("std_shp")
        .join("plot_000.shp");
    let shp_path = shp.clone();
    if !shp.exists() {
        eprintln!("测试 SHP 不存在: {}", shp.display());
        return;
        // 让测试直接过，不阻塞
    }

    let header = convert::HeaderConfig {
        crs: "2000国家大地坐标系".into(),
        band: "3".into(),
        proj: "高斯克吕格".into(),
        unit: "米".into(),
        zone: "38".into(),
        precision: "0.001".into(),
        transform: ",,,,,,".into(),
        project_info: String::new(),
    };
    let fm = convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    };

    for (label, ox) in [("ox=false (默认)", false), ("ox=true (标反)", true)] {
        let opts = convert::ShpToTxtOptions {
            ox,
            oj: true,
            on: false,
            oo: true,
            output_mode: "one_to_one".into(),
            filename_field: String::new(),
        };
        let dir = tempfile::tempdir().unwrap();
        let _ = convert::convert_shp_to_txt(
            &[shp_path.clone()],
            None,
            None,
            &header,
            &fm,
            &opts,
            dir.path(),
            None,
        )
        .unwrap();
        let txt = fs::read_to_string(dir.path().join("plot_000.txt")).unwrap();
        let first_coord_line = txt
            .lines()
            .find(|l| l.trim_start_matches('J').split(',').count() == 4)
            .unwrap();
        // 同时打印参照文件的对应行
        eprintln!("\n===== {} =====", label);
        eprintln!("输出第一坐标行: {}", first_coord_line);

        // 参照文件 test_arcpy/txt_output/plot_000.txt 的第 11 行
        // J1,1,2582988.976,38383243.971  (Y=2582988, X=38383243)
        let parts: Vec<&str> = first_coord_line.trim_start_matches('J').split(',').collect();
        let col3: f64 = parts[2].parse().unwrap();
        let col4: f64 = parts[3].parse().unwrap();
        eprintln!(
            "  第3列={}, 第4列={}",
            col3, col4
        );
        // 2582988 是 Y(northing), 38383243 是 X(easting带号)
        if (col3 - 2582988.976).abs() < 1.0 {
            eprintln!("  → 第3列是 Y → 顺序 (Y,X)  ← 与参照文件一致(政府标准)");
        } else if (col4 - 2582988.976).abs() < 1.0 {
            eprintln!("  → 第4列是 Y → 顺序 (X,Y)  ← X 在前");
        }
    }
}
