// TXT 格式解析与生成模块
use crate::geometry::IndexedRing;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 一个地块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotData {
    pub point_count: u32,
    pub area: String,
    pub fid: String,
    pub name: String,
    pub geom_type: String,
    pub tfh: String,
    pub use_field: String,
    pub dlbm: String,
    pub coords: Vec<(f64, f64)>, // (y, x) = (northing, easting) — as stored in TXT
    #[serde(default)]
    pub rings: Vec<IndexedRing>,
}

/// TXT 解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxtParseResult {
    pub project_info: String,
    pub attrs: HashMap<String, String>,
    pub plots: Vec<PlotData>,
}

/// 解析 TXT 内容
pub fn parse_txt(text: &str) -> TxtParseResult {
    let mut attrs = HashMap::new();
    let mut plots = Vec::new();

    let lines: Vec<&str> = text.lines().collect();
    let mut section = String::new();
    let mut proj_lines: Vec<String> = Vec::new();
    let mut current_plot: Option<PlotData> = None;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "[项目信息]" {
            section = "proj".to_string();
            proj_lines.clear();
            continue;
        }
        if trimmed == "[属性描述]" {
            section = "attr".to_string();
            continue;
        }
        if trimmed == "[地块坐标]" {
            // 如果上一个地块还没 push，先 flush
            if let Some(plot) = current_plot.take() {
                plots.push(plot);
            }
            section = "coord".to_string();
            continue;
        }

        match section.as_str() {
            "proj" => {
                proj_lines.push(trimmed.to_string());
            }
            "attr" => {
                if let Some(eq) = trimmed.find('=') {
                    let key = trimmed[..eq].trim().to_string();
                    let val = trimmed[eq + 1..].trim().to_string();
                    attrs.insert(key, val);
                }
            }
            "coord" => {
                // metadata line: count,area,FID,name,type,tfh,use,dlbm,@
                if trimmed.contains(",@") || trimmed.ends_with('@') {
                    // Flush previous plot
                    if let Some(plot) = current_plot.take() {
                        plots.push(plot);
                    }
                    let meta_line = trimmed.trim_end_matches('@').trim_end_matches(',');
                    let parts: Vec<&str> = meta_line.split(',').collect();
                    let count = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let area = parts.get(1).unwrap_or(&"").to_string();
                    let fid = parts.get(2).unwrap_or(&"").to_string();
                    let name = parts.get(3).unwrap_or(&"").to_string();
                    let geom_type = parts.get(4).unwrap_or(&"面").to_string();
                    let tfh = parts.get(5).unwrap_or(&"").to_string();
                    let use_field = parts.get(6).unwrap_or(&"").to_string();
                    let dlbm = parts.get(7).unwrap_or(&"").to_string();

                    current_plot = Some(PlotData {
                        point_count: count,
                        area,
                        fid,
                        name,
                        geom_type,
                        tfh,
                        use_field,
                        dlbm,
                        coords: Vec::new(),
                        rings: Vec::new(),
                    });
                } else if let Some(ref mut plot) = current_plot {
                    // J1,1,y,x or 1,1,y,x or J1,y,x ...
                    // Skip lines that don't look like coordinates
                    if !trimmed.contains(',') {
                        continue;
                    }
                    let parts: Vec<&str> = trimmed.split(',').collect();
                    // Various formats: J1,1,Y,X or J1,Y,X or 1,Y,X
                    let (part_index, y_str, x_str) = if parts.len() >= 4 {
                        // J1,1,2582988.976,38383243.971
                        (parts[1].parse::<u32>().unwrap_or(1), parts[2], parts[3])
                    } else if parts.len() >= 3 {
                        // J1,2582988.976,38383243.971 or 1,2582988.976,38383243.971
                        (1, parts[1], parts[2])
                    } else {
                        continue;
                    };

                    let y: f64 = y_str.parse().unwrap_or(0.0);
                    let x: f64 = x_str.parse().unwrap_or(0.0);
                    plot.coords.push((y, x));
                    if let Some(last) = plot.rings.last_mut() {
                        if last.part_index == part_index {
                            last.coords.push((y, x));
                        } else {
                            plot.rings.push(IndexedRing {
                                part_index,
                                coords: vec![(y, x)],
                            });
                        }
                    } else {
                        plot.rings.push(IndexedRing {
                            part_index,
                            coords: vec![(y, x)],
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // Flush last
    if let Some(plot) = current_plot.take() {
        plots.push(plot);
    }

    let project_info = proj_lines.join("\n");

    TxtParseResult {
        project_info,
        attrs,
        plots,
    }
}

/// 精度字符串 → 小数位数
fn precision_to_decimals(precision: &str) -> u32 {
    match precision {
        "1" => 0,
        "0.1" => 1,
        "0.01" => 2,
        "0.001" => 3,
        "0.0001" => 4,
        _ => 3,
    }
}

/// 按指定小数位数格式化浮点数
fn format_coord(val: f64, decimals: u32) -> String {
    match decimals {
        0 => format!("{:.0}", val),
        1 => format!("{:.1}", val),
        2 => format!("{:.2}", val),
        3 => format!("{:.3}", val),
        4 => format!("{:.4}", val),
        _ => format!("{:.3}", val),
    }
}

/// 生成 TXT 内容
pub fn generate_txt(
    project_info: &str,
    attrs: &HashMap<String, String>,
    features: &[PlotData],
    oj: bool,
) -> String {
    let mut out = String::new();

    // 从属性中读取精度配置
    let precision_str = attrs.get("精度").map(|s| s.as_str()).unwrap_or("0.001");
    let decimals = precision_to_decimals(precision_str);

    if !project_info.is_empty() {
        out.push_str("[项目信息]\n");
        out.push_str(project_info);
        out.push('\n');
    }

    out.push_str("[属性描述]\n");
    let default_attrs = [
        ("坐标系", "2000国家大地坐标系"),
        ("几度分带", "3"),
        ("投影类型", "高斯克吕格"),
        ("计量单位", "米"),
        ("带号", "38"),
        ("精度", "0.001"),
        ("转换参数", ",,,,,,"),
    ];
    for (k, v) in &default_attrs {
        out.push_str(k);
        out.push('=');
        out.push_str(attrs.get(*k).map(|s| s.as_str()).unwrap_or(v));
        out.push('\n');
    }

    out.push_str("[地块坐标]\n");
    for plot in features {
        let plot_rings = if plot.rings.is_empty() {
            vec![IndexedRing {
                part_index: 1,
                coords: plot.coords.clone(),
            }]
        } else {
            plot.rings.clone()
        };
        let point_count: usize = plot_rings.iter().map(|ring| ring.coords.len()).sum();
        let meta = format!(
            "{},{},{},{},{},{},{},{},@\n",
            point_count,
            plot.area,
            plot.fid,
            plot.name,
            plot.geom_type,
            plot.tfh,
            plot.use_field,
            plot.dlbm,
        );
        out.push_str(&meta);
        for ring in &plot_rings {
            for (i, (y, x)) in ring.coords.iter().enumerate() {
                // 闭合点序号回到 1，保持与原始格式一致
                let seq = if i > 0
                    && i == ring.coords.len() - 1
                    && (y - ring.coords[0].0).abs() < 1e-9
                    && (x - ring.coords[0].1).abs() < 1e-9
                {
                    1
                } else {
                    i + 1
                };
                if oj {
                    out.push_str(&format!(
                        "J{},{},{},{}\n",
                        seq,
                        ring.part_index,
                        format_coord(*y, decimals),
                        format_coord(*x, decimals),
                    ));
                } else {
                    out.push_str(&format!(
                        "{},{},{},{}\n",
                        seq,
                        ring.part_index,
                        format_coord(*y, decimals),
                        format_coord(*x, decimals),
                    ));
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{generate_txt, parse_txt};

    #[test]
    fn multi_part_indices_survive_txt_roundtrip() {
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

        let parsed = parse_txt(text);
        let generated = generate_txt(&parsed.project_info, &parsed.attrs, &parsed.plots, true);

        assert!(
            generated.contains("J1,2,30.000,30.000"),
            "第二个部件的 part index 应被保留，实际输出为:\n{}",
            generated
        );
    }
}
