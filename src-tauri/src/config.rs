use crate::{
    convert::{FieldMapping, HeaderConfig, ShpToTxtOptions, TxtToShpOptions},
    txt,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub format: String,
    pub schema_version: u32,
    pub name: String,
    pub active_direction: String,
    pub shp_to_txt: ShpConfig,
    pub txt_to_shp: TxtConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShpConfig {
    pub header: HeaderConfig,
    pub fields: FieldMapping,
    pub options: ShpToTxtOptions,
    pub editor: EditorState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorState {
    pub advanced: bool,
    pub preset: String,
    pub rows: Option<Vec<crate::convert::FieldColumn>>,
    #[serde(default)]
    pub projection_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TxtConfig {
    pub output_mode: String,
    pub filename_field: String,
    pub keep_lujin: bool,
    pub keep_mingc: bool,
}

impl TxtConfig {
    pub fn options(&self, output_dir: String) -> TxtToShpOptions {
        TxtToShpOptions {
            output_shp: true,
            output_mode: self.output_mode.clone(),
            filename_field: self.filename_field.clone(),
            output_dir,
            keep_lujin: self.keep_lujin,
            keep_mingc: self.keep_mingc,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({
            "format": "jisigg-conversion-config", "schema_version": 1,
            "name": "自定义", "active_direction": "shp-to-txt",
            "shp_to_txt": {
                "header": { "attrs": [
                    {"k":"坐标系","v":"2000国家大地坐标系"}, {"k":"几度分带","v":"3"},
                    {"k":"投影类型","v":"高斯克吕格"}, {"k":"计量单位","v":"米"},
                    {"k":"带号","v":""}, {"k":"精度","v":"0.001"}, {"k":"转换参数","v":",,,,,,"}
                ], "project_info": "", "omit": false },
                "fields": {"name":"", "id":"", "area":"__area_ha__", "area_decimals":4,
                    "use_field":"", "tfh":"", "dlbm":"", "columns":[]},
                "options": {"ox":false,"oj":false,"on":false,"oo":true,"oc":false,
                    "output_mode":"one_to_one","filename_field":"","og":false,"zone_type":3,
                    "proj_mode":"keep","proj_zone":null,"proj_no_prefix":false,
                    "point_layout":null,"txt_encoding":"utf8","omit_fields":false,"omit_coord_section":false},
                "editor":{"advanced":false,"preset":"std","rows":null,"projection_form":""}
            },
            "txt_to_shp":{"output_mode":"one_to_one","filename_field":"","keep_lujin":false,"keep_mingc":false}
        })).expect("内置配置结构")
    }
}

fn precision(value: Option<u32>, label: &str, errors: &mut Vec<String>) {
    if value.is_some_and(|v| v > 6) {
        errors.push(format!("{}的小数位必须为 0–6", label));
    }
}

fn columns(rows: &[crate::convert::FieldColumn], errors: &mut Vec<String>) {
    for row in rows {
        precision(row.area_decimals, &row.name, errors);
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("配置 JSON 无效：{}", e))?;
        // serde 的兼容型转换参数会忽略未知项；交换格式必须拒绝拼错的参数。
        let defaults = serde_json::to_value(Self::default()).unwrap();
        for section in ["header", "fields", "options"] {
            if let Some(obj) = value["shp_to_txt"][section].as_object() {
                let allowed = defaults["shp_to_txt"][section].as_object().unwrap();
                for key in obj.keys() {
                    if !allowed.contains_key(key) {
                        return Err(format!(
                            "shp_to_txt.{} 含未知或不可迁移字段：{}",
                            section, key
                        ));
                    }
                }
            }
        }
        fn keys(value: &serde_json::Value, allowed: &[&str], path: &str) -> Result<(), String> {
            if let Some(obj) = value.as_object() {
                for key in obj.keys() {
                    if !allowed.contains(&key.as_str()) {
                        return Err(format!("{} 含未知字段：{}", path, key));
                    }
                }
            }
            Ok(())
        }
        for row in value["shp_to_txt"]["header"]["attrs"]
            .as_array()
            .into_iter()
            .flatten()
        {
            keys(row, &["k", "v"], "header.attrs")?;
        }
        for rows in [
            &value["shp_to_txt"]["fields"]["columns"],
            &value["shp_to_txt"]["editor"]["rows"],
        ] {
            for row in rows.as_array().into_iter().flatten() {
                keys(row, &["name", "source", "area_decimals"], "字段映射行")?;
            }
        }
        let layout = &value["shp_to_txt"]["options"]["point_layout"];
        keys(
            layout,
            &[
                "columns",
                "stake_field",
                "stake_default",
                "distance_unit",
                "distance_decimals",
                "sequence_continuous",
            ],
            "point_layout",
        )?;
        for row in layout["columns"].as_array().into_iter().flatten() {
            keys(
                row,
                &[
                    "kind",
                    "source",
                    "value",
                    "distance_unit",
                    "distance_decimals",
                ],
                "point_layout.columns",
            )?;
        }
        let config: Self =
            serde_json::from_value(value).map_err(|e| format!("配置结构无效：{}", e))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut errors = Vec::new();
        if self.format != "jisigg-conversion-config" {
            errors.push("配置格式标识不正确".into());
        }
        if self.schema_version != 1 {
            errors.push(format!(
                "不支持配置版本 {}，请使用匹配的软件版本",
                self.schema_version
            ));
        }
        if self.name.trim().is_empty() {
            errors.push("配置名称不能为空".into());
        }
        if !["shp-to-txt", "txt-to-shp"].contains(&self.active_direction.as_str()) {
            errors.push("转换方向无效".into());
        }
        let s = &self.shp_to_txt;
        for mode in [&s.options.output_mode, &self.txt_to_shp.output_mode] {
            if !["one_to_one", "split_by_plot", "merge_all"].contains(&mode.as_str()) {
                errors.push(format!("输出模式无效：{}", mode));
            }
        }
        if !["utf8", "gbk"].contains(&s.options.txt_encoding.as_str()) {
            errors.push("TXT 编码必须为 utf8 或 gbk".into());
        }
        if ![3, 6].contains(&s.options.zone_type) {
            errors.push("分带必须为 3 或 6".into());
        }
        if !["keep", "A", "B", "D", "F", "G", "H"].contains(&s.options.proj_mode.as_str()) {
            errors.push("动态投影模式无效".into());
        }
        if s.options.proj_zone.is_some_and(|z| !(1..=60).contains(&z)) {
            errors.push("带号必须为 1–60".into());
        }
        if s.options.plot_filter.is_some() {
            errors.push("配置模板不能包含具体地块筛选".into());
        }
        if s.options.og && s.options.proj_mode != "keep" {
            errors.push("公里网投影与动态投影不可同时启用".into());
        }
        precision(s.fields.area_decimals, "简单面积", &mut errors);
        columns(&s.fields.columns, &mut errors);
        if let Some(rows) = &s.editor.rows {
            columns(rows, &mut errors);
        }
        if s.editor.advanced {
            // FieldColumn 通过 JSON 比较，保留顺序及全部精度设置。
            if serde_json::to_value(&s.editor.rows).unwrap()
                != serde_json::to_value(Some(&s.fields.columns)).unwrap()
            {
                errors.push("高级字段编辑状态与执行字段不一致".into());
            }
        } else if !s.fields.columns.is_empty() {
            errors.push("简单模式不能包含执行中的高级字段列".into());
        }
        if let Some(layout) = &s.options.point_layout {
            validate_layout(layout, &mut errors);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    pub fn check_fields(&self, names: &[String], source: &str) -> Result<(), String> {
        let f = &self.shp_to_txt.fields;
        let o = &self.shp_to_txt.options;
        let mut refs: Vec<&str> = if o.omit_fields {
            vec![]
        } else if self.shp_to_txt.editor.advanced {
            f.columns.iter().map(|c| c.source.as_str()).collect()
        } else {
            vec![&f.name, &f.id, &f.area, &f.use_field, &f.tfh, &f.dlbm]
        };
        if let Some(layout) = &o.point_layout {
            refs.extend(
                layout
                    .columns
                    .iter()
                    .filter(|c| c.kind == "field")
                    .map(|c| c.source.as_str()),
            );
            // 埋桩字段和命名字段具有既有兜底规则，不按缺失字段处理。
        }
        let builtins = [
            "__placeholder__",
            "__area_sqm__",
            "__area_mu__",
            "__area_ha__",
            "__area_km2__",
            "__count__",
            "__geom__",
        ];
        let mut missing: Vec<_> = refs
            .into_iter()
            .filter(|r| !r.is_empty() && !builtins.contains(r) && !names.iter().any(|n| n == r))
            .collect();
        missing.sort();
        missing.dedup();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{} 缺少配置中的源字段：{}",
                source,
                missing.join("、")
            ))
        }
    }
}

fn validate_layout(layout: &txt::PointLayout, errors: &mut Vec<String>) {
    let kinds = [
        "point", "ring", "y", "x", "sequence", "distance", "stake", "field", "fixed",
    ];
    for kind in ["point", "ring", "y", "x"] {
        if layout.columns.iter().filter(|c| c.kind == kind).count() != 1 {
            errors.push(format!("界址点布局必须恰好包含一个 {} 列", kind));
        }
    }
    if !["m", "km", "cm"].contains(&layout.distance_unit.as_str()) {
        errors.push("距离单位无效".into());
    }
    precision(Some(layout.distance_decimals), "距离", errors);
    for c in &layout.columns {
        if !kinds.contains(&c.kind.as_str()) {
            errors.push(format!("未知界址点列类型：{}", c.kind));
        }
        if c.kind == "field" && c.source.trim().is_empty() {
            errors.push("源字段列必须选择字段".into());
        }
        if c.distance_unit
            .as_ref()
            .is_some_and(|u| !["m", "km", "cm"].contains(&u.as_str()))
        {
            errors.push("距离列单位无效".into());
        }
        precision(c.distance_decimals, "距离列", errors);
    }
}
