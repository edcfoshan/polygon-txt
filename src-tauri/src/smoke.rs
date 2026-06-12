use crate::convert::{FieldMapping, HeaderConfig, ShpToTxtOptions, TxtToShpOptions};
use crate::{convert, gpkg, txt, SmokeTestConfig};
use std::fs;
use std::path::PathBuf;

pub fn run_release_smoke(config: SmokeTestConfig) -> Result<String, String> {
    if !config.txt_path.exists() {
        return Err(format!("Smoke TXT 不存在: {}", config.txt_path.display()));
    }

    fs::create_dir_all(&config.output_dir)
        .map_err(|e| format!("创建 smoke 输出目录失败: {}", e))?;

    let text = fs::read_to_string(&config.txt_path)
        .map_err(|e| format!("读取 smoke TXT 失败: {}", e))?;
    let parsed = txt::parse_txt(&text);
    if parsed.plots.is_empty() {
        return Err("Smoke TXT 中没有地块".to_string());
    }

    let header_cfg = smoke_header_from_txt(&parsed);
    let txt_options = TxtToShpOptions {
        output_shp: false,
        output_gpkg: true,
        merge: false,
        output_dir: config.output_dir.to_string_lossy().to_string(),
    };

    let txt_result = convert::convert_txt_to_shp(
        &[config.txt_path.clone()],
        &txt_options,
        &header_cfg,
    )
    .map_err(|e| format!("smoke TXT->GPKG 失败: {}", e))?;

    let gpkg_path = txt_result
        .output_files
        .iter()
        .find(|f| f.to_lowercase().ends_with(".gpkg"))
        .map(PathBuf::from)
        .ok_or_else(|| "smoke 未生成 GPKG".to_string())?;

    let gpkg_info = gpkg::read_gpkg(&gpkg_path)
        .map_err(|e| format!("smoke 读取 GPKG 失败: {}", e))?;

    let preview = convert::shp_to_txt_preview(
        &[],
        Some("gpkg"),
        Some(&gpkg_path),
        &header_cfg,
        &smoke_field_mapping(),
        &ShpToTxtOptions {
            ox: false,
            oj: true,
            op: false,
            on: false,
            oo: false,
            om: false,
            buffer: 0.0,
        },
        None,
    )
    .map_err(|e| format!("smoke GPKG->TXT 预览失败: {}", e))?;

    let preview_path = config.output_dir.join(
        config
            .txt_path
            .file_stem()
            .map(|s| format!("{}_preview.txt", s.to_string_lossy()))
            .unwrap_or_else(|| "smoke_preview.txt".to_string()),
    );
    fs::write(&preview_path, &preview)
        .map_err(|e| format!("写入 smoke 预览失败: {}", e))?;

    let report = format!(
        "SMOKE_OK txt_plots={} gpkg_layers={} gpkg_features={} gpkg={} preview={}",
        parsed.plots.len(),
        gpkg_info.layers.len(),
        gpkg_info.layers.iter().map(|l| l.num_features).sum::<usize>(),
        gpkg_path.display(),
        preview_path.display()
    );

    let report_path = config.output_dir.join("release_smoke_report.txt");
    fs::write(&report_path, &report)
        .map_err(|e| format!("写入 smoke 报告失败: {}", e))?;

    Ok(report)
}

fn smoke_header_from_txt(parsed: &txt::TxtParseResult) -> HeaderConfig {
    HeaderConfig {
        crs: parsed
            .attrs
            .get("坐标系")
            .cloned()
            .unwrap_or_else(|| "2000国家大地坐标系".to_string()),
        band: parsed
            .attrs
            .get("几度分带")
            .cloned()
            .unwrap_or_else(|| "3".to_string()),
        proj: parsed
            .attrs
            .get("投影类型")
            .cloned()
            .unwrap_or_else(|| "高斯克吕格".to_string()),
        unit: parsed
            .attrs
            .get("计量单位")
            .cloned()
            .unwrap_or_else(|| "米".to_string()),
        zone: parsed
            .attrs
            .get("带号")
            .cloned()
            .unwrap_or_else(|| "38".to_string()),
        precision: parsed
            .attrs
            .get("精度")
            .cloned()
            .unwrap_or_else(|| "0.001".to_string()),
        transform: parsed
            .attrs
            .get("转换参数")
            .cloned()
            .unwrap_or_else(|| ",,,,,,".to_string()),
        project_info: parsed.project_info.clone(),
    }
}

fn smoke_field_mapping() -> FieldMapping {
    FieldMapping {
        name: "DKMC".to_string(),
        id: "DKBH".to_string(),
        area: "MJ".to_string(),
        use_field: "DKYT".to_string(),
        tfh: "TFH".to_string(),
        dlbm: "DLBM".to_string(),
    }
}
