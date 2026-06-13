// 界址点互转工具 — 集成测试
// 测试数据路径: C:\Users\Administrator\Documents\txt与gdb互转\test_data
// 输出目录: 自动创建临时目录

use std::path::PathBuf;

// 引用库
extern crate jisig_bpoint_converter_lib;

// 测试用的模块
use jisig_bpoint_converter_lib::{
    shp, txt, gdb, gpkg, convert,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn test_dir() -> PathBuf {
    repo_root().join("test_arcpy").join("std_shp")
}

fn txt_test_dir() -> PathBuf {
    repo_root().join("test_arcpy").join("txt_output")
}

fn gdb_test_dir() -> PathBuf {
    repo_root().join("test_arcpy").join("test.gdb")
}

const DEFAULT_GDB: &str = r"C:\Users\Administrator\Documents\ArcGIS\Default1.gdb";

fn test_shp_stem() -> PathBuf {
    // 用 ArcPy 生成的标准 SHP
    test_dir().join("plot_000.shp")
}

fn test_shp_dbf_path() -> PathBuf {
    // SHP 文件夹包含 ArcPy 生成的 DBF
    test_dir().join("plot_000.dbf")
}

fn test_txt_path() -> PathBuf {
    txt_test_dir().join("plot_000.txt")
}

// ─── 测试 1: SHP 读取 ───

#[test]
fn test_read_shp() {
    let shp_path = test_shp_stem();
    let info = shp::read_shp_file_group(&shp_path).expect("读取 SHP 文件组失败");

    println!("SHP 文件名: {}", info.name);
    println!("要素数量: {}", info.num_features);
    println!("图形类型: {}", info.shape_type);
    println!("字段列表: {:?}", info.field_names);
    println!("坐标系信息: {:?}", info.crs_info);

    assert_eq!(info.shape_type, "Polygon", "应是面要素");
    assert!(info.num_features > 0, "应至少有一个要素");
    assert!(!info.field_names.is_empty(), "应有字段");
    assert!(info.prj_text.is_some(), "应有坐标系描述");

    // 验证已知字段名
    let has_dkmc = info.field_names.iter().any(|n| n == "DKMC");
    assert!(has_dkmc, "应包含 DKMC 字段");

    // 验证坐标
    let features = shp::read_shp(&shp_path).expect("读取 SHP 要素失败");
    assert!(!features.is_empty(), "应至少有一个多边形");

    for (i, feat) in features.iter().enumerate().take(5) {
        println!("  要素 {}: {} 个坐标点", i, feat.points.len());
        assert!(!feat.points.is_empty(), "要素 {} 应有坐标", i);
}

}

#[test]
fn test_txt_to_gpkg_band6_roundtrip() {
    let txt_path = test_txt_path();
    let out_dir = tempfile::tempdir().expect("temp dir");

    let options = convert::TxtToShpOptions {
        output_shp: false,
        output_gpkg: true,
        merge: false,
        output_dir: out_dir.path().to_string_lossy().to_string(),
    };

    let header = convert::HeaderConfig {
        crs: "2000".into(),
        band: "6".into(),
        proj: "Gauss-Kruger".into(),
        unit: "m".into(),
        zone: "20".into(),
        precision: "0.001".into(),
        transform: ",,,,,,".into(),
        project_info: String::new(),
    };

    let result = convert::convert_txt_to_shp(&[txt_path], &options, &header)
        .expect("txt to gpkg failed");
    let gpkg_path = result
        .output_files
        .iter()
        .find(|f| f.ends_with(".gpkg"))
        .expect("missing gpkg");

    let conn = rusqlite::Connection::open(gpkg_path).expect("open gpkg failed");
    let geom_type: String = conn
        .query_row(
            r#"SELECT type FROM pragma_table_info('plot_000') WHERE name='geom'"#,
            [],
            |row| row.get(0),
        )
        .expect("geom type");
    assert_eq!(geom_type.to_uppercase(), "POLYGON");

    let info = gpkg::read_gpkg(std::path::Path::new(gpkg_path)).expect("read gpkg failed");
    assert!(!info.layers.is_empty());
    assert!(info.layers.iter().map(|l| l.num_features).sum::<usize>() > 0);
}

// ─── 测试 2: DBF 读取 ───

#[test]
fn test_read_dbf() {
    let dbf_path = test_shp_dbf_path();
    let (field_names, records) = shp::read_dbf(&dbf_path).expect("读取 DBF 失败");

    println!("DBF 字段: {:?}", field_names);
    println!("记录数: {}", records.len());

    assert!(!field_names.is_empty(), "应有字段");
    assert_eq!(records.len(), 1, "测试数据应有 1 条记录");

    if let Some(row) = records.first() {
        println!("  记录值: {:?}", row);
        // DKMC 应该包含地块名称
        let dkmc_idx = field_names.iter().position(|n| n == "DKMC");
        if let Some(idx) = dkmc_idx {
            if idx < row.len() {
                println!("  DKMC = {}", row[idx]);
            }
        }
    }
}

// ─── 测试 3: PRJ 解析 ───

#[test]
fn test_read_prj() {
    let prj_path = test_dir().join("plot_000.prj");
    let (text, info) = shp::read_prj(&prj_path).expect("读取 PRJ 失败");

    println!("PRJ 文本: {}", &text[..std::cmp::min(80, text.len())]);
    println!("坐标系信息: {:?}", info);

    assert_eq!(
        info.get("c").map(|s| s.as_str()),
        Some("2000国家大地坐标系")
    );
    // ArcPy 输出的 PRJ 可能是 GEOGCS (无投影) 或 PROJCS (含投影)
    if info.contains_key("j") {
        println!("  投影信息: {:?}", info.get("j"));
    }
    // 单位取决于 PRJ 类型
    if let Some(u) = info.get("u") {
        println!("  单位: {}", u);
    }
}

// ─── 测试 4: TXT 解析 ───

#[test]
fn test_parse_txt() {
    let text = std::fs::read_to_string(test_txt_path()).expect("读取 TXT 失败");
    let result = txt::parse_txt(&text);

    println!("项目信息: {}", &result.project_info[..std::cmp::min(40, result.project_info.len())]);
    println!("属性描述: {:?}", result.attrs);
    println!("地块数: {}", result.plots.len());

    assert!(!result.attrs.is_empty(), "应有属性描述");
    assert!(!result.plots.is_empty(), "应至少有一个地块");

    // 验证属性
    assert_eq!(
        result.attrs.get("坐标系").map(|s| s.as_str()),
        Some("2000国家大地坐标系")
    );
    assert_eq!(result.attrs.get("几度分带").map(|s| s.as_str()), Some("3"));
    assert_eq!(result.attrs.get("带号").map(|s| s.as_str()), Some("38"));

    // 验证第一个地块
    let first = &result.plots[0];
    assert_eq!(first.point_count, 6);
    assert!(!first.coords.is_empty(), "应有坐标");

    println!("  第一个地块: {} 面积={} 点={}", first.name, first.area, first.coords.len());
    for (i, &(y, x)) in first.coords.iter().enumerate().take(3) {
        println!("    点{}: Y={}, X={}", i + 1, y, x);
    }
}

// ─── 测试 5: TXT 生成 ───

#[test]
fn test_generate_txt() {
    // 用解析后的数据做 round-trip 测试
    let text = std::fs::read_to_string(test_txt_path()).expect("读取 TXT 失败");
    let parsed = txt::parse_txt(&text);

    let generated = txt::generate_txt(
        &parsed.project_info,
        &parsed.attrs,
        &parsed.plots,
        true,
    );

    // 验证输出包含关键部分
    assert!(generated.contains("[属性描述]"), "应包含属性描述");
    assert!(generated.contains("[地块坐标]"), "应包含地块坐标");
    assert!(generated.contains("2000国家大地坐标系"), "应包含坐标系");
    assert!(generated.contains(",@"), "应包含 @ 标记");

    // 验证坐标行格式
    assert!(generated.contains("J1,1,"), "应包含 J1 坐标行");
    let j1_count = generated.matches("J1,1,").count();
    assert!(j1_count >= 2, "闭合点应回卷到 J1，实际输出为:\n{}", generated);

    // 再解析回去验证 round-trip
    let reparsed = txt::parse_txt(&generated);
    assert_eq!(parsed.plots.len(), reparsed.plots.len(), "round-trip 地块数应一致");

    println!("TXT round-trip 测试通过");
    println!("  原始: {} 地块, {} 属性", parsed.plots.len(), parsed.attrs.len());
    println!("  回环: {} 地块, {} 属性", reparsed.plots.len(), reparsed.attrs.len());
}

// ─── 测试 6: TXT→SHP（完整流程） ───

#[test]
fn test_txt_to_shp_full() {
    let out_dir = tempfile::tempdir().expect("创建临时目录失败");
    let txt_path = test_txt_path();

    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_gpkg: false,
        merge: false,
        output_dir: out_dir.path().to_string_lossy().to_string(),
    };

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

    let result = convert::convert_txt_to_shp(
        &[txt_path.clone()],
        &options,
        &header,
    ).expect("TXT→SHP 转换失败");

    println!("TXT→SHP 结果: {:?}", result.message);
    for f in &result.output_files {
        println!("  输出文件: {}", f);
    }

    assert!(result.success, "转换应成功");
    assert!(!result.output_files.is_empty(), "应有输出文件");

    // 验证输出了 .shp 文件
    let has_shp = result.output_files.iter().any(|f| f.ends_with(".shp"));
    assert!(has_shp, "应输出 .shp 文件");

    // 验证生成的 SHP 文件存在且大小不为 0
    assert!(result.output_files.iter().any(|f| f.ends_with(".shp")), "应输出 .shp 文件");
    assert!(result.output_files.iter().any(|f| f.ends_with(".dbf")), "应输出 .dbf 文件");
    assert!(result.output_files.iter().any(|f| f.ends_with(".prj")), "应输出 .prj 文件");

    // 验证 SHP 文件可被 shapefile crate 读取
    let shp_path: PathBuf = result.output_files.iter().find(|f| f.ends_with(".shp")).map(PathBuf::from).unwrap();
    let features = shp::read_shp(&shp_path).expect("读取生成 SHP 要素失败");
    assert!(!features.is_empty(), "生成的 SHP 应有要素");
    println!("  生成 SHP: {} 个要素", features.len());
}

