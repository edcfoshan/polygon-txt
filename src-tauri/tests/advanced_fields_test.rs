// 字段映射高级模式（补充耕地 12 字段格式）测试
// 数据：00测试数据/20260818修改内容/模板地块坐标信息 (1).txt（真实新格式）
//       test_data/44120000072.txt（旧 8 字段标准格式回归）
//       test_arcpy/std_shp/plot_000.shp（SHP→TXT 高级管线）

extern crate jisig_bpoint_converter_lib;

use jisig_bpoint_converter_lib::{convert, geometry::IndexedRing, shp, txt};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .to_path_buf()
}

/// 真实补充耕地模板（12 字段 + 字段名列表行 + 说明块）
fn template_txt() -> PathBuf {
    repo_root()
        .join("00测试数据")
        .join("20260818修改内容")
        .join("模板地块坐标信息 (1).txt")
}

/// 旧 8 字段标准格式
fn legacy_txt() -> PathBuf {
    repo_root().join("test_data").join("44120000072.txt")
}

fn test_shp_stem() -> PathBuf {
    repo_root().join("test_arcpy").join("std_shp").join("plot_000.shp")
}

const TEMPLATE_FIELDS: [&str; 12] = [
    "坐标点个数",
    "图斑面积",
    "图斑编号",
    "地块名称",
    "补充耕地实施年份",
    "耕地坡度级别",
    "图形属性",
    "图幅号",
    "地块用途",
    "备注",
    "地类",
    "耕地质量等级",
];

fn make_header() -> convert::HeaderConfig {
    convert::HeaderConfig {
        attrs: vec![
            convert::AttrRow { k: "坐标系".into(),   v: "2000国家大地坐标系".into() },
            convert::AttrRow { k: "几度分带".into(), v: "3".into() },
            convert::AttrRow { k: "投影类型".into(), v: "高斯克吕格".into() },
            convert::AttrRow { k: "计量单位".into(), v: "米".into() },
            convert::AttrRow { k: "带号".into(),     v: "39".into() },
            convert::AttrRow { k: "精度".into(),     v: "0.001".into() },
            convert::AttrRow { k: "转换参数".into(), v: ",,,,,,".into() },
        ],
        project_info: String::new(),
    }
}

/// 12 字段补充耕地列配置（前端 BCG_ADV_ROWS 的镜像；源字段按 plot_000.dbf 实际有值的列：
/// DKMC="DKMC"、MJ="0"、Id="0"，DKBH/TFH/DKYT/DLBM 列值为空）
fn bcg_columns() -> Vec<convert::FieldColumn> {
    [
        ("坐标点个数", "__count__"),
        ("图斑面积", "__area_ha__"),
        ("图斑编号", "DKMC"),
        ("地块名称", "DKMC"),
        ("补充耕地实施年份", ""),
        ("耕地坡度级别", ""),
        ("图形属性", "__geom__"),
        ("图幅号", "Id"),
        ("地块用途", "DKYT"),
        ("备注", ""),
        ("地类", "DLBM"),
        ("耕地质量等级", ""),
    ]
    .iter()
    .map(|(n, s)| convert::FieldColumn { name: n.to_string(), source: s.to_string() })
    .collect()
}

fn adv_options() -> convert::ShpToTxtOptions {
    convert::ShpToTxtOptions {
        proj_mode: "keep".to_string(),
        proj_zone: None,
        ox: false, oj: true, on: false, oo: true, oc: false,
        output_mode: "one_to_one".into(), filename_field: String::new(),
        og: false, zone_type: 3, proj_no_prefix: false,
    }
}

// ─── 1. 新格式解析：列表行识别 + 说明块跳过 + 按名解析 + 槽位回填 ───

