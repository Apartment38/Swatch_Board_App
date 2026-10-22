"""Two authorized output transitions; disposable project session only."""
from pathlib import Path
import json, os, re, subprocess, time, sys
from PIL import Image
root=Path('/home/erick/Projects/Swatch_Board_App')
env=json.loads((root/'target/isolated-native-lab-1/session-env.json').read_text())
assert env['XDG_RUNTIME_DIR']==str(root/'target/isolated-native-lab-1/runtime')
assert env['WAYLAND_DISPLAY']=='swatch-test'
assert env['DBUS_SESSION_BUS_ADDRESS'].startswith('unix:path='+str(root/'target/isolated-native-lab-1/runtime/bus'))
tag='native-matrix-'+(sys.argv[1] if len(sys.argv)>1 else 'normal')
assert re.fullmatch(r'native-matrix-[a-z0-9-]+',tag)
log=root/f'target/{tag}.log';stdout=root/f'target/{tag}.stdout'
cmd=[str(root/'target/release/examples/phase1a_input')]
def inject(*args):
    subprocess.run(cmd+list(map(str,args)),env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=3)
def counts(path):
    lines=[l for l in path.read_text().splitlines() if l.startswith('EVENT')]
    assert lines,path
    return {k:int(re.search(k+r'=(\d+)',lines[-1])[1]) for k in ['presses','releases','keyboard_enters','keyboard_leaves']}
def snap(label):
    path=root/f'target/{tag}-{label}.png'
    subprocess.run(['/usr/bin/spectacle','--background','--nonotify','--fullscreen','--output',str(path)],env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5)
    return Image.open(path).convert('RGB')
# Activate the primary fixture before launching so the private chooser is placed there.
inject('move',512,227);inject('left')
with stdout.open('w') as out,log.open('w') as err:
    p=subprocess.Popen([str(root/'target/release/swatch-board-picker'),'--native','--radius','5','--zoom','12','--diagnostics'],env=env,cwd=root,stdout=out,stderr=err)
    def wait(pattern,start=0,timeout=12):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            text=log.read_text();m=re.search(pattern,text[start:])
            if m:return text,m
            if p.poll() is not None:raise RuntimeError('Picker ended before '+pattern+'\n'+text[-3500:])
            time.sleep(.05)
        raise RuntimeError('Timeout '+pattern+'\n'+log.read_text()[-3500:])
    try:
        wait('Starting the authorized ScreenCast streams')
        time.sleep(.5);snap('chooser')
        for x,y in [(332,300),(690,300),(732,734)]:
            inject('move',x,y);inject('left');time.sleep(.1)
        wait('Native experimental selection active')
        inject('move',512,227)
        wait(r'LENS_PRESENTED[^\n]*color=#0000FF[^\n]*x: 512, y: 227')
        fixture_paths=[root/'target/native-select-fixture.log',root/'target/secondary-fixture.log']
        before=[counts(f) for f in fixture_paths]
        start=len(log.read_text());inject('move',1024+122,150)
        t,_=wait(r'LENS_PRESENTED[^\n]*color=#FF0000',start)
        red_image=snap('secondary-red')
        if red_image.size==(2048,768):assert red_image.getpixel((1024+122,150))==(255,0,0)
        switch1=t[start:]
        assert 'GUARD_HIDE_PRESENTED' in switch1 and 'GUARD_CLEAN_FRAME' in switch1
        start2=len(log.read_text());inject('move',606,227)
        wait(r'LENS_PRESENTED[^\n]*color=#FFFFFF[^\n]*x: 606, y: 227',start2)
        white_image=snap('primary-white')
        if white_image.size==(2048,768):assert white_image.getpixel((606,227))==(255,255,255)
        inject('left');code=p.wait(timeout=10);time.sleep(.2)
        text=log.read_text();color=stdout.read_text().strip()
        assert code==0 and color=='#FFFFFF',(code,color,text[-2500:])
        after=[counts(f) for f in fixture_paths];assert before==after,(before,after)
        assert 'GlobalShortcuts session closed' in text and 'ScreenCast session closed' in text
        transitions=[]
        for m in re.finditer(r'GUARD_HIDE_PRESENTED barrier_ns=(\d+)[^\n]*\n(?:(?!GUARD_HIDE_PRESENTED).)*?GUARD_CLEAN_FRAME pts_ns=(\d+) stream=(\d+)',text,re.S):
            cutoff,pts,stream=map(int,m.groups());assert pts>cutoff
            transitions.append({'cutoff_ns':cutoff,'video_pts_ns':pts,'stream':stream})
        assert len(transitions)>=2 and len({v['stream'] for v in transitions})>=2,transitions
        result={'case':tag,'exit':code,'stdout':color,'counts_before':before,'counts_after':after,'transitions':transitions,'secondary_screenshot_size':red_image.size,'primary_screenshot_size':white_image.size,'sessions_closed':True,'gate':'partial_not_phase1a'}
        (root/f'target/{tag}-result.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
    finally:
        if p.poll() is None:p.terminate();p.wait(timeout=5)
