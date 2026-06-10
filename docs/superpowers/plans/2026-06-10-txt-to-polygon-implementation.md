# TXT→面 功能实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**目标：** 在 prototype_v8.html 的 TXT→面 模式中实现完整的 UI（输出设置 + 增强解析日志）和标题栏版本号

**架构：** 单 HTML 文件，无构建步骤。所有改动集中在 prototype_v9.html（基于 v8 另存修改）。CSS/HTML/JS 都在同一文件内。

**涉及的资源：** `prototype_v9.html`（基于 v8 复制后修改）

---

### Task 1: 复制 v8 → v9 + 标题栏加版本号

**文件：**
- 复制：`prototype_v8.html` → `prototype_v9.html`

- [ ] **Step 1: 复制文件**

```powershell
Copy-Item "C:\Users\Administrator\Documents\txt与gdb互转\prototype_v8.html" "C:\Users\Administrator\Documents\txt与gdb互转\prototype_v9.html"
```

- [ ] **Step 2: 标题栏加 V1.0**

在 `prototype_v9.html` 中找到标题栏的 `.brand-sub` 元素：

```html
<span class="brand-sub">V1.0</span>
```

当前代码中 `.brand-sub` 在 brand 区域，修改其内容为 "V1.0"。

定位行（大约在 titlebar 中 brand 部分的 HTML）：
```html
<span class="brand-sub">测镜</span>
```
改为：
```html
<span class="brand-sub">V1.0</span>
```

- [ ] **Step 3: 确认改动**

打开 `prototype_v9.html` 确认标题栏显示 "V1.0"。

---

### Task 2: 左栏添加输出设置 UI

**文件：**
- 修改：`prototype_v9.html`

- [ ] **Step 1: 在 TXT→面左栏添加输出设置 HTML**

找到 TXT→面 左栏中文件列表下方的区域（现有代码在 `.mode-t .pnl-l .pnl-bd` 内）：

当前 TXT→面 左栏 HTML 结构：
```html
<div class="mode-t">
  <div class="pnl-h"><span class="pnl-label">文件</span></div>
  <div class="pnl-bd">
    <div class="drop" id="dropZoneTxt">...</div>
    <div class="file-list" id="flT"></div>
    <div style="text-align:right;margin-top:2px"><button class="btn" onclick="clearAllFilesTxt()">清空全部</button></div>
  </div>
</div>
```

在清空按钮后追加"输出设置"区域：

```html
<div class="div"></div>
<div class="slabel">输出设置</div>

<div class="fld">
  <label>输出格式</label>
  <div style="display:flex;gap:6px">
    <label class="ck"><input type="checkbox" id="of_shp" checked>SHP</label>
    <label class="ck"><input type="checkbox" id="of_gdb" checked>GDB</label>
  </div>
</div>

<div class="fld">
  <label>文件组织</label>
  <div style="display:flex;gap:10px">
    <label class="ck"><input type="radio" name="org_mode" id="org_sep" checked>每个TXT一个图层</label>
    <label class="ck"><input type="radio" name="org_mode" id="org_merge">合并输出</label>
  </div>
</div>

<div class="fld">
  <label>输出目录</label>
  <div style="display:flex;gap:4px">
    <input type="text" id="out_dir" readonly placeholder="选择输出文件夹…" style="flex:1;min-width:0">
    <button class="btn" id="out_btn" onclick="selectOutputDir()" style="white-space:nowrap">浏览…</button>
  </div>
</div>
```

- [ ] **Step 2: 验证 UI 渲染**

打开浏览器，切换到 TXT→面（data-mode="t"），确认：
- 左栏出现"输出设置"区域
- SHP/GDB 复选框存在
- 文件组织单选按钮存在
- 输出目录输入框 + 浏览按钮存在
- 分隔线 `.div` 正确显示

---

### Task 3: 输出目录选择逻辑 + 状态管理

**文件：**
- 修改：`prototype_v9.html`（JS）

- [ ] **Step 1: 添加 `selectOutputDir()` 函数**

在 JS 中（比如 `initTxtImport` 附近）加入：