#[test]
fn test_parse_advanced_template() {
    let text = txt::read_text_file(template_txt()).expect("读模板失败");
    let parsed = txt::parse_txt(&text);

    // 字段名列表行
    assert_eq!(parsed.meta_fields.len(), 12, "应识别 12 个元数据字段");
    for (i, f) in TEMPLATE_FIELDS.iter().enumerate() {
        assert_eq!(&parsed.meta_fields[i], f, "第 {} 个字段名应为 {}", i + 1, f);
    }

    // 9 个地块（图斑1~9）
    assert_eq!(parsed.plots.len(), 9, "模板应含 9 个地块");

    let first = &parsed.plots[0];
    // 首块：图斑1，9 个坐标点
    assert_eq!(first.coords.len(), 9, "首块应 9 个点");
    assert_eq!(first.point_count, 9, "首块声明点数 9");

    // fields 按名配对（备注为空、年份 2022）
    let get = |k: &str| {
        first
            .fields
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("缺少字段 {}", k))
    };
    assert_eq!(get("图斑面积"), "0.2464");
    assert_eq!(get("图斑编号"), "F13102220230001000009");
    assert_eq!(get("地块名称"), "图斑1");
    assert_eq!(get("补充耕地实施年份"), "2022");
    assert_eq!(get("耕地坡度级别"), "1");
    assert_eq!(get("图形属性"), "面");
    assert_eq!(get("图幅号"), "J50G017037");
    assert_eq!(get("地块用途"), "占补平衡");
    assert_eq!(get("备注"), "", "备注为空但保留字段位");
    assert_eq!(get("地类"), "水浇地");
    assert_eq!(get("耕地质量等级"), "3");

    // 6 槽位按语义名回填
    assert_eq!(first.fid, "F13102220230001000009");
    assert_eq!(first.name, "图斑1");
    assert_eq!(first.area, "0.2464");
    assert_eq!(first.geom_type, "面");
    assert_eq!(first.tfh, "J50G017037");
    assert_eq!(first.use_field, "占补平衡");
    assert_eq!(first.dlbm, "水浇地");

    // 说明块跳过的核心断言：无 (0,0) 垃圾坐标点
    // （说明块第 1 行「1、坐标点个数、图斑面积、...」若被误当坐标行会解析出 0,0）
    for plot in &parsed.plots {
        for (y, x) in &plot.coords {
            assert!(
                y.abs() > 1.0 || x.abs() > 1.0,
                "出现疑似说明块泄漏的垃圾点 ({}, {})",
                y, x
            );
        }
    }

    // 图斑3 是多部件+洞（part 1~3），验证 part_index 切环仍正常
    let third = &parsed.plots[2];
    assert!(third.rings.len() >= 3, "图斑3 应有 >=3 个环（多部件+洞）");
}

// ─── 2. 高级格式生成：不输出列表行、元数据行按列序、点数列重算 ───

#[test]
fn test_generate_advanced_meta_no_list_line() {
    let plots = vec![txt::PlotData {
        point_count: 0, // 由 generate 按 rings 重算
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
            coords: vec![(4354939.292, 39436937.125), (4354937.927, 39436938.233), (4354939.292, 39436937.125)],
        }],
        fields: vec![
            ("坐标点个数".to_string(), "0".to_string()), // 占位，应被重算为 3
            ("图斑面积".to_string(), "0.2464".to_string()),
            ("备注".to_string(), String::new()),
        ],
    }];
    let attrs = vec![convert::AttrRow { k: "精度".into(), v: "0.001".into() }];
    let out = txt::generate_txt("", &attrs, &plots, true, false);

    // 不输出字段名列表行（用户需求：接收系统按约定列序解析）
    assert!(!out.contains("【"), "高级格式不应输出字段名列表行:\n{}", out);

    // 元数据行：值按列顺序，坐标点个数列 = 实际 rings 点数（3），空值保留空位
    let meta_line = out
        .lines()
        .find(|l| l.starts_with("3,0.2464,,@"))
        .expect("元数据行应为 3,0.2464,,@");
    assert!(meta_line.ends_with(",@"), "元数据行应以 ,@ 结尾");
}

// ─── 3. 字节级兼容回归：旧格式解析/生成不变 ───

#[test]
fn test_legacy_format_unchanged() {
    let text = txt::read_text_file(legacy_txt()).expect("读旧格式失败");
    let parsed = txt::parse_txt(&text);

    // 无列表行 → fields 恒空、meta_fields 空
    assert!(parsed.meta_fields.is_empty(), "旧格式无字段名列表行");
    assert!(parsed.plots.iter().all(|p| p.fields.is_empty()), "旧格式 fields 应为空");

    // 槽位按位置切分不变
    let p = &parsed.plots[0];
    assert_eq!(p.point_count, 6);
    assert_eq!(p.area, "1.2247");
    assert_eq!(p.fid, "FID_0");
    assert_eq!(p.name, "DKMC");
    assert_eq!(p.geom_type, "面");
    assert_eq!(p.tfh, "TFH");
    assert_eq!(p.use_field, "DKYT");
    assert_eq!(p.dlbm, "DLBM");

    // 生成：fields 空 → 旧 8 字段行，无列表行
    let attrs = vec![convert::AttrRow { k: "精度".into(), v: "0.001".into() }];
    let out = txt::generate_txt("", &attrs, &parsed.plots, false, false);
    assert!(!out.contains("【"), "旧格式输出不应含列表行");
    assert!(out.contains("6,1.2247,FID_0,DKMC,面,TFH,DKYT,DLBM,@"), "旧 8 字段元数据行不变");
}