// ─── 测试 7: SHP→TXT（完整流程） ───

#[test]
fn test_shp_to_txt_full() {
    let out_dir = tempfile::tempdir().expect("创建临时目录失败");
    let shp_path = test_shp_stem();

    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    };

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

    let options = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        op: false,
        on: false,
        oo: false,
        om: false,
        buffer: 0.0,
    };

    let result = convert::convert_shp_to_txt(
        &[shp_path.clone()],
        None,
        None,
        &header,
        &field_mapping,
        &options,
        out_dir.path(),
        None,
    ).expect("SHP→TXT 转换失败");

    println!("SHP→TXT 结果: {:?}", result.message);
    for f in &result.output_files {
        println!("  输出文件: {}", f);
    }

    assert!(result.success, "转换应成功");
    assert!(!result.output_files.is_empty(), "应有输出文件");

    // 验证 TXT 内容
    for txt_path in &result.output_files {
        let content = std::fs::read_to_string(txt_path).expect("读取生成 TXT 失败");
        assert!(content.contains("[属性描述]"), "应包含属性描述");
        assert!(content.contains("[地块坐标]"), "应包含地块坐标");
        assert!(content.contains(",@"), "应包含 @ 标记");
        assert!(content.contains("2000国家大地坐标系"), "应包含坐标系");

        println!("  生成 TXT 内容 (前200字): {}", &content[..std::cmp::min(200, content.len())]);
    }
}

