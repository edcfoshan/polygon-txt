// 端到端验证：[属性描述] 段自定义属性行（用 00测试数据 真实文件）
// 注意：本测试引用作者本机的 00测试数据 绝对路径，仅本地可跑（非 CI 测试）。

use jisig_bpoint_converter_lib::convert;
use std::path::PathBuf;

const DATA_DIR: &str = "C:/Users/Administrator/Documents/txt与gdb互转/00测试数据";

/// 模拟用户给出的样例：在固定 7 项之前插入 3 行自定义属性行
fn header_with_custom_attrs() -> convert::HeaderConfig {
    convert::HeaderConfig {
        attrs: vec![
            convert::AttrRow { k: "格式版本号".into(),   v: "1.01版本".into() },
            convert::AttrRow { k: "数据产生单位".into(), v: "有限公司".into() },
            convert::AttrRow { k: "数据产生日期".into(), v: "2025-12-16".into() },
            convert::AttrRow { k: "坐标系".into(),       v: "2000国家大地坐标系".into() },
            convert::AttrRow { k: "几度分带".into(),     v: "3".into() },
            convert::AttrRow { k: "投影类型".into(),     v: "高斯克吕格".into() },
            convert::AttrRow { k: "计量单位".into(),     v: "米".into() },
            convert::AttrRow { k: "带号".into(),         v: "38".into() },
            convert::AttrRow { k: "精度".into(),         v: "0.001".into() },
            convert::AttrRow { k: "转换参数".into(),     v: ",,,,,,".into() },
        ],
        project_info: "项目名称=测试项目\n项目所在县区代码=440000".into(),
    }
}

fn field_mapping() -> convert::FieldMapping {
    convert::FieldMapping {
        name: "DKMC".into(),
        id: "DKBH".into(),
        area: "MJ".into(),
        use_field: "DKYT".into(),
        tfh: "TFH".into(),
        dlbm: "DLBM".into(),
    }
}

#[test]
fn real_shp_to_txt_custom_attr_descriptions() {
    let shp: PathBuf = format!("{}/试验数据0626.shp", DATA_DIR).into();
    assert!(shp.exists(), "测试数据不存在: {:?}", shp);

    let header = header_with_custom_attrs();
    let opts = convert::ShpToTxtOptions {
        ox: false,
        oj: true,
        on: false,
        oo: false,
        oc: false,
        output_mode: "one_to_one".into(),
        filename_field: String::new(), og: false, zone_type: 3,
            proj_no_prefix: false,
            proj_mode: "keep".to_string(),
        proj_zone: None,
};
    let preview = convert::shp_to_txt_preview(&[shp], None, None, &header, &field_mapping(), &opts, None)
        .expect("SHP→TXT 预览失败");

    println!("=== SHP→TXT 预览（前 600 字）===\n{}\n...", &preview[..preview.len().min(600)]);

    // [项目信息] 段输出（非空才输出）
    assert!(preview.contains("[项目信息]"), "应输出项目信息段");
    assert!(preview.contains("项目名称=测试项目"));
    // [属性描述] 段
    assert!(preview.contains("[属性描述]"));
    assert!(preview.contains("格式版本号=1.01版本"));
    assert!(preview.contains("数据产生单位=有限公司"));
    assert!(preview.contains("数据产生日期=2025-12-16"));
    // 顺序：自定义行在标准项之前
    let pos_custom = preview.find("数据产生日期=2025-12-16").unwrap();
    let pos_std = preview.find("坐标系=2000国家大地坐标系").unwrap();
    assert!(pos_custom < pos_std, "自定义行应在标准项之前");
    // [地块坐标] 段
    assert!(preview.contains("[地块坐标]"));
}

#[test]
fn real_txt_to_shp_unaffected_by_attr_descriptions() {
    let txt_path: PathBuf = format!("{}/新建txt.txt", DATA_DIR).into();
    assert!(txt_path.exists(), "测试数据不存在: {:?}", txt_path);

    let out = tempfile::tempdir().expect("temp dir");
    let header = convert::HeaderConfig {
        attrs: vec![
            convert::AttrRow { k: "坐标系".into(), v: "2000国家大地坐标系".into() },
            convert::AttrRow { k: "几度分带".into(), v: "3".into() },
            convert::AttrRow { k: "带号".into(), v: "38".into() },
        ],
        project_info: String::new(),
    };
    let opts = convert::TxtToShpOptions {
        output_shp: true,
        output_mode: "one_to_one".into(),
        filename_field: String::new(),
        output_dir: out.path().to_string_lossy().to_string(),
        keep_lujin: false,
        keep_mingc: false,
    };
    let result = convert::convert_txt_to_shp(&[txt_path], &opts, &header)
        .expect("TXT→SHP 转换失败");
    println!("=== TXT→SHP 结果 === success={} count={} files={:?}",
             result.success, result.processed_count, result.output_files);
    assert!(result.success, "TXT→SHP 应成功");
    assert!(!result.output_files.is_empty(), "应输出 SHP 文件");
}
