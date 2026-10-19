// 桌面与 CLI 的稳定配置契约，不包含路径、数据选择或主题。
export function makePortableConfig({ name, direction, header, fields, options, txtOptions, projectionForm = "" }) {
  const { adv, omit_fields, omit_coord_section, ...mapping } = fields;
  const { plot_filter, ...portableOptions } = options;
  return {
    format: "jisigg-conversion-config", schema_version: 1,
    name: name.trim() || "自定义", active_direction: direction === "t" ? "txt-to-shp" : "shp-to-txt",
    shp_to_txt: {
      header, fields: mapping, options: portableOptions,
      editor: { advanced: !!adv?.on, preset: adv?.preset || "std", rows: adv?.rows ?? null, projection_form: projectionForm },
    },
    txt_to_shp: {
      output_mode: txtOptions.output_mode, filename_field: txtOptions.filename_field,
      keep_lujin: !!txtOptions.keep_lujin, keep_mingc: !!txtOptions.keep_mingc,
    },
  };
}

export function portableToPreset(config, existingNames, id) {
  const base = config.name.trim();
  let name = base;
  for (let i = 2; existingNames.includes(name); i++) name = `${base}_${i}`;
  const s = config.shp_to_txt;
  return {
    id, n: name, h: s.header, p: s.options,
    f: { ...s.fields, omit_fields: s.options.omit_fields, omit_coord_section: s.options.omit_coord_section,
      adv: { on: s.editor.advanced, preset: s.editor.preset, rows: s.editor.rows } },
    t: config.txt_to_shp, direction: config.active_direction === "txt-to-shp" ? "t" : "s",
    projection_form: s.editor.projection_form,
  };
}
