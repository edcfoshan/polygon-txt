// 右栏「地图」页签：Leaflet + 天地图影像 WMTS，画全部地块边界，与属性表双向联动高亮
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';

// 天地图浏览器端 key：首个为主用 key；瓦片持续报错时自动轮换下一个重试（每 key 一轮机会）。
// 注意：key 类型/域名白名单收窄会整批 403（DataServer code 301012），轮换全败才降级。
const TKS = [
  'cf54b743f5ea84975144b8d2fc4a67bb',
  'f39c33458191a4ffd0d5469786b85803',
  '4267820f43926eaf808d61dc07269beb',
  'b5b47b269dea9014fc1ebae419a7e70c',
  '6adf4d6217eca1707c3e04847bc46cf7',
  '7e09c8628c9d7c4203f64bf95f9599f7',
];

const STYLE_NORMAL = { color: '#2f7fd1', weight: 1.5, opacity: 0.9, fillColor: '#2f7fd1', fillOpacity: 0.08 };
const STYLE_DIM = { color: '#9aa0a6', weight: 1, opacity: 0.35, fillColor: '#9aa0a6', fillOpacity: 0.02 };
const STYLE_HL = { color: '#e0453a', weight: 3, opacity: 1, fillColor: '#e0453a', fillOpacity: 0.12 };

const state = {
  inited: false,
  map: null,
  svgRenderer: null,
  layers: new Map(),   // uid → L.polygon
  hlLayer: null,
  hlTimer: null,
  data: null,
  filterUids: null,    // Set('si:pi') | null（null = 不筛/条件有误 → 全量正常色）
  offline: false,
  tileLayers: [],
  tileErrTimes: [],
  tkIdx: 0,
  rotated: false,
  fitted: true,        // true=当前数据已完成首次定位；'pending'=待容器尺寸就绪后再 fit 一次
};

function $(id) { return document.getElementById(id); }

function ensureMap() {
  if (state.inited) return;
  const box = $('mapDiv');
  if (!box) return;
  state.inited = true;
  // attributionControl=false：默认署名带 leafletjs.com 外链，应用内点击会把整个 WebView 导航走；
  // 手动加一个无前缀链接的署名控件，仅保留灰色「© 天地图」纯文字
  state.map = L.map(box, { preferCanvas: true, zoomControl: true, attributionControl: false });
  L.control.attribution({ prefix: false }).addTo(state.map);
  state.svgRenderer = L.svg({ padding: 0.5 });
  addTiles();
  state.map.on('resize', () => state.map.invalidateSize());
}

function addTiles() {
  const tk = TKS[state.tkIdx];
  const opts = { subdomains: '01234567', maxZoom: 18 };
  const img = L.tileLayer(`https://t{s}.tianditu.gov.cn/DataServer?T=img_w&x={x}&y={y}&l={z}&tk=${tk}`, opts);
  const cia = L.tileLayer(`https://t{s}.tianditu.gov.cn/DataServer?T=cia_w&x={x}&y={y}&l={z}&tk=${tk}`, { ...opts, pane: 'overlayPane', interactive: false, attribution: '© 天地图' });
  img.addTo(state.map);
  cia.addTo(state.map);
  state.tileLayers = [img, cia];
  state.tileErrTimes = [];
  img.on('tileerror', onTileError);
  cia.on('tileerror', onTileError);
  img.on('tileload', onTileLoad);
}

function onTileError() {
  if (state.offline) return;
  const now = Date.now();
  state.tileErrTimes.push(now);
  state.tileErrTimes = state.tileErrTimes.filter((t) => now - t < 8000);
  if (state.tileErrTimes.length >= 3) rotateOrDegrade();
}

function rotateOrDegrade() {
  if (state.tkIdx < TKS.length - 1) {
    state.tkIdx++;
    state.rotated = true;
    for (const l of state.tileLayers) state.map.removeLayer(l);
    state.tileLayers = [];
    state.tileErrTimes = [];
    addTiles();
    note(`当前天地图 key 不可用，已切换备用 key（${state.tkIdx + 1}/${TKS.length}）`);
  } else {
    degrade();
  }
}

function onTileLoad() {
  // 成功加载：清错误窗口；轮换成功清提示；降级后成功则恢复瓦片层
  state.tileErrTimes = [];
  if (state.rotated) {
    state.rotated = false;
    note('');
  }
  if (state.offline) {
    state.offline = false;
    if (!state.tileLayers.length) addTiles();
  }
}

function degrade() {
  state.offline = true;
  for (const l of state.tileLayers) state.map.removeLayer(l);
  state.tileLayers = [];
  note('天地图瓦片加载失败（key 均不可用或网络不可达），已切换灰底 + 纯地块轮廓');
}

function note(text) {
  const el = $('mapNote');
  if (!el) return;
  if (!text) { el.style.display = 'none'; return; }
  el.textContent = text;
  el.style.display = 'block';
}