// ─── 4. 往返：导出值 + 带列表行导入按名解析（导出本身不含列表行） ───

#[test]
fn test_roundtrip_advanced() {
    let text = txt::read_text_file(template_txt()).expect("读模板失败");
    let parsed = txt::parse_txt(&text);
    assert!(!parsed.plots.is_empty());

    // 用解析结果重新生成（导出不含列表行，用户需求）
    let attrs: Vec<convert::AttrRow> = parsed
        .attrs
        .iter()
        .map(|(k, v)| convert::AttrRow { k: k.clone(), v: v.clone() })
        .collect();
    let out = txt::generate_txt(&parsed.project_info, &attrs, &parsed.plots, false, false);
    assert!(!out.contains("【"), "导出不应含列表行");

    // 接收方场景：把列表行插回 [地块坐标] 后（外部文件自带列表行）再导入，
    // 导出的元数据值应能按名解析正确读回
    let with_list = out.replacen(
        "[地块坐标]\n",
        &format!("[地块坐标]\n【{},@】\n", parsed.meta_fields.join(",")),
        1,
    );
    let reparsed = txt::parse_txt(&with_list);

    assert_eq!(reparsed.meta_fields, parsed.meta_fields);
    assert_eq!(reparsed.plots.len(), parsed.plots.len());
    for (a, b) in parsed.plots.iter().zip(reparsed.plots.iter()) {
        // fields 深相等（坐标点个数列重算后与声明值一致）
        assert_eq!(a.fields, b.fields, "fields 应往返一致");
        assert_eq!(a.coords, b.coords, "coords 应往返一致");
        assert_eq!(a.rings.len(), b.rings.len(), "环数应往返一致");
    }
}

// ─── 5. SHP→TXT 高级管线：列表行 + 12 列值 + 预览/导出一致 ───

#[test]
fn test_shp_to_txt_advanced_pipeline() {
    let shp_path = test_shp_stem();
    let header = make_header();
    let field_mapping = convert::FieldMapping {
        name: "DKMC".into(), id: "DKBH".into(), area: "MJ".into(),
        use_field: "DKYT".into(), tfh: "TFH".into(), dlbm: "DLBM".into(),
        columns: bcg_columns(),
    };
    let options = adv_options();

    // 导出
    let out_dir = tempfile::tempdir().expect("temp dir");
    let result = convert::convert_shp_to_txt(
        &[shp_path.clone()], None, None, &header, &field_mapping, &options,
        out_dir.path(), None,
    ).expect("高级模式转换失败");
    assert!(result.success, "{}", result.message);
    let exported = std::fs::read_to_string(out_dir.path().join("plot_000.txt")).expect("读输出");

    // 不输出字段名列表行；12 列元数据行
    assert!(!exported.contains("【"), "不应输出字段名列表行");
    for line in exported.lines() {
        // 找元数据行（以 ,@ 结尾且非【列表行）
        if line.ends_with(",@") && !line.starts_with('【') {
            let parts: Vec<&str> = line.strip_suffix(",@").unwrap().split(',').collect();
            assert_eq!(parts.len(), 12, "元数据行应 12 列: {}", line);
            // 锁定列：图形属性=面；坐标点个数=数值
            assert_eq!(parts[6], "面", "图形属性列应为「面」");
            assert!(parts[0].parse::<u32>().is_ok(), "坐标点个数列应为数值");
            // 源字段映射列值（plot_000.dbf：DKMC 列="DKMC"、Id 列="0"）
            assert_eq!(parts[3], "DKMC", "地块名称列应来自源 DKMC");
            assert_eq!(parts[2], "DKMC", "图斑编号列应来自源 DKMC（重复映射同一源允许）");
            assert_eq!(parts[7], "0", "图幅号列应来自源 Id");
            // 未映射列输出空
            assert_eq!(parts[9], "", "备注列未映射应为空");
            break;
        }
    }

    // 预览一致性：同一参数走 preview，内容应与导出一致（小文件不受 2000 行截断）
    let preview = convert::shp_to_txt_preview(
        &[shp_path], None, None, &header, &field_mapping, &options, None,
    ).expect("预览失败");
    assert_eq!(preview, exported.trim_end(), "预览与导出应逐字一致");
}