```js
function selectOutputDir(){
  const inp=document.createElement('input');
  inp.type='file';inp.webkitdirectory=true;
  inp.onchange=function(e){
    const files=e.target.files;
    if(files.length){
      const path=files[0].webkitRelativePath.split('/')[0];
      // 实际路径通过 file 对象的 fullPath 无法直接获取，回退到用户选择后获取文件夹名
      // 浏览器沙箱限制：无法直接获取完整路径，显示文件夹名
      const dirInput=document.getElementById('out_dir');
      if(dirInput) dirInput.value=path;
    }
  };
  inp.click();
}
```

> **注意：** 浏览器安全限制导致无法直接获取文件夹的完整路径。`selectOutputDir()` 实际获取的是文件夹名称。生产环境中需要 Electron 或 Node.js File API 来获取真实路径。对于当前 HTML 原型，显示选择的文件夹名称即可。

- [ ] **Step 2: 添加 `getOutputSettings()` 工具函数**

```js
function getOutputSettings(){
  return {
    shp: document.getElementById('of_shp')?.checked||false,
    gdb: document.getElementById('of_gdb')?.checked||false,
    merge: document.getElementById('org_merge')?.checked||false,
    dir: document.getElementById('out_dir')?.value||''
  };
}
```

---

### Task 4: 增强解析日志（树形结构 + 坐标系信息）

**文件：**
- 修改：`prototype_v9.html`（`parseTxtPreview()` 和 `renderTxtParseLog()`）

- [ ] **Step 1: 增强 `parseTxtPreview()` 返回坐标系信息**

现有 `parseTxtPreview()` 只返回 `{plots, totalPlots}`。需要增强为返回 `{plots, totalPlots, crs}`，其中 `crs` 从 TXT 的 `[属性描述]` 段解析。

修改代码：

```js
function parseTxtPreview(text){
  const lines=text.split('\n').map(l=>l.trim()).filter(l=>l);
  let section='', inCoords=false, plots=[], currentPlot=null;
  let crs={c:'2000国家大地坐标系',b:'3',j:'高斯克吕格',u:'米',z:'38'};
  for(const line of lines){
    if(line==='[属性描述]'){section='attr';continue}
    if(line==='[项目信息]'){section='proj';continue}
    if(line==='[地块坐标]'){section='coords';inCoords=true;continue}
    if(line.startsWith('[')){section='';inCoords=false;continue}
    if(section==='attr'){
      if(line.startsWith('坐标系=')) crs.c=line.split('=')[1]||crs.c;
      if(line.startsWith('几度分带=')) crs.b=line.split('=')[1]||crs.b;
      if(line.startsWith('投影类型=')) crs.j=line.split('=')[1]||crs.j;
      if(line.startsWith('计量单位=')) crs.u=line.split('=')[1]||crs.u;
      if(line.startsWith('带号=')) crs.z=line.split('=')[1]||crs.z;
    }
    if(!inCoords) continue;
    if(line.includes(',@')||line.endsWith(',@')){
      const parts=line.split(',');
      const count=parseInt(parts[0])||0;
      const area=parts[1]||'';
      const name=parts[3]||'';
      const tfh=parts[5]||'';
      const use=parts[6]||'';
      const dlbm=parts[7]||'';
      currentPlot={count,area,name,tfh,use,dlbm,coords:[]};
      plots.push(currentPlot);
    }else if(currentPlot){
      const m=line.match(/^J?\d+,\d+,([\d.]+),([\d.]+)/);
      if(m) currentPlot.coords.push([parseFloat(m[1]),parseFloat(m[2])]);
    }
  }
  return {plots,totalPlots:plots.length,crs};
}
```

- [ ] **Step 2: 更新 `updateTxtStats()` 显示汇总统计**

```js
function updateTxtStats(){
  const pv=document.getElementById('pvT');if(!pv)return;
  let totalPlots=0,totalPts=0;
  txtFiles.forEach(f=>{totalPlots+=f.totalPlots;f.plots.forEach(p=>totalPts+=p.coords.length)});
  // 文件列表已有，更新解析日志
  renderTxtParseLog();
}
```

- [ ] **Step 3: 重写 `renderTxtParseLog()` 使用树形格式**

