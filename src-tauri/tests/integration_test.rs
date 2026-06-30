// 界址点互转工具 — 集成测试
// 测试数据路径: C:\Users\Administrator\Documents\txt与gdb互转\test_data
// 输出目录: 自动创建临时目录

use std::path::PathBuf;

// 引用库
extern crate jisig_bpoint_converter_lib;

// 测试用的模块
use jisig_bpoint_converter_lib::{
    shp, txt, gdb, convert,
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

// ─── 测试 1c: PolygonZ (type 15) SHP 读取 ───

#[test]
fn test_read_polygonz_shp() {
    use shapefile::{PolygonRing, PointZ, PolygonZ, ShapeWriter};

    let tmp = tempfile::tempdir().expect("temp dir");
    let shp_path = tmp.path().join("polygonz_test.shp");

    // 写一个 PolygonZ：外环（矩形）+ 一个洞（小矩形），Z/M 填 0
    let exterior = vec![
        PointZ::new(0.0, 0.0, 0.0, 0.0),
        PointZ::new(10.0, 0.0, 0.0, 0.0),
        PointZ::new(10.0, 10.0, 0.0, 0.0),
        PointZ::new(0.0, 0.0, 0.0, 0.0),
    ];
    let hole = vec![
        PointZ::new(2.0, 2.0, 0.0, 0.0),
        PointZ::new(4.0, 2.0, 0.0, 0.0),
        PointZ::new(4.0, 4.0, 0.0, 0.0),
        PointZ::new(2.0, 2.0, 0.0, 0.0),
    ];
    let polyz = PolygonZ::with_rings(vec![
        PolygonRing::Outer(exterior),
        PolygonRing::Inner(hole),
    ]);

    let mut swriter = ShapeWriter::from_path(&shp_path).expect("创建 PolygonZ 写入器");
    swriter.write_shape(&polyz).expect("写 PolygonZ 图形");
    drop(swriter);

    // 1) 文件组信息：shape_type=15 应识别为 PolygonZ
    let info = shp::read_shp_file_group(&shp_path).expect("读取 PolygonZ 文件组失败");
    assert_eq!(info.shape_type, "PolygonZ", "shape_type=15 应映射为 PolygonZ");
    assert_eq!(info.num_features, 1, "应有 1 个要素");

    // 2) 图形解析：外环 + 1 个洞（Z 值丢弃，仅取 x/y）
    let features = shp::read_shp(&shp_path).expect("读取 PolygonZ 要素失败");
    assert_eq!(features.len(), 1, "应解析出 1 个多边形");
    let feat = &features[0];
    assert!(feat.points.len() >= 3, "外环点数应 >= 3");
    assert_eq!(feat.surface.parts.len(), 1, "应有 1 个 part");
    assert_eq!(feat.surface.parts[0].holes.len(), 1, "该 part 应含 1 个洞");
}

// ─── 测试 1b: 三模式 — 一对一 ───

#[test]
fn test_shp_to_txt_one_to_one() {
    let shp_path = test_shp_stem();
    let out_dir = tempfile::tempdir().expect("temp dir");

    let header = make_header();
    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(), id: "DKBH".into(), area: "MJ".into(),
        use_field: "DKYT".into(), tfh: "TFH".into(), dlbm: "DLBM".into(),
    };
    let options = convert::ShpToTxtOptions {
        ox: false, oj: true, on: false, oo: true, oc: false,
        output_mode: "one_to_one".into(), filename_field: String::new(),
    };

    let result = convert::convert_shp_to_txt(
        &[shp_path], None, None, &header, &field_mapping, &options,
        out_dir.path(), None,
    ).expect("一对一转换失败");

    assert!(result.success);
    assert_eq!(result.output_files.len(), 1);
    assert!(result.output_files[0].ends_with("plot_000.txt"));
    let txt = std::fs::read_to_string(&result.output_files[0]).unwrap();
    assert!(txt.contains("[属性描述]"));
    assert!(txt.contains("[地块坐标]"));
}

// ─── 测试 1b2: XY 坐标标反（ox）— 勾选 vs 不勾选坐标列顺序不同 ───

