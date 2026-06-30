// TXT 格式解析与生成模块
use crate::convert::AttrRow;
use crate::geometry::IndexedRing;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// 读取文本文件，按 BOM → UTF-8 → GBK 顺序探测编码解码。
///
/// 替代 `std::fs::read_to_string`（仅支持 UTF-8），兼容中国大陆常见的 GBK 编码 TXT。
/// GBK 解码器（encoding_rs）永不失败，最差情况返回含替换字符的字符串。
pub fn read_text_file<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|e| format!("读取文件失败: {}", e))?;

    // 1. BOM 探测
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec())
            .map_err(|e| format!("UTF-8 BOM 解码失败: {}", e));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok(encoding_rs::UTF_16LE.decode(&bytes[2..]).0.into_owned());
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Ok(encoding_rs::UTF_16BE.decode(&bytes[2..]).0.into_owned());
    }

    // 2. 严格 UTF-8
    if let Ok(s) = std::str::from_utf8(&bytes) {
        return Ok(s.to_string());
    }

    // 3. 回退 GBK
    let (cow, _, had_errors) = encoding_rs::GBK.decode(&bytes);
    if had_errors {
        eprintln!("警告：文件既非 UTF-8 也非完整 GBK: {}", path.display());
    }
    Ok(cow.into_owned())
}

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
    attrs: &[AttrRow],
    features: &[PlotData],
    oj: bool,
    oc: bool,
) -> String {
    let mut out = String::new();

    // 从属性中读取精度配置（"精度"行被删/改名时兜底 0.001）
    let precision_str = attrs
        .iter()
        .find(|r| r.k.trim() == "精度")
        .map(|r| r.v.as_str())
        .unwrap_or("0.001");
    let decimals = precision_to_decimals(precision_str);

    if !project_info.is_empty() {
        out.push_str("[项目信息]\n");
        out.push_str(project_info);
        out.push('\n');
    }

    // [属性描述] 段：按 attrs 列表顺序输出，键值都空的行跳过（非强制）
    out.push_str("[属性描述]\n");
    for row in attrs {
        if row.k.trim().is_empty() && row.v.trim().is_empty() {
            continue;
        }
        out.push_str(&row.k);
        out.push('=');
        out.push_str(&row.v);
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
        // J 界址点序号在单个地块内跨环（外环/洞/多部件）连续递增；
        // 每个地块从 J1 起。闭合点（与首点重合的末点，仅 oo=true 时存在）：
        //   oc=false（回到环首点，默认）→ 序号 = ring_start，不消耗 counter
        //   oc=true （续编）              → 序号 = counter，消耗一个号
        let mut counter: u32 = 1;
        for ring in &plot_rings {
            let ring_start = counter;
            for (i, (y, x)) in ring.coords.iter().enumerate() {
                let is_closing = i > 0
                    && i == ring.coords.len() - 1
                    && (y - ring.coords[0].0).abs() < 1e-9
                    && (x - ring.coords[0].1).abs() < 1e-9;
                let seq = if is_closing && !oc {
                    ring_start
                } else {
                    let s = counter;
                    counter += 1;
                    s
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
    use super::{generate_txt, parse_txt, AttrRow, PlotData};
    use crate::geometry::IndexedRing;

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
        let attrs_vec: Vec<AttrRow> = parsed
            .attrs
            .iter()
            .map(|(k, v)| AttrRow {
                k: k.clone(),
                v: v.clone(),
            })
            .collect();
        let generated = generate_txt(&parsed.project_info, &attrs_vec, &parsed.plots, true, false);

        assert!(
            generated.contains("J4,2,30.000,30.000"),
            "第二个部件的 part index 应被保留且 J 序号跨环连续（首点 J4），实际输出为:\n{}",
            generated
        );
    }

    #[test]
    fn closing_point_continue_mode() {
        // oc=true（续编）：闭合点占下一个连续序号，下一环起始号随之递增。
        // 第一环 3 真实点 + 闭合 → J1,J2,J3,J4(闭合)；第二环 → J5,J6,J7,J8(闭合)。
        let plots = vec![PlotData {
            point_count: 8,
            area: "100".into(),
            fid: "F1".into(),
            name: "续编地块".into(),
            geom_type: "面".into(),
            tfh: "".into(),
            use_field: "".into(),
            dlbm: "".into(),
            coords: vec![],
            rings: vec![
                IndexedRing {
                    part_index: 1,
                    coords: vec![(10.0, 10.0), (10.0, 20.0), (20.0, 20.0), (10.0, 10.0)],
                },
                IndexedRing {
                    part_index: 2,
                    coords: vec![(30.0, 30.0), (30.0, 40.0), (40.0, 40.0), (30.0, 30.0)],
                },
            ],
        }];
        let attrs = vec![AttrRow {
            k: "精度".into(),
            v: "0.001".into(),
        }];
        let out = generate_txt("", &attrs, &plots, true, true);

        assert!(out.contains("J1,1,10.000,10.000"), "首点应 J1:\n{}", out);
        assert!(
            out.contains("J4,1,10.000,10.000"),
            "续编模式第一环闭合点应为 J4:\n{}",
            out
        );
        assert!(
            out.contains("J5,2,30.000,30.000"),
            "续编模式第二环首点应为 J5:\n{}",
            out
        );
        assert!(
            out.contains("J8,2,30.000,30.000"),
            "续编模式第二环闭合点应为 J8:\n{}",
            out
        );
    }

    #[test]
    fn custom_attr_rows_preserve_order_skip_empty_and_precision_fallback() {
        // 用户在固定项之前插入 3 行自定义，末尾留一个空行（应被跳过）
        let attrs = vec![
            AttrRow { k: "格式版本号".into(),   v: "1.01版本".into() },
            AttrRow { k: "数据产生单位".into(), v: "有限公司".into() },
            AttrRow { k: "坐标系".into(),       v: "2000国家大地坐标系".into() },
            AttrRow { k: "几度分带".into(),     v: "3".into() },
            AttrRow { k: "精度".into(),         v: "0.0001".into() },
            AttrRow { k: "".into(),             v: "".into() }, // 空行 → 跳过
        ];
        let plots = vec![PlotData {
            point_count: 1,
            area: "0".into(),
            fid: "FID_0".into(),
            name: "测试".into(),
            geom_type: "面".into(),
            tfh: "".into(),
            use_field: "".into(),
            dlbm: "".into(),
            coords: vec![(10.0, 20.0)],
            rings: vec![],
        }];
        let out = generate_txt("", &attrs, &plots, true, false);

        // 自定义行出现在固定项之前
        let pos_custom = out.find("格式版本号=1.01版本").unwrap();
        let pos_std = out.find("坐标系=2000国家大地坐标系").unwrap();
        assert!(pos_custom < pos_std, "自定义行应在固定项之前:\n{}", out);
        // 精度=4 → 坐标 4 位小数
        assert!(out.contains("J1,1,10.0000,20.0000"), "精度应作用于坐标:\n{}", out);
        // 空行不输出（不出现行首裸 "="）
        assert!(!out.contains("\n=\n"), "空行不应输出:\n{}", out);

        // 精度行被删 → 兜底 0.001（3 位小数）
        let attrs_no_prec = vec![
            AttrRow { k: "坐标系".into(), v: "2000国家大地坐标系".into() },
        ];
        let out2 = generate_txt("", &attrs_no_prec, &plots, true, false);
        assert!(out2.contains("J1,1,10.000,20.000"), "精度行缺失应兜底 0.001:\n{}", out2);
    }
}