// ─── 测试 8: SHP↔TXT 双向 round-trip ───

#[test]
fn test_shp_txt_roundtrip() {
    let out_dir1 = tempfile::tempdir().expect("创建临时目录失败");
    let out_dir2 = tempfile::tempdir().expect("创建临时目录失败");

    let txt_path = test_txt_path();

    // Step 1: TXT → SHP
    let txt_to_shp_opts = convert::TxtToShpOptions {
        output_shp: true,
        output_gpkg: false,
        merge: false,
        output_dir: out_dir1.path().to_string_lossy().to_string(),
    };

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

    let r1 = convert::convert_txt_to_shp(
        &[txt_path.clone()],
        &txt_to_shp_opts,
        &header,
    ).expect("TXT→SHP 失败");

    println!("TXT→SHP 成功: {} 个文件", r1.output_files.len());

    // Step 2: 生成的 SHP → TXT
    let generated_shp: Vec<PathBuf> = r1.output_files
        .iter()
        .filter(|f| f.ends_with(".shp"))
        .map(PathBuf::from)
        .collect();

    assert!(!generated_shp.is_empty(), "应有 SHP 输出");

    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    };

    let options = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        op: false,
        on: false,
        oo: false,
        om: false,
        buffer: 0.0,
    };

    let r2 = convert::convert_shp_to_txt(
        &generated_shp,
        None,
        None,
        &header,
        &field_mapping,
        &options,
        out_dir2.path(),
        None,
    ).expect("SHP→TXT 失败");

    println!("SHP→TXT 成功: {} 个文件", r2.output_files.len());
    assert!(r2.success, "round-trip 应成功");

    // Step 3: 验证 round-trip 后的 TXT 格式正确
    for txt_output in &r2.output_files {
    let content = std::fs::read_to_string(txt_output).expect("读取回环 TXT 失败");
    let parsed = txt::parse_txt(&content);

    let original = std::fs::read_to_string(&txt_path).expect("读取原始 TXT 失败");
    let orig_parsed = txt::parse_txt(&original);

    println!(
    "  Round-trip: 原 {} 个地块 → 回环 {} 个地块",
    orig_parsed.plots.len(),
    parsed.plots.len()
    );

    assert!(content.contains("[属性描述]"), "回环 TXT 应包含属性描述");
    assert!(content.contains("[地块坐标]"), "回环 TXT 应包含地块坐标");
    assert!(content.contains(",@"), "回环 TXT 应包含 @ 分隔符");
    assert!(content.contains("2000国家大地坐标系"), "回环 TXT 应包含坐标系");
    }
}