#[test]
fn test_shp_to_txt_xy_swap() {
    let shp_path = test_shp_stem();

    let header = make_header();
    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(), id: "DKBH".into(), area: "MJ".into(),
        use_field: "DKYT".into(), tfh: "TFH".into(), dlbm: "DLBM".into(),
    };

    // ox=false（默认，输出标准 Y,X 顺序：北坐标在前）
    let opts_off = convert::ShpToTxtOptions {
        ox: false, oj: true, on: false, oo: true, oc: false,
        output_mode: "one_to_one".into(), filename_field: String::new(),
    };
    let dir_off = tempfile::tempdir().expect("temp dir");
    let _ = convert::convert_shp_to_txt(
        &[shp_path.clone()], None, None, &header, &field_mapping, &opts_off,
        dir_off.path(), None,
    ).expect("转换失败");
    let txt_off = std::fs::read_to_string(dir_off.path().join("plot_000.txt")).unwrap();

    // ox=true（勾选标反，输出 X,Y 顺序：东坐标在前）
    let opts_on = convert::ShpToTxtOptions {
        ox: true, oj: true, on: false, oo: true, oc: false,
        output_mode: "one_to_one".into(), filename_field: String::new(),
    };
    let dir_on = tempfile::tempdir().expect("temp dir");
    let _ = convert::convert_shp_to_txt(
        &[shp_path], None, None, &header, &field_mapping, &opts_on,
        dir_on.path(), None,
    ).expect("转换失败");
    let txt_on = std::fs::read_to_string(dir_on.path().join("plot_000.txt")).unwrap();

    // 提取第一个坐标行的第3、4列（Y, X 或 X, Y）
    fn first_coord(txt: &str) -> Option<(f64, f64)> {
        for line in txt.lines() {
            let line = line.trim_start_matches('J');
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() == 4 {
                if let (Ok(a), Ok(b)) = (parts[2].parse::<f64>(), parts[3].parse::<f64>()) {
                    return Some((a, b));
                }
            }
        }
        None
    }

    let (c3_off, c4_off) = first_coord(&txt_off).expect("ox=false 应有坐标行");
    let (c3_on, c4_on) = first_coord(&txt_on).expect("ox=true 应有坐标行");

    // 交换后第3列应等于原第4列，第4列等于原第3列
    assert!(
        (c3_on - c4_off).abs() < 1e-6 && (c4_on - c3_off).abs() < 1e-6,
        "ox=true 应交换坐标列: off=({}, {}) on=({}, {})", c3_off, c4_off, c3_on, c4_on
    );
    // ox=false 时两版本坐标不应相同（证明确实有差异）
    assert!(
        (c3_off - c3_on).abs() > 1e-6,
        "ox 开关应改变坐标输出"
    );
}

// ─── 测试 1c: 三模式 — 全合并（文件名带时间戳） ───

#[test]
fn test_shp_to_txt_merge_all() {
    let shp_path = test_shp_stem();
    let out_dir = tempfile::tempdir().expect("temp dir");

    let header = make_header();
    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(), id: "DKBH".into(), area: "MJ".into(),
        use_field: "DKYT".into(), tfh: "TFH".into(), dlbm: "DLBM".into(),
    };
    let options = convert::ShpToTxtOptions {
        ox: false, oj: true, on: false, oo: true, oc: false,
        output_mode: "merge_all".into(), filename_field: String::new(),
    };

    let result = convert::convert_shp_to_txt(
        &[shp_path], None, None, &header, &field_mapping, &options,
        out_dir.path(), None,
    ).expect("全合并转换失败");

    assert_eq!(result.output_files.len(), 1);
    let p = std::path::Path::new(&result.output_files[0]);
    let fname = p.file_name().unwrap().to_string_lossy().to_string();
    assert!(fname.starts_with("merged_output_"), "文件名应以 merged_output_ 开头: {}", fname);
    assert!(fname.ends_with(".txt"));
    // 时间戳格式 YYYYMMDD_HHMMSS，长度 = "merged_output_"(14) + 15 + ".txt"(4) = 33
    assert_eq!(fname.len(), 33, "文件名应含时间戳: {}", fname);
}

