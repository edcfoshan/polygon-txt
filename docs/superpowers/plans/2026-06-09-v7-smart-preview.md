# v7 智能识别 + 实时预览 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 prototype_v7.html 中实现三件事：文件导入后字段自动匹配、.prj 空间参考自动识别填充表头、右栏完整 TXT 输出实时预览。

**Architecture:** 所有改动集中在 `prototype_v7.html`（纯 HTML/CSS/JS，无构建步骤）。新增 JS 函数：`parsePrj()` 解析 WKT、`parseDbfFields()` 提取字段名、`parseShpHeader()` 验证 SHP、`autoMatchFields()` 执行字段匹配、`autoFillHeader()` 填充表头、`rebuildPreview()` 完整预览 + 150ms debounce。现有 `af()` 函数从 demo 改为真实文件输入，现有 `up()` 函数从表头骨架预览改为完整 TXT 预览。

**Tech Stack:** 纯浏览器端 JavaScript，无依赖。FileReader API 读取二进制文件，正则解析 WKT/PRJ 文本。

---

### Task 1: 真实文件输入 — 替换 demo `af()`

**Files:**
- Modify: `prototype_v7.html` 中的 `af()` 函数及 drop 区域

- [ ] **Step 1: 修改 drop 区域，支持 SHP 三件套输入**

将 drop 区改为触发 `<input type="file" multiple accept=".shp,.dbf,.prj">`。文件选中后按扩展名分类存储到全局变量 `loadedFiles`。

定位到当前 HTML 中的 drop 区域（`<div class="drop" onclick="af()">`），把 `onclick="af()"` 去掉，改为 JS 绑定。新增全局变量和文件处理：

```javascript
// ═══ File handling ═══
let loadedFiles={}; // {shp:File, dbf:File, prj:File, shx:File}
const FIELD_MATCH_RULES={
  fn:['DKMC','MC','NAME'],      // 地块名称
  fi:['DKBH','BH','ID'],        // 地块编号
  fa:['MJ','AREA'],             // 面积
  fu:['DKYT','YT','YONGTU'],    // 用途
  fm:['TFH'],                   // 图幅号
  fd:['DLBM','DL']              // 地类
};

function initFileInput(){
  const drop=document.querySelector('.drop');
  drop.style.cursor='pointer';
  drop.addEventListener('click',()=>{
    const inp=document.createElement('input');
    inp.type='file';inp.multiple=true;inp.accept='.shp,.dbf,.prj,.shx';
    inp.onchange=handleFileSelect;
    inp.click();
  });
  // drag & drop
  drop.addEventListener('dragover',e=>{e.preventDefault();drop.style.borderColor='var(--ac)';});
  drop.addEventListener('dragleave',()=>{drop.style.borderColor='';});
  drop.addEventListener('drop',e=>{
    e.preventDefault();drop.style.borderColor='';
    handleFiles(e.dataTransfer.files);
  });
}

function handleFileSelect(e){handleFiles(e.target.files)}
function handleFiles(fileList){
  loadedFiles={};
  for(const f of fileList){
    const ext=f.name.split('.').pop().toLowerCase();
    if(['shp','dbf','prj','shx'].includes(ext)) loadedFiles[ext]=f;
  }
  if(!loadedFiles.shp){toast('请选择 .shp 文件');return;}
  renderFileList();
  processImport();
}

function renderFileList(){
  const fl=$('fl');fl.innerHTML='';
  for(const [ext,f] of Object.entries(loadedFiles)){
    const size=(f.size/1024).toFixed(0)+'KB';
    fl.innerHTML+=`<div class="fitem"><span class="fn">◈ ${f.name}</span><span class="fs">${size}</span><button onclick="removeFile('${ext}')">×</button></div>`;
  }
}

function removeFile(ext){
  delete loadedFiles[ext];
  if(!loadedFiles.shp){loadedFiles={};$('fl').innerHTML='';$('pv').textContent='等待文件…';return}
  renderFileList();
  if(loadedFiles.shp) processImport();
}
```

在 `<script>` 标签内的 `init()` 函数末尾追加 `initFileInput();`。

删除旧 demo 函数 `af()` 和 `DF` 变量。

- [ ] **Step 2: 重新加载页面，确认 drop 区点击弹出文件选择器**

打开 `prototype_v7.html`，点击 drop 区 → 应弹出文件选择器；选择 SHP 三件套 → 文件列表显示文件名和大小。

---

### Task 2: .dbf 字段名解析 + SHP 头部验证

**Files:**
- Modify: `prototype_v7.html` 中的新增 JS

- [ ] **Step 1: 实现 `parseDbfFields(file)` — 从 dBase III+ 文件头提取字段名**