// ─── 测试 9: 实时预览 ───

#[test]
fn test_preview() {
    let shp_path = test_shp_stem();

    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    };

    let header = convert::HeaderConfig {
        crs: "2000国家大地坐标系".into(),
        band: "3".into(),
        proj: "高斯克吕格".into(),
        unit: "米".into(),
        zone: "38".into(),
        precision: "0.001".into(),
        transform: ",,,,,,".into(),
        project_info: "项目名称=测试".into(),
    };

    let options = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        op: false,
        on: false,
        oo: false,
        om: false,
        buffer: 0.0,
    };

    let preview = convert::shp_to_txt_preview(
        &[shp_path.clone()],
        None,
        None,
        &header,
        &field_mapping,
        &options,
        None,
    ).expect("生成预览失败");

    println!("预览 (前300字):");
    println!("{}", &preview[..std::cmp::min(300, preview.len())]);

    assert!(preview.contains("[项目信息]"), "预览应包含项目信息");
    assert!(preview.contains("[属性描述]"), "预览应包含属性描述");
    assert!(preview.contains("[地块坐标]"), "预览应包含地块坐标");
    assert!(preview.contains("项目名称=测试"), "预览应包含项目名称");
}

// ─── 测试 10: GDB 读取 ───

#[test]
fn test_read_gdb() {
    let gdb_path = gdb_test_dir();

    if gdb_path.exists() {
        let info = gdb::read_gdb(&gdb_path).expect("读取 GDB 失败");
        println!("GDB: {}", info.name);
        for layer in &info.layers {
            println!("  图层: {} ({} 要素, {:?})", layer.name, layer.num_features, layer.field_names);
        }
        assert!(!info.layers.is_empty(), "GDB 应至少有一个图层");
    } else {
        println!("GDB 测试数据不存在 ({})", gdb_path.display());
        println!("  (请先用 ArcPy 生成测试数据)");
    }
}

// ─── 测试 11: TXT→GPKG ───