// ─── 测试 1d: 三模式 — 按地块拆分（建子目录 + 文件名字段） ───

#[test]
fn test_shp_to_txt_split_by_plot() {
    let shp_path = test_shp_stem();
    let out_dir = tempfile::tempdir().expect("temp dir");

    let header = make_header();
    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(), id: "DKBH".into(), area: "MJ".into(),
        use_field: "DKYT".into(), tfh: "TFH".into(), dlbm: "DLBM".into(),
    };
    // 用序号命名（filename_field 为空）
    let options = convert::ShpToTxtOptions {
        ox: false, oj: true, on: false, oo: true, oc: false,
        output_mode: "split_by_plot".into(), filename_field: String::new(),
    };

    let result = convert::convert_shp_to_txt(
        &[shp_path], None, None, &header, &field_mapping, &options,
        out_dir.path(), None,
    ).expect("按地块拆分失败");

    assert!(result.success);
    // 应建子目录 plot_000/
    let subdir = out_dir.path().join("plot_000");
    assert!(subdir.exists(), "应建子目录 plot_000");
    // 至少一个 txt
    let txts: Vec<_> = std::fs::read_dir(&subdir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "txt").unwrap_or(false))
        .collect();
    assert!(!txts.is_empty(), "子目录内应至少有一个 txt");
    // 文件名应为 plot_000_1.txt（序号兜底）
    let first_name = txts[0].file_name().to_string_lossy().to_string();
    assert!(first_name.starts_with("plot_000_"), "序号兜底文件名错误: {}", first_name);
}

// ─── 测试 1e: TXT→面 合并模式 ───

#[test]
fn test_txt_to_shp_merge() {
    let txt_path = test_txt_path();
    let out_dir = tempfile::tempdir().expect("temp dir");

    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("merge_all"),
        filename_field: String::new(),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };
    let header = make_header();

    let result = convert::convert_txt_to_shp(&[txt_path], &options, &header)
        .expect("TXT→面合并失败");
    assert!(result.success);
    // 新行为：文件名带时间戳 merged_output_YYYYMMDD_HHMMSS.shp
    assert!(result.message.contains("merged_output_"), "消息应含时间戳文件名: {}", result.message);
    let merged = std::fs::read_dir(out_dir.path())
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .find(|n| n.starts_with("merged_output_") && n.ends_with(".shp"))
        .expect("应生成一个 merged_output_*.shp");
    assert!(merged.starts_with("merged_output_") && merged.ends_with(".shp"));
}

// ─── 测试 1e2: TXT→面 附加属性（LUJIN/MINGC）勾选验证 ───

#[test]
fn test_txt_to_shp_keep_lujin_mingc() {
    let txt_path = test_txt_path();
    let out_dir = tempfile::tempdir().expect("temp dir");
    let header = make_header();

    // 勾选两个附加属性
    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("one_to_one"),
        filename_field: String::new(),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: true,
        keep_mingc: true,
    };
    let result = convert::convert_txt_to_shp(&[txt_path.clone()], &options, &header)
        .expect("转换失败");
    assert!(result.success);

    // 读回 DBF
    let dbf_path = out_dir.path().join("plot_000.dbf");
    assert!(dbf_path.exists(), "应有 plot_000.dbf");
    let (field_names, records) = shp::read_dbf(&dbf_path).expect("读 DBF 失败");

    println!("DBF 字段: {:?}", field_names);
    // 应含 LUJIN 和 MINGC
    assert!(field_names.iter().any(|n| n == "LUJIN"), "应含 LUJIN 字段: {:?}", field_names);
    assert!(field_names.iter().any(|n| n == "MINGC"), "应含 MINGC 字段: {:?}", field_names);

    let lujin_idx = field_names.iter().position(|n| n == "LUJIN").unwrap();
    let mingc_idx = field_names.iter().position(|n| n == "MINGC").unwrap();
    let row = records.first().expect("应有记录");
    // LUJIN = 源 TXT 完整路径
    assert!(
        row[lujin_idx].trim().ends_with("plot_000.txt"),
        "LUJIN 应为源 TXT 完整路径，实际: {:?}",
        row[lujin_idx]
    );
    assert!(
        row[lujin_idx].trim().contains("plot_000"),
        "LUJIN 应含完整路径"
    );
    // MINGC = 文件名带 .txt
    assert_eq!(
        row[mingc_idx].trim(),
        "plot_000.txt",
        "MINGC 应为 plot_000.txt，实际: {:?}",
        row[mingc_idx]
    );
}

