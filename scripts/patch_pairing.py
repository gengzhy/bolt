import os
import sys

js_file = 'tauri_app/dist/assets/index-TmELkt7w.js'
css_file = 'tauri_app/dist/assets/index-mi7y1Qz8.css'

with open(js_file, 'r', encoding='utf-8') as f:
    js = f.read()

# 1. Patch useBt event handler for case 2, case 3, case 8
old_case2_3 = 'x.state==="connected"?e(`已连接 ${x.name??O}（${x.transport??"?"}）`):x.err?e(`连接断开：${x.err}`):e("已断开");break}case 3:Bn.value={pair_id:x.pair_id,uuid:x.uuid,name:x.name,code:x.code};break;'
new_case2_3 = 'x.state==="connected"?e(`已连接 ${x.name??O}（${x.transport??"?"}）`):x.err?e(`连接断开：${x.err}`):e("已断开");Bn.value&&Bn.value.uuid===O&&(Bn.value=null);break}case 3:Bn.value={pair_id:x.pair_id,uuid:x.uuid,name:x.name,code:x.code,is_initiator:Boolean(x.is_initiator)};break;'

if old_case2_3 in js:
    js = js.replace(old_case2_3, new_case2_3, 1)
    print("case 2 & 3 patched")
else:
    print("case 2 & 3 already patched")

old_case8 = 'case 8:e(`错误(${x.code}): ${x.message??""}`),s();break'
new_case8 = 'case 8:e(`错误(${x.code}): ${x.message??""}`),Bn.value=null,s();break'
if old_case8 in js:
    js = js.replace(old_case8, new_case8, 1)
    print("case 8 patched")

# 2. Patch Modals component
prefix = 'Cd=qt'
start = js.find(prefix)
end = js.find('z(s)?(q(),J(', start)

initiator_str = 'a("div",{class:"pair-anim"},[a("div",{class:"pulse-ring"}),a("div",{class:"spinner-ring"}),a("div",{class:"spinner-center"})]),a("h3",null,"设备配对中..."),a("p",null,[Be("正在与 "),a("b",null,K(z(t).name),1),Be(" 配对")]),a("div",{class:"code-container"},[a("p",bd,K(z(t).code),1)]),a("p",{class:"hint"},"请在对方设备屏幕上核对这 6 位验证码并确认接受"),a("div",md,[a("button",{class:"btn btn-cancel",onClick:o=>z(n)(z(t).pair_id,!1)},"取消")])'

receiver_str = 'l[5]||(l[5]=a("h3",null,"配对请求",-1)),a("p",null,[a("b",null,K(z(t).name),1),l[4]||(l[4]=Be(" 请求配对",-1))]),a("div",{class:"code-container"},[a("p",bd,K(z(t).code),1)]),l[6]||(l[6]=a("p",{class:"hint"},"请在对方屏幕上核对这 6 位验证码是否一致",-1)),a("div",md,[a("button",{class:"btn",onClick:l[0]||(l[0]=o=>z(n)(z(t).pair_id,!1))},"拒绝"),a("button",{class:"btn primary",onClick:l[1]||(l[1]=o=>z(n)(z(t).pair_id,!0))},"验证码一致，接受")])'

new_block = 'Cd=qt({__name:"Modals",setup(e){const{pairReq:t,transReq:s,respondPair:n,respondTransfer:i}=Gt();return(r,l)=>z(t)?(z(t).is_initiator?(q(),J("div",_d,[a("div",vd,[' + initiator_str + '])])):(q(),J("div",_d,[a("div",vd,[' + receiver_str + '])]))):'

js = js[:start] + new_block + js[end:]

with open(js_file, 'w', encoding='utf-8') as f:
    f.write(js)

print("JS patched successfully!")

# 3. Patch CSS with animation and code styling
with open(css_file, 'r', encoding='utf-8') as f:
    css = f.read()

extra_css = """
/* Pairing animation & code container */
.pair-anim{width:48px;height:48px;margin:0 auto 12px;position:relative;display:flex;align-items:center;justify-content:center}
.pulse-ring{position:absolute;inset:-6px;border-radius:50%;border:2px solid rgba(79,142,247,.28);animation:pulse 2.2s cubic-bezier(.2,.8,.4,1) infinite}
.spinner-ring{width:100%;height:100%;border-radius:50%;border:3px solid rgba(79,142,247,.14);border-top-color:#4f8ef7;border-right-color:#4f8ef7;animation:spin .9s cubic-bezier(.55,.15,.45,.85) infinite}
.spinner-center{position:absolute;width:8px;height:8px;border-radius:50%;background:#4f8ef7;box-shadow:0 0 10px rgba(79,142,247,.8)}
@keyframes spin{to{transform:rotate(360deg)}}
@keyframes pulse{0%{transform:scale(.8);opacity:.9}50%{transform:scale(1.22);opacity:.15}100%{transform:scale(.8);opacity:.9}}
.code-container{display:inline-flex;justify-content:center;background:rgba(79,142,247,.05);border:1px dashed rgba(79,142,247,.35);border-radius:12px;padding:8px 24px;margin:12px auto}
.btn-cancel{background:var(--panel,#fff);border:1px solid var(--line,#cbd5e1);color:var(--muted,#64748b);border-radius:8px;padding:8px 24px;font-size:13px;cursor:pointer;transition:all .2s}
.btn-cancel:hover{background:var(--bg-hover,#f1f5f9);color:var(--text,#1e293b);border-color:#94a3b8}
"""

if ".pair-anim" not in css:
    css += extra_css
    with open(css_file, 'w', encoding='utf-8') as f:
        f.write(css)
    print("CSS patched successfully!")
else:
    print("CSS already contains pair-anim")