```javascript
async function parseDbfFields(file){
  const buf=await file.arrayBuffer();
  const dv=new DataView(buf);
  const fieldCount=(dv.getUint8(4)+dv.getUint8(5)*256-1-32)/32|0;
  const fields=[];
  for(let i=0;i<fieldCount;i++){
    let name='';
    for(let j=0;j<11;j++){const c=dv.getUint8(32+i*32+j);if(c===0)break;name+=String.fromCharCode(c)}
    fields.push(name.trim());
  }
  return fields;
}
```

- [ ] **Step 2: 实现 `parseShpHeader(file)` — 验证 SHP 文件头（文件代码 9994）并读取坐标范围**

```javascript
async function parseShpHeader(file){
  const buf=await file.arrayBuffer();
  const dv=new DataView(buf);
  const code=dv.getInt32(0,false);
  if(code!==9994) throw new Error('不是有效的 SHP 文件');
  // Bounding box (8 doubles = 64 bytes from offset 36)
  const bbox=[];
  for(let i=0;i<8;i++) bbox.push(dv.getFloat64(36+i*8,true));
  return {code,bbox}; // bbox: [xmin,ymin,xmax,ymax,zmin,zmax,mmin,mmax]
}
```

- [ ] **Step 3: 实现 `prjToHeader(prjText)` — 从 WKT 提取表头参数**

```javascript
function prjToHeader(prjText){
  const h={};
  // 坐标系
  if(/CGCS\s*2000|2000.*国家|China.*2000/i.test(prjText)) h.c='2000国家大地坐标系';
  else if(/Xian.*1980|1980.*西安/i.test(prjText)) h.c='1980西安坐标系';
  else if(/Beijing.*1954|1954.*北京/i.test(prjText)) h.c='1954北京坐标系';
  else if(/WGS.*84/i.test(prjText)) h.c='WGS84坐标系';
  else h.c='2000国家大地坐标系';

  // 投影
  if(/Transverse_Mercator|Gauss.*Kruger/i.test(prjText)) h.j='高斯克吕格';
  else if(/Lambert/i.test(prjText)) h.j='兰伯特';
  else if(/Albers/i.test(prjText)) h.j='阿尔伯斯';
  else h.j='高斯克吕格';

  // 分带 — 从中央经线判断
  const cm=prjText.match(/Longitude_Of_Origin["\s,\]]*([\d.]+)/i)||prjText.match(/Central_Meridian["\s,\]]*([\d.]+)/i);
  if(cm){
    const lon=parseFloat(cm[1]);
    const zone=Math.round((lon-1.5)/3);
    h.z=zone<10?'0'+zone:''+zone;
    h.b='3';
  }else{h.z='';h.b='3'}

  // 单位
  if(/UNIT\["Meter/i.test(prjText)) h.u='米';
  else if(/UNIT\["Degree/i.test(prjText)) h.u='度';
  else h.u='米';

  return h;
}
```

- [ ] **Step 4: 用测试文件验证** — 用浏览器打开，导入一个真实 SHP，console.log 打印解析出的字段名和表头参数，确认匹配预期。

---

### Task 3: 字段映射自动匹配 + 表头自动填充

**Files:**
- Modify: `prototype_v7.html` 中的 JS

- [ ] **Step 1: 实现 `autoMatchFields(fieldNames)` — 扫描 + 匹配 + 更新下拉框**

```javascript
function autoMatchFields(fieldNames){
  for(const [key,rules] of Object.entries(FIELD_MATCH_RULES)){
    const sel=$(key);
    if(!sel)continue;
    // 替换 option 列表为文件实际字段
    const currentVal=sel.value; // 保留当前值（可能来自预设）
    sel.innerHTML='';
    // 先找匹配
    let matched='';
    for(const r of rules){
      if(fieldNames.includes(r)){matched=r;break}
    }
    // 构建 option 列表
    if(!matched && rules.length>0 && key!=='fa'){
      // 没匹配到：保留第一个规则作为默认，但 value 为空
      sel.innerHTML='<option value="">默认</option>';
    }else{
      // 有匹配 或 面积字段
      sel.innerHTML='';
    }
    // 填充文件所有字段
    fieldNames.forEach(fn=>{
      const sel2=matched===fn?' selected':'';
      sel.innerHTML+=`<option value="${fn}"${sel2}>${fn}</option>`;
    });
  }
}
```

- [ ] **Step 2: 实现 `autoFillHeader(headerInfo)` — 填充中栏表头并标记"自动识别"**