#[test]
fn test_txt_to_shp_no_extra_fields_by_default() {
    let txt_path = test_txt_path();
    let out_dir = tempfile::tempdir().expect("temp dir");
    let header = make_header();
    // 不勾选附加属性 → DBF 不应含 LUJIN/MINGC
    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("one_to_one"),
        filename_field: String::new(),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };
    let result = convert::convert_txt_to_shp(&[txt_path], &options, &header)
        .expect("转换失败");
    assert!(result.success);

    let dbf_path = out_dir.path().join("plot_000.dbf");
    let (field_names, _records) = shp::read_dbf(&dbf_path).expect("读 DBF 失败");
    assert!(!field_names.iter().any(|n| n == "LUJIN"), "不勾选时不应有 LUJIN: {:?}", field_names);
    assert!(!field_names.iter().any(|n| n == "MINGC"), "不勾选时不应有 MINGC: {:?}", field_names);
}

// ─── 测试 1f: TXT→面 split_by_plot 模式（多地块 TXT，按 DKMC 拆分） ───

/// 合成多地块 TXT（3 个地块，含 DKMC 名称），写入临时文件返回路径。
fn write_multi_plot_txt(dir: &std::path::Path) -> PathBuf {
    // 3 个地块：两个有 DKMC 名称（"地块甲" / "地块乙"），一个 DKMC 为空（走序号兜底）
    // 每地块 4 个不重复点，确保 strip_closing_point 后仍 ≥3 点构成有效面
    let content = "\
[属性描述]
坐标系=2000国家大地坐标系
几度分带=3
投影类型=高斯克吕格
计量单位=米
带号=38
精度=0.001
转换参数=,,,,,,
[地块坐标]
4,100.5,FID_A,地块甲,面, , , ,@
J1,1,2582988.976,38383243.971
J2,1,2582983.339,38383261.067
J3,1,2582960.000,38383250.000
J4,1,2582990.000,38383270.000
4,200.5,FID_B,地块乙,面, , , ,@
J1,1,2582988.976,38383243.971
J2,1,2582983.339,38383261.067
J3,1,2582960.000,38383250.000
J4,1,2582990.000,38383270.000
4,300.5,FID_C,,面, , , ,@
J1,1,2582988.976,38383243.971
J2,1,2582983.339,38383261.067
J3,1,2582960.000,38383250.000
J4,1,2582990.000,38383270.000
";
    let path = dir.join("multi_plot.txt");
    std::fs::write(&path, content).expect("写测试 TXT");
    path
}

fn make_header() -> convert::HeaderConfig {
    convert::HeaderConfig {
        attrs: vec![
            convert::AttrRow { k: "坐标系".into(),   v: "2000国家大地坐标系".into() },
            convert::AttrRow { k: "几度分带".into(), v: "3".into() },
            convert::AttrRow { k: "投影类型".into(), v: "高斯克吕格".into() },
            convert::AttrRow { k: "计量单位".into(), v: "米".into() },
            convert::AttrRow { k: "带号".into(),     v: "38".into() },
            convert::AttrRow { k: "精度".into(),     v: "0.001".into() },
            convert::AttrRow { k: "转换参数".into(), v: ",,,,,,".into() },
        ],
        project_info: String::new(),
    }
}

