"""Live assertions for spike 003; only the disposable project KWin session."""
import json
import pathlib
import queue
import re
import signal
import subprocess
import sys
import threading
import time


def parse_submission(line):
    m=re.search(r'LENS_SUBMIT commit=(\d+) rect=\(\s*(-?\d+)\s*,\s*(-?\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\) source=\(\s*(\d+)\s*,\s*(\d+)\s*\) color=(#[0-9A-Fa-f]{6})',line)
    if m is None:return None
    values=[int(v) for v in m.groups()[:7]]
    if values[3]<=0 or values[4]<=0:return None
    return {'commit':values[0],'rect':tuple(values[1:5]),'source':tuple(values[5:7]),'color':m[8].upper()}


def main():
    from PIL import Image
    root=pathlib.Path('/home/erick/Projects/Swatch_Board_App')
    env=json.loads((root/'target/isolated-native-lab-1/session-env.json').read_text())
    assert env['XDG_RUNTIME_DIR']==str(root/'target/isolated-native-lab-1/runtime')
    assert env['WAYLAND_DISPLAY']=='swatch-test'
    assert env['DBUS_SESSION_BUS_ADDRESS'].startswith('unix:path='+str(root/'target/isolated-native-lab-1/runtime/bus'))
    tag='private-clean-overlay-live-1'
    log=root/f'target/{tag}-probe.log'
    fixture=root/f'target/{tag}-fixture.log'
    binary=root/'target/release/examples/phase1a_clean_overlay_probe'
    assert binary.is_file(),'Build the real probe before running live acceptance'
    p=subprocess.Popen([str(binary),'--run'],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
    assert p.stdout is not None
    lines=p.stdout
    q=queue.Queue()
    def reader():
        with log.open('w') as f:
            for line in lines:
                f.write(line);f.flush();q.put(line)
    worker=threading.Thread(target=reader,daemon=True);worker.start()
    fp=None
    def inject(*args):
        subprocess.run([str(root/'target/release/examples/phase1a_input'),*map(str,args)],cwd=root,env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=3)
    def snapshot(label):
        path=root/f'target/{tag}-{label}.png'
        subprocess.run(['/usr/bin/spectacle','--background','--nonotify','--fullscreen','--output',str(path)],cwd=root,env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5)
        image=Image.open(path).convert('RGB')
        assert image.size==(2048,768),'Unexpected private test geometry; no input may be injected'
        return image
    submitted={};origin=None;baseline=None;target=None;hide_barrier=None;clean_pts=None;selected_color=None
    old_lens_pixel=None;underlying_pixel=None;events=[]
    signal.signal(signal.SIGTERM,lambda s,f:sys.exit(128+s))
    try:
        deadline=time.monotonic()+40
        while time.monotonic()<deadline:
            try:line=q.get(timeout=.1)
            except queue.Empty:
                if p.poll() is not None:break
                continue
            if 'Starting the authorized ScreenCast streams' in line:
                time.sleep(1);snapshot('chooser')
                for x,y in [(332,300),(732,734)]:
                    inject('move',x,y);inject('left');time.sleep(.1)
            if 'PREPARE 3s:' in line:
                with fixture.open('w') as out:
                    fp=subprocess.Popen([str(root/'target/release/examples/phase1a_fixture')],cwd=root,env=env,stdout=out,stderr=subprocess.STDOUT)
                time.sleep(.7);inject('move',800,500);time.sleep(.1)
                text=fixture.read_text();motions=[l for l in text.splitlines() if 'motion local_x=' in l or 'pointer_enter local_x=' in l]
                assert motions,'No calibrated fixture under pointer; refusing click'
                m=re.search(r'local_x=([0-9.]+) local_y=([0-9.]+)',motions[-1]);assert m
                origin=(800-float(m[1]),500-float(m[2]))
                inject('left');time.sleep(.1)
                baseline=[l for l in fixture.read_text().splitlines() if l.startswith('EVENT')][-1]
                assert 'focused=true' in baseline
                inject('move',origin[0]+450,origin[1]+150)
            candidate=parse_submission(line)
            if candidate is not None:
                candidate['after_clean']=clean_pts is not None
                submitted[candidate['commit']]=candidate
            if 'GUARD_HIDE_PRESENTED' in line:
                m=re.search(r'barrier_ns=(\d+)',line);assert m,line
                hide_barrier=int(m[1]);events.append(line.strip())
            if 'GUARD_CLEAN_FRAME' in line:
                m=re.search(r'pts_ns=(\d+)',line);assert m,line
                clean_pts=int(m[1]);assert hide_barrier is not None and clean_pts>hide_barrier
                events.append(line.strip())
            if 'LENS_PRESENTED' in line:
                m=re.search(r'commit=(\d+)',line);assert m,line
                c=submitted.get(int(m[1]));assert c is not None,('Presented unknown commit',line)
                if target is None:
                    assert c['color']=='#0000FF',('Initial controlled blue mismatch',c)
                    x,y,w,h=c['rect'];target=(x+w//2,y+h//2)
                    assert origin is not None
                    local=(target[0]-origin[0],target[1]-origin[1])
                    assert 0<=local[0]<900 and 0<=local[1]<650,('Lens target outside fixture',target)
                    assert not (500<=local[0]<860 and 320<=local[1]<600),'Target is animated; choose a static fixture point'
                    old_lens_pixel=snapshot('before-move').getpixel(target)
                    inject('move',*target);events.append('MOVE_INTO_OLD_LENS '+str(target))
                elif c['source']==target and c['after_clean']:
                    underlying_pixel=snapshot('after-clean').getpixel(target)
                    expected='#%02X%02X%02X'%underlying_pixel
                    assert c['color']==expected,('Clean sample disagrees with independently captured uncovered target',c,expected)
                    assert old_lens_pixel!=underlying_pixel,'Occlusion test did not distinguish own lens pixels from the fixture'
                    selected_color=expected
                    inject('left')
                    break
        assert selected_color is not None,('No verified clean post-barrier presentation',log.read_text()[-6000:])
        code=p.wait(timeout=15);worker.join(timeout=2)
        text=log.read_text();fixture_text=fixture.read_text()
        final=[l for l in fixture_text.splitlines() if l.startswith('EVENT')][-1]
        assert code==0,(code,text[-3000:])
        selection_lines=[l for l in text.splitlines() if 'SELECTED_RELEASE button=272 action=select' in l]
        assert any(f'color={selected_color}' in l for l in selection_lines),('Selected color differs from verified displayed color',selection_lines,selected_color)
        assert clean_pts is not None and hide_barrier is not None
        assert 'session_close=Ok(())' in text
        for counter in ['presses=1','releases=1','keyboard_enters=1','keyboard_leaves=0','focused=true']:
            assert counter in final,(counter,final)
        result={'gate':'spike_only_not_phase1a','probe_exit':code,'origin':origin,'target':target,'old_lens_pixel_rgb':old_lens_pixel,'uncovered_target_rgb':underlying_pixel,'selected_color':selected_color,'hide_barrier_ns':hide_barrier,'clean_video_pts_ns':clean_pts,'clean_video_after_hide':clean_pts>hide_barrier,'fixture_baseline':baseline,'fixture_final':final,'session_closed':True,'events':events}
        (root/f'target/{tag}-result.json').write_text(json.dumps(result,indent=2))
        print(json.dumps(result,indent=2))
    finally:
        if p.poll() is None:p.terminate();p.wait(timeout=5)
        if fp is not None and fp.poll() is None:fp.terminate();fp.wait(timeout=5)


if __name__=='__main__':main()