```javascript
let headerManual={}; // 用户手动修改过的字段集合

function autoFillHeader(info){
  const map={c:'hc',b:'hb',j:'hj',u:'hu',z:'hz'};
  for(const [k,id] of Object.entries(map)){
    if(!headerManual[id] && info[k]){
      $(id).value=info[k];
      $(id).style.borderColor='var(--ac)'; // 琥珀色边框 = 自动识别
      setTimeout(()=>{$(id).style.borderColor='';},2000);
    }
  }
  up(); // 触发预览更新
}

// 在 init() 末尾绑定 — 中栏 hcfg 内所有 input/select 的 change 标记为手动
function initHeaderManual(){
  document.querySelectorAll('#hc,#hb,#hj,#hu,#hz,#ha,#ht').forEach(el=>{
    el.addEventListener('input',()=>{headerManual[el.id]=true;el.style.borderColor='';});
    el.addEventListener('change',()=>{headerManual[el.id]=true;el.style.borderColor='';});
  });
}
```

- [ ] **Step 3: 实现 `processImport()` — 串联文件解析 + 字段匹配 + 表头填充**

```javascript
async function processImport(){
  try{
    // 1. 验证 SHP
    await parseShpHeader(loadedFiles.shp);

    // 2. 解析字段名
    let fieldNames=[];
    if(loadedFiles.dbf){
      fieldNames=await parseDbfFields(loadedFiles.dbf);
      autoMatchFields(fieldNames);
    }

    // 3. 解析 PRJ
    if(loadedFiles.prj){
      const prjText=await loadedFiles.prj.text();
      const info=prjToHeader(prjText);
      autoFillHeader(info);
    }

    // 4. 更新预览
    up();
    toast('文件识别完成');
  }catch(e){
    console.error(e);
    toast('文件解析失败: '+e.message);
  }
}
```

- [ ] **Step 4: 确认** — 导入含 .shp+.dbf+.prj 的文件集，验证下拉框替换为文件字段名、表头自动填充。

---

### Task 4: 实时预览 — 完整 TXT 输出 + debounce

**Files:**
- Modify: `prototype_v7.html` 中的 `up()` 函数和预览区

- [ ] **Step 1: 重写 `up()` — 完整 TXT 预览，含模拟坐标**

```javascript
let previewTimer=null, lastPreviewKey='';

function schedulePreview(){
  clearTimeout(previewTimer);
  previewTimer=setTimeout(up,150);
}

function up(){
  // 表头部分（来自中栏）
  let txt=`[属性描述]
坐标系=${$('hc').value}
几度分带=${$('hb').value}
投影类型=${$('hj').value}
计量单位=${$('hu').value}
带号=${$('hz').value}
精度=${$('ha').value}
转换参数=${$('ht').value}
[地块坐标]`;

  // 如果有导入文件，追加地块预览
  if(loadedFiles.shp){
    const pp=+$('pp').value||3;
    const oj=$('oj').checked;
    const oo=$('oo').checked;
    const ox=$('ox').checked;

    // 模拟地块数据（后续 ArcPy 桥接真实数据）
    const plots=[
      {name:'城东新区A',id:'DK-001',area:'125.36',use:'住宅用地',coords:[[38501234.567,3421098.765],[38501456.789,3421102.345],[38501460.123,3420987.654],[38501230.456,3420985.432]]},
      {name:'城东新区B',id:'DK-002',area:'89.72',use:'商业用地',coords:[[38501500.111,3421100.222],[38501600.333,3421105.444],[38501598.555,3420990.666],[38501499.777,3420988.888]]},
      {name:'城东新区C',id:'DK-003',area:'210.45',use:'工业用地',coords:[[38501000.999,3421100.111],[38501150.888,3421103.333],[38501148.777,3420985.555],[38500999.666,3420983.777]]}
    ];

    const fnMap={'fn':'地块名称','fi':'地块编号','fa':'面积','fu':'用途'};
    Object.keys(fnMap).forEach(k=>{
      const sel=$(k);
      if(sel&&sel.value) fnMap[k]=sel.value;
    });

    const MAX_PLOTS=3,MAX_PTS=20;
    txt+='\n';
    const shown=Math.min(plots.length,MAX_PLOTS);
    for(let p=0;p<shown;p++){
      const pl=plots[p];
      txt+=`\n${fnMap.fn}=${pl.name}`;
      txt+=`\n${fnMap.fi}=${pl.id}`;
      txt+=`\n${fnMap.fa}=${pl.area}`;
      txt+=`\n${fnMap.fu}=${pl.use}`;
      const pts=pl.coords.slice(0,MAX_PTS);
      pts.forEach((c,i)=>{
        let x=c[0].toFixed(pp),y=c[1].toFixed(pp);
        if(ox){const t=x;x=y;y=t}
        const pre=oj?'J':'';
        txt+=`\n${pre}${i+1},${x},${y}`;
      });
    }
    if(plots.length>MAX_PLOTS) txt+=`\n; 共 ${plots.length} 个地块（此处仅显示前 ${MAX_PLOTS} 个）`;
  }

  // 仅更新变化部分
  const key=txt;
  if(key===lastPreviewKey) return;
  lastPreviewKey=key;
  $('pv').textContent=txt;
}
```