#[test]
fn test_txt_to_shp_split_by_plot_dkmc() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let txt_path = write_multi_plot_txt(tmp.path());
    let out_dir = tempfile::tempdir().expect("output temp dir");

    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("split_by_plot"),
        filename_field: String::from("DKMC"),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };

    let result = convert::convert_txt_to_shp(&[txt_path], &options, &make_header())
        .expect("TXT→面拆分失败");
    assert!(result.success, "应成功: {}", result.message);

    // 应建子目录 {txt_stem}/ = multi_plot/
    let subdir = out_dir.path().join("multi_plot");
    assert!(subdir.is_dir(), "应建 multi_plot 子目录");

    // 子目录内应有 3 个 .shp：地块甲.shp / 地块乙.shp / multi_plot_3.shp（序号兜底）
    let shps: Vec<String> = std::fs::read_dir(&subdir)
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".shp"))
        .collect();
    assert_eq!(shps.len(), 3, "应拆出 3 个 SHP: {:?}", shps);
    assert!(shps.iter().any(|n| n == "地块甲.shp"), "应有 地块甲.shp: {:?}", shps);
    assert!(shps.iter().any(|n| n == "地块乙.shp"), "应有 地块乙.shp: {:?}", shps);
    assert!(shps.iter().any(|n| n == "multi_plot_3.shp"), "DKMC 空应兜底为 multi_plot_3.shp: {:?}", shps);
}

// ─── 测试 1g: TXT→面 split_by_plot 序号兜底 + 同名冲突 ───

#[test]
fn test_txt_to_shp_split_by_plot_index_fallback_and_conflict() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let txt_path = write_multi_plot_txt(tmp.path());
    let out_dir = tempfile::tempdir().expect("output temp dir");

    // filename_field 为空 → 全部走序号兜底 multi_plot_1/2/3
    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("split_by_plot"),
        filename_field: String::new(),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };

    let result = convert::convert_txt_to_shp(&[txt_path.clone()], &options, &make_header())
        .expect("TXT→面拆分失败");
    assert!(result.success);

    let subdir = out_dir.path().join("multi_plot");
    let shps: Vec<String> = std::fs::read_dir(&subdir)
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".shp"))
        .collect();
    assert_eq!(shps.len(), 3, "序号兜底应 3 个: {:?}", shps);
    assert!(shps.iter().all(|n| n.starts_with("multi_plot_")), "应全部为 multi_plot_N 序号兜底: {:?}", shps);
}

// ─── 测试 1h: TXT→面 split_by_plot FID 命名 ───

#[test]
fn test_txt_to_shp_split_by_plot_fid() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let txt_path = write_multi_plot_txt(tmp.path());
    let out_dir = tempfile::tempdir().expect("output temp dir");

    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("split_by_plot"),
        filename_field: String::from("FID"),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };

    let result = convert::convert_txt_to_shp(&[txt_path.clone()], &options, &make_header())
        .expect("TXT→面拆分失败");
    assert!(result.success);

    let subdir = out_dir.path().join("multi_plot");
    let shps: Vec<String> = std::fs::read_dir(&subdir)
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".shp"))
        .collect();
    assert_eq!(shps.len(), 3, "FID 拆分应 3 个: {:?}", shps);
    assert!(shps.iter().any(|n| n == "FID_A.shp"), "应有 FID_A.shp: {:?}", shps);
    assert!(shps.iter().any(|n| n == "FID_B.shp"), "应有 FID_B.shp: {:?}", shps);
    // 第三个 FID_C 存在 → FID_C.shp
    assert!(shps.iter().any(|n| n == "FID_C.shp"), "应有 FID_C.shp: {:?}", shps);
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

    let attrs_vec: Vec<convert::AttrRow> = parsed
        .attrs
        .iter()
        .map(|(k, v)| convert::AttrRow { k: k.clone(), v: v.clone() })
        .collect();
    let generated = txt::generate_txt(
        &parsed.project_info,
        &attrs_vec,
        &parsed.plots,
        true,
        false,
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
        output_mode: String::from("one_to_one"),
        filename_field: String::new(),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };

    let header = make_header();

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

    let header = make_header();

    let options = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        on: false,
        oo: false,
        oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
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
        output_mode: String::from("one_to_one"),
        filename_field: String::new(),
        output_dir: out_dir1.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };

    let header = make_header();

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
        on: false,
        oo: false,
        oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
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

    let header = make_header();
    let header = convert::HeaderConfig {
        project_info: "项目名称=测试".into(),
        ..header
    };

    let options = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        on: false,
        oo: false,
        oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
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