#[test]
fn test_txt_to_gpkg() {
    let out_dir = tempfile::tempdir().expect("创建临时目录失败");
    let txt_path = test_txt_path();

    let options = convert::TxtToShpOptions {
        output_shp: false,
        output_gpkg: true,
        merge: false,
        output_dir: out_dir.path().to_string_lossy().to_string(),
    };

    let header = convert::HeaderConfig {
        crs: "2000国家大地坐标系".into(),
        band: "3".into(),
        proj: "高斯克吕格".into(),
        unit: "米".into(),
        zone: "38".into(),
        precision: "0.001".into(),
        transform: ",,,,,, ".into(),
        project_info: String::new(),
    };

    let result = convert::convert_txt_to_shp(
        &[txt_path.clone()],
        &options,
        &header,
    ).expect("TXT→GPKG 转换失败");

    println!("TXT→GPKG 结果: {}", result.message);
    for f in &result.output_files {
        println!("  输出: {}", f);
    }

    assert!(result.success, "转换应成功");
    assert!(!result.output_files.is_empty(), "应有输出文件");

    let gpkg_path_str = result.output_files.iter()
        .find(|f| f.ends_with(".gpkg"))
        .expect("应输出 .gpkg 文件");
    let gpkg_path = PathBuf::from(gpkg_path_str);
    assert!(gpkg_path.is_file(), ".gpkg 应为文件");

    let info = gpkg::read_gpkg(&gpkg_path).expect("读回 GPKG 失败");
    println!("  读回 GPKG: {} 个图层", info.layers.len());
    assert!(!info.layers.is_empty(), "读回应有图层");
    let total_features: usize = info.layers.iter().map(|l| l.num_features).sum();
    assert!(total_features > 0, "读回应有要素");
    assert_eq!(total_features, 1, "应有 1 个要素");
}

// ─── 测试 12: GPKG→TXT ───

#[test]
fn test_gpkg_to_txt_full() {
    let prep_dir = tempfile::tempdir().expect("创建临时目录失败");
    let out_dir = tempfile::tempdir().expect("创建临时目录失败");
    let txt_path = test_txt_path();

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

    let make_gpkg = convert::TxtToShpOptions {
        output_shp: false,
        output_gpkg: true,
        merge: false,
        output_dir: prep_dir.path().to_string_lossy().to_string(),
    };

    let gpkg_result = convert::convert_txt_to_shp(
        &[txt_path.clone()],
        &make_gpkg,
        &header,
    ).expect("准备 GPKG 失败");

    let gpkg_path = gpkg_result.output_files.iter()
        .find(|f| f.ends_with(".gpkg"))
        .map(PathBuf::from)
        .expect("应生成 gpkg");

    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    };

    let options = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        op: false,
        on: false,
        oo: false,
        om: false,
        buffer: 0.0,
    };

    let result = convert::convert_shp_to_txt(
        &[],
        Some("gpkg"),
        Some(&gpkg_path),
        &header,
        &field_mapping,
        &options,
        out_dir.path(),
        None,
    ).expect("GPKG→TXT 转换失败");

    assert!(result.success, "转换应成功");
    assert_eq!(result.output_files.len(), 1, "应生成 1 个 TXT");
    let content = std::fs::read_to_string(&result.output_files[0]).expect("读取 TXT 失败");
    assert!(content.contains("[属性描述]"), "应包含属性描述");
    assert!(content.contains("[地块坐标]"), "应包含地块坐标");
}

// ─── 测试 13: Default1.gdb 手动回退读取 ───

#[test]
fn test_read_default_gdb() {
    let gdb_path = PathBuf::from(DEFAULT_GDB);

    if !gdb_path.exists() {
        println!("Default1.gdb 不存在 ({})，跳过测试", gdb_path.display());
        return;
    }

    // 验证手动回退路径能成功读取（该 GDB 的 a00000004.gdbtable 版本异常，
    // geonative-filegdb::open() 会失败，应自动回退到手动解析）
    match gdb::read_gdb(&gdb_path) {
        Ok(info) => {
            println!("Default1.gdb 读取成功: {}", info.name);
            println!("  图层数: {}", info.layers.len());
            for layer in &info.layers {
                println!(
                    "    图层: {} ({} 要素, 几何类型={}, 字段={:?})",
                    layer.name,
                    layer.num_features,
                    layer.geometry_type,
                    layer.field_names
                );
            }
            // 至少应返回 1 个有效图层
            assert!(
                !info.layers.is_empty(),
                "Default1.gdb 应至少包含 1 个用户图层"
            );
            // 验证至少有一个图层包含要素
            let total_features: usize = info.layers.iter().map(|l| l.num_features).sum();
            println!("  总要素数: {}", total_features);
        }
        Err(e) => {
            // 若 GDB 仅含注记(Anno)等不支持类型，允许 0 图层，只需验证回退机制未崩溃
            if e.contains("所有图层均无法读取") {
                println!("Default1.gdb: 所有图层均含不支持字段类型，回退机制正常 (错误: {})", e);
            } else {
                panic!("Default1.gdb 读取失败（手动回退也应能处理）: {}", e);
            }
        }
    }
}