- [ ] **Step 2: 绑定 debounce 监听 — 左栏和中栏所有输入变更触发 `schedulePreview()`**

在 `init()` 中替换原有 `change` 监听：

```javascript
// 旧的（会直接调 up）：
// document.querySelectorAll('#R input,#R select,.pnl-l select').forEach(...)
// 新的：
document.querySelectorAll('.pnl-l select,.pnl-l input,.pnl-m select,.pnl-m input, #R input, #R select').forEach(e=>{
  e.addEventListener('input',schedulePreview);
  e.addEventListener('change',schedulePreview);
  // checkbox 也需要
  if(e.type==='checkbox') e.addEventListener('change',schedulePreview);
});
document.querySelectorAll('.pnl-m input[type=checkbox]').forEach(e=>e.addEventListener('change',schedulePreview));
```

直接在 `init()` 末尾统一绑定所有输入到 debounce：

```javascript
function bindPreviewListeners(){
  const all=document.querySelectorAll('.pnl-l input,.pnl-l select,.pnl-m input,.pnl-m select,.pnl-m input[type=checkbox],#R input,#R select');
  all.forEach(el=>{
    el.addEventListener('input',schedulePreview);
    el.addEventListener('change',schedulePreview);
  });
}
```

- [ ] **Step 3: 确认** — 修改小数位/勾选 checkbox → 150ms 后预览区自动刷新；改坐标系 → 预览表头更新但坐标串不变。

---

### Task 5: 预览区 DOM — 右栏高度自适应预览内容

**Files:**
- Modify: `prototype_v7.html` 中的 CSS

- [ ] **Step 1: 预览区滚动行为**

当前 `.pv` 有 `max-height:114px` — 当完整 TXT 预览内容增多时可能不够。改为：

```css
.pv{
  font-family:"JetBrains Mono","Cascadia Code",Consolas,monospace;
  font-size:10.5px;line-height:1.55;padding:10px 12px;
  border-radius:6px;white-space:pre;overflow-y:auto;
  max-height:260px; /* 从 114px 加大 */
  background:var(--pv-bg);color:var(--pv-fg);
  border:1px solid var(--brd);
  position:relative;
  box-shadow:var(--shadow-sm);
}
```

（直接替换 `.pv` 中的 `max-height` 值）

- [ ] **Step 2: 确认** — 导入文件后，预览区显示完整 TXT，内容多时有纵向滚动条。

---

### Task 6: 集成验证 — 端到端测试

**Files:**
- Verify: `prototype_v7.html`

- [ ] **Step 1: 无文件状态** — 打开页面，预览区显示表头骨架 + `[地块坐标]`（无地块数据），跟当前 v7 行为一致。

- [ ] **Step 2: 导入 SHP 三件套** — 准备一组测试文件（.shp + .dbf + .prj）：
  1. 字段名包含 `DKMC`、`DKBH`、`MJ`、`DKYT`、`TFH`、`DLBM`
  2. PRJ 包含 CGCS2000 / 3度带 / 中央经线114

  导入后验证：下拉框全部替换为文件字段名并自动选中匹配项；表头自动填入坐标系/分带/带号等。

- [ ] **Step 3: 修改字段映射 / 参数** — 手动改地块名称字段 → 预览区地图块名立即更新（150ms debounce）；改 XY 标反勾选 → 坐标 X/Y 立即交换。

- [ ] **Step 4: 预设交互** — 先导入文件（自动识别生效），再切预设 → 触发 `ld()` → 但 `headerManual` 标记保护手动改动不被覆盖。切换文件 → `headerManual` 清空，允许新文件重新识别。

- [ ] **Step 5: 主题切换** — 浅色/暗色切换 → 预览区背景和文字色跟随更新。

---

### Task 7: 清理旧代码 + 最终提交

- [ ] **Step 1: 删除不再需要的旧代码**

```javascript
// 删除：
const DF=[...]; // 旧 demo 文件列表
function af(){...} // 旧 demo 函数
// tp() 函数中不再需要（pnl-tog 已移除）
```

- [ ] **Step 2: 确保 init() 完整调用链**

```javascript
function init(){
  // ... 现有预设加载逻辑 ...
  initFileInput();      // Task 1
  initHeaderManual();   // Task 3
  bindPreviewListeners();// Task 4
  up();
}
```

- [ ] **Step 3: 浏览器最终验证** — 完整走一遍流程：打开 → 默认浅色 → 导入文件 → 字段自动匹配 → 表头自动填充 → 预览实时更新 → 修改参数 → 预览 debounce 刷新 → 切暗色 → 一切正常。