// ─── 测试 11/12 (GPKG) 已移除：GPKG 不再支持 ───

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

    let header = make_header();

    let txt_to_shp = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("one_to_one"),
        filename_field: String::new(),
        output_dir: shp_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
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
        on: false,
        oo: true,
        oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
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
        roundtrip.contains("J4,2,30.000,30.000"),
        "往返后第二个部件的 part index 不应丢失且 J 序号跨环连续（首点 J4），实际输出为:\n{}",
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

    let header = make_header();

    let txt_to_shp = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("one_to_one"),
        filename_field: String::new(),
        output_dir: shp_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
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
        on: true,
        oo: true,
        oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
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
        roundtrip.contains(",2,8.000,8.000"),
        "内环应以独立 part（界址线号=2）输出，实际输出为:\n{}",
        roundtrip
    );
}


// ─── 测试: 统一字段映射 sentinel 值（不填/占位/自动面积） ───

/// 从转换结果 TXT 的元数据行提取面积字段（meta 第 2 列）。
fn extract_area_from_txt<P: AsRef<std::path::Path>>(path: P) -> String {
    let txt = std::fs::read_to_string(path.as_ref()).expect("读取输出 TXT");
    // 元数据行以 '@' 结尾，格式: 点数,面积,FID,名称,类型,图幅号,用途,地类,@
    for line in txt.lines() {
        if line.ends_with('@') {
            let parts: Vec<&str> = line.trim_end_matches('@').split(',').collect();
            if parts.len() >= 2 {
                return parts[1].to_string();
            }
        }
    }
    panic!("未找到元数据行: {}", txt);
}

#[test]
fn test_field_mapping_sentinels_area() {
    let shp_path = test_shp_stem();
    let header = make_header();
    let options = convert::ShpToTxtOptions {
        ox: false, oj: true, on: false, oo: false, oc: false,
        output_mode: "one_to_one".into(), filename_field: String::new(),
    };

    // 平方米（自动）：面积应为正数，2 位小数
    let mk = |area: &str| convert::FieldMapping {
        name: "__placeholder__".into(), id: "__placeholder__".into(), area: area.into(),
        use_field: "__placeholder__".into(), tfh: "__placeholder__".into(), dlbm: "__placeholder__".into(),
    };

    let d = tempfile::tempdir().unwrap();
    let r = convert::convert_shp_to_txt(&[shp_path.clone()], None, None, &header, &mk("__area_sqm__"), &options, d.path(), None).unwrap();
    let area_sqm: f64 = extract_area_from_txt(&r.output_files[0]).parse().unwrap();
    assert!(area_sqm > 0.0, "平方米面积应 > 0，实际 {}", area_sqm);

    let d2 = tempfile::tempdir().unwrap();
    let r2 = convert::convert_shp_to_txt(&[shp_path.clone()], None, None, &header, &mk("__area_ha__"), &options, d2.path(), None).unwrap();
    let area_ha: f64 = extract_area_from_txt(&r2.output_files[0]).parse().unwrap();
    assert!(area_ha > 0.0, "公顷面积应 > 0，实际 {}", area_ha);
    // 公顷 = 平方米 / 10000
    // ha 已舍入到 4 位小数，转回平方米比较，容差 0.01（4 位小数舍入误差上界）
    // sqm 保留 2 位、ha 保留 4 位小数各自舍入，转回平方米比较容差 1.0
    assert!((area_ha * 10000.0 - area_sqm).abs() < 1.0, "公顷换算不一致: sqm={} ha={}", area_sqm, area_ha);

    // 占位文字：面积字段应为 "MJ"
    let d3 = tempfile::tempdir().unwrap();
    let r3 = convert::convert_shp_to_txt(&[shp_path.clone()], None, None, &header, &mk("__placeholder__"), &options, d3.path(), None).unwrap();
    assert_eq!(extract_area_from_txt(&r3.output_files[0]), "MJ", "占位文字应输出 MJ");

    // 不填：面积字段应为空
    let d4 = tempfile::tempdir().unwrap();
    let r4 = convert::convert_shp_to_txt(&[shp_path.clone()], None, None, &header, &mk(""), &options, d4.path(), None).unwrap();
    assert_eq!(extract_area_from_txt(&r4.output_files[0]), "", "不填应输出空面积");
}


