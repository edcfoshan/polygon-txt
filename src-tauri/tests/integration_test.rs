// 界址点互转工具 — 集成测试
// 测试数据路径: C:\Users\Administrator\Documents\txt与gdb互转\test_data
// 输出目录: 自动创建临时目录

use std::collections::HashMap;
use std::path::PathBuf;

// 引用库
extern crate jisig_bpoint_converter_lib;

// 测试用的模块
use jisig_bpoint_converter_lib::{
    shp, txt, gdb, convert,
};

const TEST_DIR: &str = r"C:\Users\Administrator\Documents\txt与gdb互转\test_arcpy\std_shp";
const TXT_TEST_DIR: &str = r"C:\Users\Administrator\Documents\txt与gdb互转\test_arcpy\txt_output";
const GDB_TEST_DIR: &str = r"C:\Users\Administrator\Documents\txt与gdb互转\test_arcpy\test.gdb";

fn test_shp_stem() -> PathBuf {
    // 用 ArcPy 生成的标准 SHP
    PathBuf::from(TEST_DIR).join("plot_000.shp")
}

fn test_shp_dbf_path() -> PathBuf {
    // SHP 文件夹包含 ArcPy 生成的 DBF
    PathBuf::from(TEST_DIR).join("plot_000.dbf")
}

fn test_txt_path() -> PathBuf {
    PathBuf::from(TXT_TEST_DIR).join("plot_000.txt")
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
    let prj_path = PathBuf::from(TEST_DIR).join("plot_000.prj");
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
    );

    // 验证输出包含关键部分
    assert!(generated.contains("[属性描述]"), "应包含属性描述");
    assert!(generated.contains("[地块坐标]"), "应包含地块坐标");
    assert!(generated.contains("2000国家大地坐标系"), "应包含坐标系");
    assert!(generated.contains(",@"), "应包含 @ 标记");

    // 验证坐标行格式
    assert!(generated.contains("J1,1,"), "应包含 J1 坐标行");
    assert!(generated.contains("J6,1,"), "6点应包含 J6");

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
    use std::collections::HashMap;

    let out_dir = tempfile::tempdir().expect("创建临时目录失败");
    let txt_path = test_txt_path();

    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_gdb: false,
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
    let shp_files_exist: Vec<&str> = result.output_files.iter().filter(|f| f.ends_with(".shp") || f.ends_with(".dbf") || f.ends_with(".prj")).map(|s| s.as_str()).collect();
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
    };

    let result = convert::convert_shp_to_txt(
        &[shp_path.clone()],
        None,
        &header,
        &field_mapping,
        &options,
        out_dir.path(),
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

    let shp_path = test_shp_stem();
    let txt_path = test_txt_path();

    // Step 1: TXT → SHP
    let txt_to_shp_opts = convert::TxtToShpOptions {
        output_shp: true,
        output_gdb: false,
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
    };

    let r2 = convert::convert_shp_to_txt(
        &generated_shp,
        None,
        &header,
        &field_mapping,
        &options,
        out_dir2.path(),
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
    };

    let preview = convert::shp_to_txt_preview(
        &[shp_path.clone()],
        None,
        &header,
        &field_mapping,
        &options,
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
    let gdb_path = PathBuf::from(GDB_TEST_DIR);

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