// ─── 6. TXT→SHP 分支：FIELDn 动态字段 / 标准格式 6 拼音 + DKBH 修复 ───

#[test]
fn test_txt_to_shp_fieldn_and_standard() {
    // 6a. 12 字段模板 → FIELD1~FIELD12 全字段
    let out_dir = tempfile::tempdir().expect("temp dir");
    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };
    let result = convert::convert_txt_to_shp(&[template_txt()], &options, &make_header())
        .expect("模板转 SHP 失败");
    assert!(result.success, "{}", result.message);
    // 一对一输出一个 SHP 文件组（shp/dbf/prj/cpg 四件套）
    let shp_path = PathBuf::from(
        result
            .output_files
            .iter()
            .find(|f| f.ends_with(".shp"))
            .expect("应输出 .shp"),
    );
    let info = shp::read_shp_file_group(&shp_path).expect("读回 SHP 失败");
    // 字段名：12 个 FIELD（read_dbf 的 dbase Record 迭代顺序随机，文件头顺序由 test_dbf_header_field_order 校验）
    let fieldn = info.field_names.iter().filter(|f| f.starts_with("FIELD")).count();
    assert_eq!(fieldn, 12, "应有 12 个 FIELD 字段: {:?}", info.field_names);
    // 首行值：FIELD1=点数 9、FIELD4=图斑1、FIELD7=面、FIELD10=空（备注）
    assert_eq!(info.num_features, 9, "应 9 个要素");
    let rec = &info.field_records[0];
    let val = |name: &str| -> String {
        let pos = info.field_names.iter().position(|n| n == name).expect(name);
        rec[pos].clone()
    };
    assert_eq!(val("FIELD1"), "9");
    assert_eq!(val("FIELD4"), "图斑1");
    assert_eq!(val("FIELD7"), "面");
    assert_eq!(val("FIELD10"), "");

    // 6b. 标准 8 字段 → 6 拼音键 + DKBH 修复（应填 plot.fid 而非空）
    let out_dir2 = tempfile::tempdir().expect("temp dir 2");
    let options2 = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
        output_dir: out_dir2.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };
    let header38 = convert::HeaderConfig {
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
    };
    let result2 = convert::convert_txt_to_shp(&[legacy_txt()], &options2, &header38)
        .expect("旧格式转 SHP 失败");
    assert!(result2.success, "{}", result2.message);

    let info2 = shp::read_shp_file_group(&PathBuf::from(&result2.output_files[0])).expect("读回 SHP 失败");
    assert!(info2.field_names.iter().any(|f| f == "DKMC"), "标准格式应输出 DKMC: {:?}", info2.field_names);
    assert!(!info2.field_names.iter().any(|f| f.starts_with("FIELD")), "标准格式不应有 FIELD 字段");
    let rec2 = &info2.field_records[0];
    let dkbh = {
        let pos = info2.field_names.iter().position(|n| n == "DKBH").expect("DKBH");
        rec2[pos].clone()
    };
    assert_eq!(dkbh, "FID_0", "DKBH 应填元数据行第 3 列（plot.fid），不再恒空");
}

// DBF 文件头字段顺序校验（read_dbf 的 dbase Record 迭代顺序随机，须直接读二进制头）
#[test]
fn test_dbf_header_field_order() {
    let out_dir = tempfile::tempdir().expect("temp dir");
    let options = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
        output_dir: out_dir.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };
    let result = convert::convert_txt_to_shp(&[template_txt()], &options, &make_header()).unwrap();
    let dbf = result.output_files.iter().find(|f| f.ends_with(".dbf")).unwrap();
    let bytes = std::fs::read(dbf).unwrap();
    // DBF 头：32 字节后每 32 字节一个字段描述符，前 11 字节是字段名
    let header_len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let num_fields = (header_len - 33) / 32;
    let mut names = Vec::new();
    for i in 0..num_fields {
        let off = 32 + i * 32;
        let end = bytes[off..off + 11].iter().position(|&b| b == 0).unwrap_or(11);
        names.push(String::from_utf8_lossy(&bytes[off..off + end]).to_string());
    }
    println!("DBF 头字段顺序 = {:?}", names);
    assert_eq!(names, vec![
        "FIELD1","FIELD2","FIELD3","FIELD4","FIELD5","FIELD6","FIELD7","FIELD8",
        "FIELD9","FIELD10","FIELD11","FIELD12",
    ], "DBF 文件头字段顺序应按元数据行顺序");
}