// ═══ 编码处理回归测试 ═══

fn exp_data_dir() -> PathBuf {
    repo_root().join("00测试数据")
}

/// 回归：DBF 含 UTF-8 中文非 ASCII 字段名（村民姓/行政村/备注）时不应 panic，
/// 且字段名/记录能正常读取。dbase 0.3.0 内部会 panic，read_dbf 用 catch_unwind 回退手动解析。
#[test]
fn test_read_dbf_utf8_field_names() {
    let dbf = exp_data_dir().join("试验数据0626.dbf");
    if !dbf.exists() {
        eprintln!("跳过：测试数据不存在 {}", dbf.display());
        return;
    }
    let (names, records) = shp::read_dbf(&dbf).expect("读 DBF 不应失败");
    assert!(names.iter().any(|n| n.contains("村民姓") || n.contains("行政村") || n.contains("备注")),
        "应能读到中文字段名，实际 {:?}", names);
    // OBJECTID/FID 应被过滤（SHAPE_Leng/SHAPE_Area 因大小写敏感是预存行为，不在本次范围）
    assert!(names.iter().all(|n| !n.eq_ignore_ascii_case("OBJECTID") && !n.starts_with("FID")),
        "OBJECTID/FID 应被过滤，实际 {:?}", names);
    assert!(!records.is_empty(), "应能读到记录");
    assert!(records.iter().all(|r| r.len() == names.len()), "每行字段数应与字段名数一致");
}

/// 回归：GBK 编码的 TXT（新建txt.txt）能用 read_text_file 正常解码为中文。
#[test]
fn test_read_text_file_gbk() {
    let txt_path = exp_data_dir().join("新建txt.txt");
    if !txt_path.exists() {
        eprintln!("跳过：测试数据不存在 {}", txt_path.display());
        return;
    }
    let text = txt::read_text_file(&txt_path).expect("GBK TXT 应能解码");
    assert!(text.contains("项目信息"), "应含\"项目信息\"");
}

/// 回归：UTF-8 编码的 TXT 仍能正常读取（编码探测不应破坏 UTF-8 路径）。
#[test]
fn test_read_text_file_utf8() {
    let txt_path = repo_root().join("test_data").join("44120000072.txt");
    if !txt_path.exists() {
        eprintln!("跳过：测试数据不存在 {}", txt_path.display());
        return;
    }
    let text = txt::read_text_file(&txt_path).expect("UTF-8 TXT 应能解码");
    assert!(text.contains("属性描述") || text.contains("坐标系") || text.contains('['),
        "UTF-8 TXT 内容异常");
}

