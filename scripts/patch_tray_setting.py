import re

with open('tauri_app/dist/assets/index-TmELkt7w.js', 'r', encoding='utf-8') as f:
    content = f.read()

# 查找设置变量定义
target_var = 'R=ne(!1),L=ne(!0)'
if target_var not in content:
    print('Error: target_var not found')
    exit(1)

# 查找配置加载
target_load = 'R.value=!!h.stealth_mode,I.value=!!h.auto_accept_trusted'
if target_load not in content:
    print('Error: target_load not found')
    exit(1)

# 查找隐身模式的模板位置
# ... a("div",ff,[a("div",df,[ ... onChange: ... stealth_mode:R.value ... ])])
m = re.search(r'(a\("div",([a-zA-Z0-9_$]+),\[a\("div",([a-zA-Z0-9_$]+),\[h\[28\]\|\|\(h\[28\]=a\("span",\{class:"setting-title"\},"[^"]+",-1\)\),a\("div",([a-zA-Z0-9_$]+),\[Ve\(a\("input",\{"onUpdate:modelValue":h\[2\]\|\|\(h\[2\]=Z=>R\.value=Z\),type:"checkbox",class:"custom-switch",onChange:h\[3\]\|\|\(h\[3\]=Z=>C\(\{stealth_mode:R\.value\}\)\)\},null,544\),\[\[Rn,R\.value\]\]\)\s*\]\)\s*\]\),h\[29\]\|\|\(h\[29\]=a\("div",\{class:"setting-desc"\},"[^"]+",-1\)\)\]\))', content)

if not m:
    print('Error: stealth_mode template not found')
    # 尝试更宽松的正则
    idx = content.find('stealth_mode:R.value')
    print('Context around stealth_mode:R.value:')
    print(content[idx-150:idx+200])
    exit(1)

full_stealth_match = m.group(1)
cls_item = m.group(2)
cls_row = m.group(3)
cls_ctrl = m.group(4)

print(f"Matched! cls_item={cls_item}, cls_row={cls_row}, cls_ctrl={cls_ctrl}")

tray_ui = f',a("div",{cls_item},[a("div",{cls_row},[a("span",{{class:"setting-title"}},"关闭时最小化到托盘"),a("div",{cls_ctrl},[Ve(a("input",{{"onUpdate:modelValue":Z=>tt.value=Z,type:"checkbox",class:"custom-switch",onChange:Z=>C({{minimize_to_tray:tt.value}})}},null,544),[[Rn,tt.value]])])]),a("div",{{class:"setting-desc"}},"点击关闭按钮时隐藏到系统托盘，保持后台运行而非退出")])'

new_content = content.replace(target_var, 'R=ne(!1),tt=ne(!1),L=ne(!0)', 1)
new_content = new_content.replace(target_load, 'R.value=!!h.stealth_mode,tt.value=!!h.minimize_to_tray,I.value=!!h.auto_accept_trusted', 1)
new_content = new_content.replace(full_stealth_match, full_stealth_match + tray_ui, 1)

with open('tauri_app/dist/assets/index-TmELkt7w.js', 'w', encoding='utf-8') as f:
    f.write(new_content)

print("SUCCESSFULLY PATCHED dist/assets/index-TmELkt7w.js")
