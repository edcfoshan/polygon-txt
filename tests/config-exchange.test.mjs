import { test } from 'node:test';
import assert from 'node:assert/strict';
import { makePortableConfig, portableToPreset } from '../src/config-exchange.js';

test('双向配置保留高级编辑状态，排除路径和地块选择，重名新增', () => {
  const input = {
    name: '配置甲', direction: 't', header: { attrs: [], project_info: '项目', omit: true },
    fields: { name: 'NAME', id: '', area: '__area_mu__', use_field: '', tfh: '', dlbm: '', area_decimals: 3,
      columns: [], omit_fields: true, omit_coord_section: false,
      adv: { on: false, preset: 'custom', rows: [{ name: '面积', source: '__area_mu__', area_decimals: 5 }] } },
    options: { proj_mode: 'F', proj_zone: 20, zone_type: 6, output_mode: 'merge_all', plot_filter: [[0, 1]], omit_fields: true },
    txtOptions: { output_mode: 'split_by_plot', filename_field: 'DKMC', keep_lujin: true, keep_mingc: false, output_dir: 'C:/private' },
    projectionForm: '6',
  };
  const exported = JSON.parse(JSON.stringify(makePortableConfig(input)));
  assert.equal(exported.active_direction, 'txt-to-shp');
  assert.equal(exported.shp_to_txt.options.plot_filter, undefined);
  assert.equal(exported.txt_to_shp.output_dir, undefined);
  assert.equal(exported.shp_to_txt.fields.adv, undefined);
  const preset = portableToPreset(exported, ['配置甲', '配置甲_2'], 'u1');
  assert.equal(preset.n, '配置甲_3');
  assert.equal(preset.direction, 't');
  assert.deepEqual(preset.f.adv, input.fields.adv);
  assert.equal(preset.t.keep_lujin, true);
  assert.equal(preset.p.proj_zone, 20);
  assert.equal(preset.projection_form, '6');
  assert.equal(preset.f.omit_fields, true);
});
