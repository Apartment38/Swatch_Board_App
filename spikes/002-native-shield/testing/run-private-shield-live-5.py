import os, pathlib, subprocess, threading, queue, time, json, re
root=pathlib.Path('/home/erick/Projects/Swatch_Board_App')
env=json.loads((root/'target/isolated-native-lab-1/session-env.json').read_text())
assert env['XDG_RUNTIME_DIR']==str(root/'target/isolated-native-lab-1/runtime')
assert env['DBUS_SESSION_BUS_ADDRESS'].startswith('unix:path='+str(root/'target/isolated-native-lab-1/runtime/bus'))
log=root/'target/private-shield-live-5-probe.log';fixture=root/'target/private-shield-live-5-fixture.log'
p=subprocess.Popen([str(root/'target/release/examples/phase1a_shield_probe'),'--run','--large-cursor'],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
stdout=p.stdout
assert stdout is not None
q=queue.Queue()
def reader():
    with log.open('w') as f:
        assert stdout is not None
        for line in stdout:
            f.write(line);f.flush();q.put(line)
threading.Thread(target=reader,daemon=True).start()
fp=None;origin=None;base=None;cursor=False
def inject(*args):
    subprocess.run([str(root/'target/release/examples/phase1a_input'),*args],cwd=root,env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
try:
    deadline=time.monotonic()+180
    while time.monotonic()<deadline:
        try: line=q.get(timeout=.1)
        except queue.Empty:
            if p.poll() is not None:break
            continue
        if 'PREPARE 3s:' in line:
            fout=fixture.open('w')
            fp=subprocess.Popen([str(root/'target/release/examples/phase1a_fixture')],cwd=root,env=env,stdout=fout,stderr=subprocess.STDOUT)
            time.sleep(.7);inject('move','800','500');time.sleep(.1)
            ft=fixture.read_text();motions=[l for l in ft.splitlines() if 'motion local_x=' in l or 'pointer_enter local_x=' in l]
            if not motions:raise RuntimeError('Fixture not under calibration pointer; no click injected')
            m=re.search(r'local_x=([0-9.]+) local_y=([0-9.]+)',motions[-1]);assert m is not None
            origin=(800-float(m[1]),500-float(m[2]));inject('left');time.sleep(.1)
            base=[l for l in fixture.read_text().splitlines() if l.startswith('EVENT')][-1]
            if 'focused=true' not in base:raise RuntimeError('No focused fixture before shield')
            inject('move',str(origin[0]+450),str(origin[1]+150))
            print('FIXTURE_BASE',base,'ORIGIN',origin,flush=True)
        if 'CURSOR_SET ' in line:
            cursor=True;break
    if not cursor:raise RuntimeError('No actual cursor set; no selection click injected. '+log.read_text())
    time.sleep(.5)
    subprocess.run(['/usr/bin/spectacle','--background','--nonotify','--fullscreen','--pointer','--output',str(root/'target/private-shield-live-5-cursor.png')],env=env,cwd=root,check=True,timeout=8)
    time.sleep(.5)
    inject('left')
    code=p.wait(timeout=15);time.sleep(.2)
    text=log.read_text();ft=fixture.read_text();events=[l for l in ft.splitlines() if l.startswith('EVENT')]
    samples=[l for l in text.splitlines() if 'SAMPLE stream=' in l and 'offset=40 ' in l]
    blue=[l for l in samples if 'r: 0, g: 0, b: 255, a: 255' in l]
    result={'probe_exit':code,'origin':origin,'baseline':base,'fixture_final_events':events[-10:],'selected_left_release':'SELECTED_RELEASE button=272 action=select' in text,'cursor_set':cursor,'frame_callback_events':text.count('CURSOR_FRAME_CALLBACK '),'cursor_presented_events':len(re.findall(r'CURSOR_PRESENTATION Presented',text)),'offset40_sample_count':len(samples),'offset40_blue_count':len(blue),'cleanup_reported':'cursor_surface_destroyed=true' in text,'session_closed':'session_close=Ok(())' in text,'fixture_press_count':re.findall(r'presses=(\d+)',events[-1])[-1] if events else None,'fixture_release_count':re.findall(r'releases=(\d+)',events[-1])[-1] if events else None}
    (root/'target/private-shield-live-5-result.json').write_text(json.dumps(result,indent=2))
    print(text);print(json.dumps(result,indent=2),flush=True)
finally:
    if p.poll() is None:p.terminate();p.wait(timeout=5)
    if fp is not None and fp.poll() is None:fp.terminate();fp.wait(timeout=5)
