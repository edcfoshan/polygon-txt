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

time.sleep(1)
result = send("Runtime.evaluate", {
    "expression": """
    var panel = document.querySelector('.mode-t .pnl-bd');
    var items = [];
    for (var el of panel.children) {
        var label = '';
        if (el.classList.contains('slabel')) label = '标题:' + el.textContent.trim();
        else if (el.querySelector && el.querySelector('#t_keep_lujin')) label = '附加属性区';
        else if (el.querySelector && el.querySelector('#out_dir')) label = '输出目录';
        else if (el.querySelector && el.querySelector('input[name=t_output_mode]')) label = '输出模式区';
        else if (el.id === 't_filenameFieldRow') label = '拆分文件名下拉';
        else label = el.tagName + '.' + (el.className||'');
        items.push(label);
    }
    items.join(' | ')
    """,
    "returnByValue": True
})
print("=== TXT→面 面板顺序（从上到下）===")
print(result.get("result", {}).get("result", {}).get("value"))
ws.close()
