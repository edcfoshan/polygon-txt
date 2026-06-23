import json, urllib.request, time
from websocket import create_connection

resp = urllib.request.urlopen("http://127.0.0.1:9222/json")
targets = json.loads(resp.read())
page = next(t for t in targets if t.get("type") == "page")
ws = create_connection(page["webSocketDebuggerUrl"])

mid = 0
def send(method, params=None):
    global mid
    mid += 1
    msg = {"id": mid, "method": method, "params": params or {}}
    ws.send(json.dumps(msg))
    while True:
        r = json.loads(ws.recv())
        if r.get("id") == mid: return r

time.sleep(2)
# 切到 TXT→面 标签
send("Runtime.evaluate", {"expression": "window.sw('t')"})
time.sleep(1)

result = send("Runtime.evaluate", {
    "expression": """
    JSON.stringify({
        lujin_exists: !!document.getElementById('t_keep_lujin'),
        mingc_exists: !!document.getElementById('t_keep_mingc'),
        lujin_label: document.getElementById('t_keep_lujin')?.parentElement?.textContent?.trim(),
        mingc_label: document.getElementById('t_keep_mingc')?.parentElement?.textContent?.trim(),
        lujin_checked: document.getElementById('t_keep_lujin')?.checked,
        mingc_checked: document.getElementById('t_keep_mingc')?.checked,
        slabel_before: document.getElementById('t_keep_lujin')?.closest('.mode-t')?.querySelectorAll('.slabel'),
        // 附加属性区的位置：应在输出模式后、输出目录前
        t_panel_order: Array.from(document.querySelectorAll('.mode-t .pnl-bd > *')).map(el => {
            if (el.classList?.contains('slabel')) return 'SLABEL:' + el.textContent.trim();
            if (el.id === 't_filenameFieldRow') return 'filenameFieldRow';
            if (el.classList?.contains('ck-grid') && el.querySelector('#t_keep_lujin')) return '附加属性区';
            if (el.classList?.contains('fld')) return 'fld:输出目录';
            return el.tagName + (el.id ? '#'+el.id : '');
        }),
    })
    """,
    "returnByValue": True
})
val = result.get("result", {}).get("result", {}).get("value")
if val:
    d = json.loads(val)
    print("=== TXT→面 附加属性 UI 验证 ===")
    for k, v in d.items():
        if k == 't_panel_order':
            print(f"  面板顺序:")
            for item in v:
                print(f"    - {item}")
        else:
            print(f"  {k}: {v}")
    print()
    print(f"保留输出路径复选框存在: {'✓' if d['lujin_exists'] else '✗'}")
    print(f"txt名称复选框存在: {'✓' if d['mingc_exists'] else '✗'}")
    print(f"默认未勾选: {'✓' if not d['lujin_checked'] and not d['mingc_checked'] else '✗'}")
ws.close()