function ringToLatLngs(ring) {
  // 后端 [lon, lat] → Leaflet [lat, lng]
  return ring.map((p) => [p[1], p[0]]);
}

function rebuild() {
  if (!state.inited || !state.map) return;
  for (const l of state.layers.values()) state.map.removeLayer(l);
  state.layers.clear();
  clearHighlight();
  const geo = state.data;
  if (!geo || !geo.plots) { note(''); fitAll(); return; }
  const degradedSrc = new Set((geo.sources || []).map((s, i) => (s.degraded ? i : -1)).filter((i) => i >= 0));
  let drawn = 0;
  for (const p of geo.plots) {
    if (degradedSrc.has(p.si) || !p.rings || !p.rings.length) continue;
    const latlngs = p.rings.map(ringToLatLngs);
    const layer = L.polygon(latlngs, { ...STYLE_NORMAL });
    layer.bindTooltip(`${(geo.sources[p.si]?.name || '')} · #${p.pi + 1}`, { sticky: true });
    layer.on('click', () => {
      if (window.TV) TV.setUid(`${p.si}:${p.pi}`, 'map');
    });
    layer.addTo(state.map);
    state.layers.set(`${p.si}:${p.pi}`, layer);
    drawn++;
  }
  if (drawn === 0) note('所选数据源缺少投影信息（无带号前缀且无 PRJ），无法上图；属性表仍可用');
  else note('');
  applyFilterStyle();
  fitAll();
  state.fitted = 'pending';
}

function fitAll() {
  if (!state.map) return;
  const corners = [];
  for (const l of state.layers.values()) {
    const b = l.getBounds();
    corners.push(b.getNorthWest(), b.getSouthEast());
  }
  if (corners.length) {
    state.map.fitBounds(L.latLngBounds(corners).pad(0.15));
  } else {
    state.map.setView([34.3, 114.3], 5); // 全国概览兜底
  }
}

function applyFilterStyle() {
  for (const [uid, layer] of state.layers) {
    const hit = !state.filterUids || state.filterUids.has(uid);
    layer.setStyle(hit ? STYLE_NORMAL : STYLE_DIM);
  }
}

function clearHighlight() {
  if (state.hlLayer) {
    state.map.removeLayer(state.hlLayer);
    state.hlLayer = null;
  }
  clearTimeout(state.hlTimer);
}

export const MV = {
  // 收到 read_plot_table_geo 数据（全量）；地图未初始化时仅缓存，首次打开页签再画。
  // 新数据到来即标记待定位：下次地图可见且尺寸就绪时自动 fit 到数据范围
  setData(geo) {
    state.data = geo;
    state.fitted = 'pending';
    if (state.inited) rebuild();
  },

  reset() {
    state.data = null;
    state.filterUids = null;
    state.fitted = true;
    if (state.inited) rebuild();
  },

  setFilter(uids) {
    state.filterUids = uids ? new Set(uids.map(([si, pi]) => `${si}:${pi}`)) : null;
    if (state.inited) applyFilterStyle();
  },

  // 表格行 → 地图：飞行定位 + 红色高亮闪烁
  highlight(uid) {
    if (!state.inited) return;
    const layer = state.layers.get(uid);
    clearHighlight();
    if (!layer) return;
    const bounds = layer.getBounds();
    state.map.flyToBounds(bounds, { padding: [40, 40], maxZoom: 18, duration: 0.5 });
    state.hlLayer = L.polygon(layer.getLatLngs(), { ...STYLE_HL, renderer: state.svgRenderer, interactive: false, className: 'plot-blink' });
    state.hlLayer.addTo(state.map);
    // 闪烁 3 次后移除动画类，保留红色描边表示当前选中
    state.hlTimer = setTimeout(() => {
      if (state.hlLayer?.getElement()) state.hlLayer.getElement().classList.remove('plot-blink');
    }, 1600);
  },

  // 首次切到地图页签 / 窗口变化时调用
  ensureInit() {
    ensureMap();
    if (!state.map) return;
    if (state.data && !state.layers.size) rebuild();
    setTimeout(() => {
      if (!state.map) return;
      state.map.invalidateSize();
      // 容器尺寸就绪后兜底补一次定位（rebuild 时若面板刚显示，fitBounds 可能按 0 尺寸计算）
      if (state.fitted === 'pending') {
        fitAll();
        state.fitted = true;
      }
      // 已有选中行（表格里点的）→ 打开地图时补高亮
      const sel = window.TV?.getSelectedUid?.();
      if (sel) MV.highlight(sel);
    }, 60);
  },

  invalidate() {
    if (state.map) setTimeout(() => state.map.invalidateSize(), 60);
  },
};

window.addEventListener('resize', () => MV.invalidate());