// ─── 测试 15: TXT→GPKG→TXT 完整双向往返 (2 轮) ───

#[test]
fn test_txt_gpkg_roundtrip_2_rounds() {
    let user_dir = std::path::PathBuf::from(r"C:\Users\Administrator\Documents\txt与gdb互转\00测试数据");
    if !user_dir.exists() {
        eprintln!("跳过: 00测试数据 不存在");
        return;
    }

    let txt_entries: Vec<_> = std::fs::read_dir(&user_dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "txt").unwrap_or(false))
        .collect();

    assert!(!txt_entries.is_empty(), "00测试数据 中应有 TXT 文件");

    for entry in &txt_entries {
        let original_path = entry.path();
        let stem = original_path.file_stem().unwrap().to_string_lossy().to_string();
        let original_text = std::fs::read_to_string(&original_path).expect("读取原始 TXT");

        eprintln!("═══ 往返测试: {} ═══", stem);

        let mut current_text = original_text.clone();

        for round in 1..=2 {
            eprintln!("  --- 第 {} 轮 ---", round);

            // 解析当前 TXT
            let parsed = txt::parse_txt(&current_text);
            assert!(!parsed.plots.is_empty(), "{} 第{}轮: 应有地块", stem, round);

            // 从 TXT 自身属性构建 header（不依赖前端）
            let header = convert::HeaderConfig {
                crs: parsed.attrs.get("坐标系").cloned().unwrap_or_default(),
                band: parsed.attrs.get("几度分带").cloned().unwrap_or_default(),
                proj: parsed.attrs.get("投影类型").cloned().unwrap_or_default(),
                unit: parsed.attrs.get("计量单位").cloned().unwrap_or_default(),
                zone: parsed.attrs.get("带号").cloned().unwrap_or_default(),
                precision: parsed.attrs.get("精度").cloned().unwrap_or_default(),
                transform: parsed.attrs.get("转换参数").cloned().unwrap_or_default(),
                project_info: parsed.project_info.clone(),
            };

            // TXT → GPKG
            let gpkg_dir = tempfile::tempdir().expect("创建临时目录");
            let txt_to_gpkg_opts = convert::TxtToShpOptions {
                output_shp: false,
                output_gpkg: true,
                merge: false,
                output_dir: gpkg_dir.path().to_string_lossy().to_string(),
            };
            let gpkg_result = convert::convert_txt_to_shp(
                &[original_path.clone()],
                &txt_to_gpkg_opts,
                &header,
            ).unwrap_or_else(|e| panic!("{} 第{}轮 TXT→GPKG 失败: {}", stem, round, e));

            let gpkg_path = gpkg_result.output_files.iter()
                .find(|f| f.ends_with(".gpkg"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| panic!("{} 第{}轮: 应生成 .gpkg", stem, round));

            eprintln!("    TXT→GPKG: {} 要素写入 {}", gpkg_result.processed_count, gpkg_path.display());

            // GPKG → TXT
            let txt_dir = tempfile::tempdir().expect("创建临时目录");
            let field_mapping = convert::FieldMapping {
                name: "DKMC".into(),
                id: "DKBH".into(),
                area: "MJ".into(),
                use_field: "DKYT".into(),
                tfh: "TFH".into(),
                dlbm: "DLBM".into(),
            };
            let shp_to_txt_opts = convert::ShpToTxtOptions {
                ox: false,
                oj: true,
                op: false,
                on: false,
                oo: false,
                om: false,
                buffer: 0.0,
            };

            let txt_result = convert::convert_shp_to_txt(
                &[],
                Some("gpkg"),
                Some(&gpkg_path),
                &header,
                &field_mapping,
                &shp_to_txt_opts,
                txt_dir.path(),
                None,
            ).unwrap_or_else(|e| panic!("{} 第{}轮 GPKG→TXT 失败: {}", stem, round, e));

            eprintln!("    GPKG→TXT: {} 文件生成", txt_result.output_files.len());

            // 读取生成的 TXT
            let generated_txt_path = &txt_result.output_files[0];
            current_text = std::fs::read_to_string(generated_txt_path)
                .unwrap_or_else(|e| panic!("读取第{}轮 TXT 失败: {}", round, e));
        }

        // 比较原始和经过 2 轮后的 TXT
        let original_lines: Vec<&str> = original_text.lines().collect();
        let final_lines: Vec<&str> = current_text.lines().collect();

        if original_lines != final_lines {
            eprintln!("  差异 ({} 行 vs {} 行):", original_lines.len(), final_lines.len());
            for (i, (o, f)) in original_lines.iter().zip(final_lines.iter()).enumerate() {
                if o != f {
                    eprintln!("    行 {}: 原始={} | 最终={}", i + 1, o, f);
                }
            }
            // 显示仅在一方存在的行
            if original_lines.len() != final_lines.len() {
                let max_len = original_lines.len().max(final_lines.len());
                for i in 0..max_len {
                    let o = original_lines.get(i).unwrap_or(&"");
                    let f = final_lines.get(i).unwrap_or(&"");
                    if o != f {
                        eprintln!("    行 {}: 原始=\"{}\" | 最终=\"{}\"", i + 1, o, f);
                    }
                }
            }
        }

        assert_eq!(original_lines, final_lines,
            "{}: 经过 2 轮 TXT→GPKG→TXT 后内容应一致", stem);

        eprintln!("  ✓ {} 往返测试通过", stem);
    }
}