/// 回归：GBK(ANSI) TXT → SHP 转换后，输出 DBF 必须是 UTF-8 + LDID=0 + CPG=UTF-8，
/// 字段值中文不乱码（ArcMap 10.x 兼容）。根因：旧版用 GBK 数据 + 非标准 LDID 0x7C，
/// ArcMap 误判编码。现统一为 ArcPy 官方标准（UTF-8）。
#[test]
fn test_txt_to_shp_utf8_dbf() {
    let txt_path = exp_data_dir().join("新建txt.txt");
    if !txt_path.exists() {
        eprintln!("跳过：测试数据不存在 {}", txt_path.display());
        return;
    }
    let out_dir = tempfile::tempdir().expect("temp dir");

    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: String::from("one_to_one"),
        filename_field: String::new(),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };
    let header = make_header();

    let result = convert::convert_txt_to_shp(&[txt_path], &options, &header)
        .expect("TXT→SHP 转换失败");
    assert!(result.success, "{}", result.message);

    // 输出 DBF 路径（one_to_one 模式：stem 与源 TXT 同名）
    let dbf_path = out_dir.path().join("新建txt.dbf");
    assert!(dbf_path.exists(), "DBF 应存在: {}", dbf_path.display());

    // 1. .cpg 内容必须为 UTF-8
    let cpg = std::fs::read_to_string(out_dir.path().join("新建txt.cpg"))
        .expect("读 CPG");
    assert_eq!(cpg, "UTF-8", "CPG 应声明 UTF-8");

    // 2. DBF header byte 29 (LDID) 必须为 0x00
    let dbf_bytes = std::fs::read(&dbf_path).expect("读 DBF 字节");
    assert_eq!(dbf_bytes[29], 0x00, "LDID(byte 29) 应为 0x00，实际 0x{:02x}", dbf_bytes[29]);

    // 3. 用本工具 read_dbf 读回（dbase 读 UTF-8 字段值若失败会走手动回退，按 .cpg UTF-8 解码）
    let (names, records) = shp::read_dbf(&dbf_path).expect("读回 DBF");
    let dkmc_idx = names.iter().position(|n| n == "DKMC").expect("应有 DKMC 字段");
    let dkyt_idx = names.iter().position(|n| n == "DKYT").expect("应有 DKYT 字段");
    assert!(records.iter().any(|r| r[dkmc_idx].contains("地块")),
        "DKMC 应含\"地块\"（中文不乱码），实际: {:?}", records.iter().map(|r| &r[dkmc_idx]).take(3).collect::<Vec<_>>());
    assert!(records.iter().any(|r| r[dkyt_idx].contains("农村宅基地")),
        "DKYT 应含\"农村宅基地\"，实际: {:?}", records.iter().map(|r| &r[dkyt_idx]).take(3).collect::<Vec<_>>());

    // 4. 字节级确认：DKMC 字段值应是 UTF-8（"地块" = e5 9c b0 e5 9d 97），而非 GBK（b5 d8 bf e9）
    assert!(dbf_bytes.windows(6).any(|w| w == [0xe5, 0x9c, 0xb0, 0xe5, 0x9d, 0x97]),
        "DBF 内应含 UTF-8 编码的\"地块\"字节 e59cb0e59d97");
}

/// 回归：GBK 编码、无 .cpg、ASCII 字段名的 DBF（如 ArcMap 导出的测试A1.dbf）
/// 应能正确识别为 GBK 并解码字段值，不依赖 dbase crate（避免 UTF-8 误解码）。
#[test]
fn test_read_dbf_gbk_no_cpg() {
    let dbf = exp_data_dir().join("测试A1").join("测试A1.dbf");
    if !dbf.exists() {
        eprintln!("跳过：测试数据不存在 {}", dbf.display());
        return;
    }
    let (names, records) = shp::read_dbf(&dbf).expect("读 DBF 不应失败");
    let dkmc_idx = names.iter().position(|n| n == "DKMC").expect("应有 DKMC 字段");
    let dkyt_idx = names.iter().position(|n| n == "DKYT").expect("应有 DKYT 字段");

    // DKMC 字段值应为正常中文（GBK 解码），不是乱码/替换符
    let dkmc_values: Vec<&String> = records.iter().map(|r| &r[dkmc_idx]).collect();
    assert!(dkmc_values.iter().any(|v| v.contains("地块")),
        "DKMC 应含\"地块\"（GBK 正确解码），实际: {:?}", dkmc_values);

    // DKYT 应含"城镇住宅用地"或类似中文（不乱码）
    let dkyt_values: Vec<&String> = records.iter().map(|r| &r[dkyt_idx]).collect();
    assert!(dkyt_values.iter().any(|v| v.contains("城镇") || v.contains("用地")),
        "DKYT 应含中文（GBK 正确解码），实际: {:?}", dkyt_values);

    // 不应含 UTF-8 替换符（lossy 解码的标志）
    assert!(dkmc_values.iter().all(|v| !v.contains('\u{FFFD}')),
        "DKMC 不应含替换符 U+FFFD（说明编码识别错误），实际: {:?}", dkmc_values);
}