```js
function renderTxtParseLog(){
  const pv=document.getElementById('pvT');if(!pv)return;
  let txt='';
  const MAX=10;
  const shown=Math.min(txtFiles.length,MAX);
  for(let i=0;i<shown;i++){
    const f=txtFiles[i];
    const crs=f.crs||{};
    txt+=`◆ ${f.name}\n`;
    txt+=`  坐标系: ${crs.c||'2000国家大地坐标系'} / ${crs.b||'3'}度分带 / 带号${crs.z||'38'}\n`;
    txt+=`  地块: ${f.totalPlots} | 坐标点: 共${f.plots.reduce((s,p)=>s+p.coords.length,0)}个\n`;
    // 显示前 10 个地块明细
    const maxShow=Math.min(f.plots.length,10);
    for(let j=0;j<maxShow;j++){
      const p=f.plots[j];
      const pre=j===maxShow-1?'└─':'├─';
      txt+=`  ${pre} ${p.name||'?'} ${p.use||''} ${p.area||'?'} ${p.count||p.coords.length}点\n`;
    }
    if(f.plots.length>10) txt+=`  ... 还有 ${f.plots.length-10} 个地块\n`;
    if(i<shown-1) txt+='\n';
  }
  if(txtFiles.length>MAX) txt+=`\n; 共 ${txtFiles.length} 个文件（此处仅显示前 ${MAX} 个）`;
  pv.textContent=txt||'等待导入 TXT 文件…';
}
```

- [ ] **Step 4: 在 `handleTxtFiles()` 中保持 `crs` 在 `txtFiles` 对象中**

确认 `handleTxtFiles()` 正确保存 crs：

```js
async function handleTxtFiles(fileList){
  txtFiles.length=0;
  for(const f of fileList){
    if(!f.name.toLowerCase().endsWith('.txt')) continue;
    const text=await f.text();
    const info=parseTxtPreview(text);
    txtFiles.push({name:f.name, size:f.size, ...info}); // info 现在包含 crs
  }
  if(!txtFiles.length){toast('请选择 .txt 文件');return}
  renderTxtFileList();
  renderTxtParseLog();
  updateTxtStats();
}
```

当前代码 `txtFiles.push({name:f.name, size:f.size, ...info})` 已经用展开运算符，无需改动。

---

### Task 5: 运行转换时校验输出设置

**文件：**
- 修改：`prototype_v9.html`（`runTxt()`）

- [ ] **Step 1: 增强 `runTxt()`**

```js
function runTxt(){
  if(!txtFiles.length){toast('请先导入 TXT 文件');return}
  const settings=getOutputSettings();
  if(!settings.shp&&!settings.gdb){toast('请至少选择一种输出格式');return}
  if(!settings.dir){toast('请选择输出目录');return}
  const pf=document.getElementById('pfT'),ps=document.getElementById('psT');
  pf.style.width='100%';ps.textContent='完成';
  const mode=settings.merge?'合并':'每个TXT一个';
  const fmt=(settings.shp?'SHP ':'')+(settings.gdb?'GDB':'').trim();
  toast(`✓ 转换完成 — ${txtFiles.length} 个文件 → ${fmt}（${mode}）`);
}
```

---

### Task 6: 自检 + 打开浏览器验证

- [ ] **Step 1: 页面结构检查**

用浏览器打开 `prototype_v9.html`，切换到 TXT→面 模式（点击"TXT→面" tab），逐项确认：
- 标题栏显示 V1.0
- 左栏包含：TXT 导入区 + 文件列表 + 清空按钮 + 分隔线 + 输出设置（格式/组织/目录）
- 右栏包含：解析结果区域 + 运行按钮 + 进度条
- 面→TXT 模式不受影响

- [ ] **Step 2: 功能交互检查**

- 导入一个正确的 TXT 文件，确认树形解析日志正常显示（坐标系、地块列表）
- 输出格式 SHP/GDB 可分别勾选
- 文件组织单选可切换
- 浏览按钮可点击（但不要求获取真实路径）
- 清空按钮正常工作
- 运行按钮在缺输出目录时弹出提示

---

### Task 7: 清理旧会话文件

- [ ] **Step 1: 删除 brainstorm 会话文件（可选）**

```powershell
Remove-Item "C:\Users\Administrator\Documents\txt与gdb互转\.superpowers" -Recurse -Force -ErrorAction SilentlyContinue
```