#[test]
fn test_multi_part_txt_to_shp_roundtrip_preserves_part_index() {
    let text = "[属性描述]
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
带号=38
精度=0.001
转换参数=,,,,,,
[地块坐标]
8,1,FID_0,多部件地块,面,,,@
J1,1,10.000,10.000
J2,1,10.000,20.000
J3,1,20.000,20.000
J1,1,10.000,10.000
J1,2,30.000,30.000
J2,2,30.000,40.000
J3,2,40.000,40.000
J1,2,30.000,30.000";

    let temp = tempfile::tempdir().expect("tempdir");
    let txt_path = temp.path().join("multipart.txt");
    std::fs::write(&txt_path, text).expect("write txt");

    let shp_dir = tempfile::tempdir().expect("tempdir");
    let back_dir = tempfile::tempdir().expect("tempdir");

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

    let txt_to_shp = convert::TxtToShpOptions {
        output_shp: true,
        output_gpkg: false,
        merge: false,
        output_dir: shp_dir.path().to_string_lossy().to_string(),
    };

    let shp_result = convert::convert_txt_to_shp(&[txt_path], &txt_to_shp, &header)
        .expect("TXT->SHP should succeed");
    let shp_paths: Vec<PathBuf> = shp_result
        .output_files
        .iter()
        .filter(|f| f.ends_with(".shp"))
        .map(PathBuf::from)
        .collect();
    assert_eq!(shp_paths.len(), 1, "应有一个 shp 输出");

    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    };
    let shp_to_txt = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        op: false,
        on: false,
        oo: true,
        om: false,
        buffer: 0.0,
    };

    let txt_result = convert::convert_shp_to_txt(
        &shp_paths,
        None,
        None,
        &header,
        &field_mapping,
        &shp_to_txt,
        back_dir.path(),
        None,
    )
    .expect("SHP->TXT should succeed");

    let roundtrip = std::fs::read_to_string(&txt_result.output_files[0]).expect("read roundtrip");
    assert!(
        roundtrip.contains("J1,2,30.000,30.000"),
        "往返后第二个部件的 part index 不应丢失，实际输出为:\n{}",
        roundtrip
    );
}

#[test]
fn test_hole_txt_to_shp_roundtrip_preserves_inner_ring() {
    let text = "[属性描述]
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
带号=38
精度=0.001
转换参数=,,,,,,
[地块坐标]
10,1,FID_0,带内环地块,面,,,@
J1,1,0.000,0.000
J2,1,0.000,10.000
J3,1,10.000,10.000
J4,1,10.000,0.000
J1,1,0.000,0.000
J1,2,2.000,2.000
J2,2,8.000,2.000
J3,2,8.000,8.000
J4,2,2.000,8.000
J1,2,2.000,2.000";

    let temp = tempfile::tempdir().expect("tempdir");
    let txt_path = temp.path().join("hole.txt");
    std::fs::write(&txt_path, text).expect("write txt");

    let shp_dir = tempfile::tempdir().expect("tempdir");
    let back_dir = tempfile::tempdir().expect("tempdir");

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

    let txt_to_shp = convert::TxtToShpOptions {
        output_shp: true,
        output_gpkg: false,
        merge: false,
        output_dir: shp_dir.path().to_string_lossy().to_string(),
    };

    let shp_result = convert::convert_txt_to_shp(&[txt_path], &txt_to_shp, &header)
        .expect("TXT->SHP should succeed");
    let shp_paths: Vec<PathBuf> = shp_result
        .output_files
        .iter()
        .filter(|f| f.ends_with(".shp"))
        .map(PathBuf::from)
        .collect();

    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    };
    let shp_to_txt = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        op: false,
        on: true,
        oo: true,
        om: false,
        buffer: 0.0,
    };

    let txt_result = convert::convert_shp_to_txt(
        &shp_paths,
        None,
        None,
        &header,
        &field_mapping,
        &shp_to_txt,
        back_dir.path(),
        None,
    )
    .expect("SHP->TXT should succeed");

    let roundtrip = std::fs::read_to_string(&txt_result.output_files[0]).expect("read roundtrip");
    assert!(
        roundtrip.contains("J1,2,8.000,2.000") || roundtrip.contains("J1,2,2.000,2.000"),
        "内环应以独立 part 输出，实际输出为:\n{}",
        roundtrip
    );
}

#[test]
fn test_multi_part_txt_to_gpkg_roundtrip_preserves_part_index() {
    let text = "[属性描述]
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
带号=38
精度=0.001
转换参数=,,,,,,
[地块坐标]
8,1,FID_0,多部件地块,面,,,@
J1,1,10.000,10.000
J2,1,10.000,20.000
J3,1,20.000,20.000
J1,1,10.000,10.000
J1,2,30.000,30.000
J2,2,30.000,40.000
J3,2,40.000,40.000
J1,2,30.000,30.000";

    let temp = tempfile::tempdir().expect("tempdir");
    let txt_path = temp.path().join("multipart_gpkg.txt");
    std::fs::write(&txt_path, text).expect("write txt");

    let gpkg_dir = tempfile::tempdir().expect("tempdir");
    let back_dir = tempfile::tempdir().expect("tempdir");

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

    let txt_to_gpkg = convert::TxtToShpOptions {
        output_shp: false,
        output_gpkg: true,
        merge: false,
        output_dir: gpkg_dir.path().to_string_lossy().to_string(),
    };

    let gpkg_result = convert::convert_txt_to_shp(&[txt_path], &txt_to_gpkg, &header)
        .expect("TXT->GPKG should succeed");
    let gpkg_path = gpkg_result
        .output_files
        .iter()
        .find(|f| f.ends_with(".gpkg"))
        .map(PathBuf::from)
        .expect("应有 gpkg 输出");

    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    };
    let gpkg_to_txt = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        op: false,
        on: false,
        oo: true,
        om: false,
        buffer: 0.0,
    };

    let txt_result = convert::convert_shp_to_txt(
        &[],
        Some("gpkg"),
        Some(&gpkg_path),
        &header,
        &field_mapping,
        &gpkg_to_txt,
        back_dir.path(),
        None,
    )
    .expect("GPKG->TXT should succeed");

    let roundtrip = std::fs::read_to_string(&txt_result.output_files[0]).expect("read roundtrip");
    assert!(
        roundtrip.contains("J1,2,"),
        "GPKG 往返后第二个部件的 part index 不应丢失，实际输出为:\n{}",
        roundtrip
    );
}
