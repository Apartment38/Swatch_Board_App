import os, pathlib, subprocess, sys, time, json, socket, select, signal
root=pathlib.Path('/home/erick/Projects/Swatch_Board_App')
base=root/'target/isolated-native-lab-1'
for name in ['home','runtime','tmp','config','data/applications','cache','state','logs']:
    (base/name).mkdir(parents=True,exist_ok=True)
(base/'runtime').chmod(0o700)
env={'PATH':'/usr/bin:/bin:/usr/sbin','LANG':'C.UTF-8','HOME':str(base/'home'),
     'USER':'erick','LOGNAME':'erick','XDG_RUNTIME_DIR':str(base/'runtime'),
     'TMPDIR':str(base/'tmp'),'XDG_CONFIG_HOME':str(base/'config'),
     'XDG_DATA_HOME':str(base/'data'),'XDG_CACHE_HOME':str(base/'cache'),
     'XDG_STATE_HOME':str(base/'state'),'XDG_CONFIG_DIRS':'/etc/xdg',
     'XDG_DATA_DIRS':'/usr/local/share:/usr/share','XDG_CURRENT_DESKTOP':'KDE',
     'XDG_SESSION_DESKTOP':'KDE','XDG_SESSION_TYPE':'wayland','WAYLAND_DISPLAY':'swatch-test'}
entry=(root/'spikes/001-native-cancel/org.apartment38.SwatchBoard.NativeProbe.desktop').read_text()
entry=entry.replace('Exec=phase1a_cancel_probe --authorize','Exec='+str(root/'target/release/examples/phase1a_cancel_probe')+' --authorize')
(base/'data/applications/org.apartment38.SwatchBoard.NativeProbe.desktop').write_text(entry)
processes=[];stopping=False

def stop_signal(*_):
    global stopping
    stopping=True
signal.signal(signal.SIGTERM,stop_signal);signal.signal(signal.SIGINT,stop_signal)
def launch(name,args):
    f=(base/'logs'/f'{name}.log').open('w')
    p=subprocess.Popen(args,env=env,cwd=root,stdout=f,stderr=subprocess.STDOUT)
    processes.append((name,p));return p

def query(*args):
    return subprocess.run(['/usr/bin/qdbus6',*args],env=env,cwd=root,capture_output=True,text=True,timeout=5)

def wait_owner(name,timeout=20):
    end=time.monotonic()+timeout
    while time.monotonic()<end and not stopping:
        result=query('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus.NameHasOwner',name)
        if result.stdout.strip()=='true':return
        if any(p.poll() is not None for _,p in processes):break
        time.sleep(.1)
    raise RuntimeError('No private bus owner for '+name)

sock=None
try:
    buslog=(base/'logs/dbus.log').open('w')
    bus=subprocess.Popen(['/usr/bin/dbus-daemon','--session','--nofork','--address=unix:path='+str(base/'runtime/bus'),'--print-address=1'],env=env,stdout=subprocess.PIPE,stderr=buslog,text=True)
    processes.append(('dbus',bus));assert bus.stdout is not None
    address=bus.stdout.readline().strip()
    assert address.startswith('unix:path='+str(base/'runtime/bus'))
    env['DBUS_SESSION_BUS_ADDRESS']=address
    launch('pipewire',['/usr/bin/pipewire'])
    launch('kwin',['/usr/bin/kwin_wayland','--virtual','--width','1024','--height','768','--output-count','2','--scale','1','--socket','swatch-test','--no-lockscreen','--no-kactivities'])
    wait_owner('org.kde.KWin')
    # Policy-only profile links capture streams without opening audio/camera devices.
    launch('policy',['/usr/bin/wireplumber','--profile','policy'])
    # A virtual desktop has no physical pointer to keep wl_seat capabilities alive.
    launch('input-device',[str(root/'target/release/examples/phase1a_input'),'hold','3600'])
    support=query('org.kde.KWin','/KWin','supportInformation')
    assert support.returncode==0 and 'KWin::VirtualBackend' in support.stdout
    (base/'logs/support-information.txt').write_text(support.stdout)
    launch('portal-kde',['/usr/lib/xdg-desktop-portal-kde'])
    wait_owner('org.freedesktop.impl.portal.desktop.kde')
    launch('portal',['/usr/lib/xdg-desktop-portal','--replace'])
    wait_owner('org.freedesktop.portal.Desktop')
    (base/'session-env.json').write_text(json.dumps(env,indent=2))
    (base/'session-env.json').chmod(0o600)
    sock=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
    control=base/'runtime/control'
    if control.exists():control.unlink()
    sock.bind(str(control));sock.listen(2)
    print('NESTED_KWIN_READY '+str(base),flush=True)
    deadline=time.monotonic()+3600
    while not stopping and time.monotonic()<deadline:
        ready,_,_=select.select([sock],[],[],.2)
        if ready:
            c,_=sock.accept()
            with c:
                command=c.recv(1024).decode().strip()
                c.sendall(json.dumps({'isolated':True,'processes':{n:{'pid':p.pid,'exit':p.poll()} for n,p in processes}}).encode())
                if command=='stop':stopping=True
        failed=[(n,p.returncode) for n,p in processes if p.poll() is not None]
        if failed:raise RuntimeError('Private service exited: '+repr(failed))
except Exception as error:
    print('NESTED_KWIN_FAILED '+repr(error),flush=True)
    raise
finally:
    if sock is not None:sock.close()
    for _,p in reversed(processes):
        if p.poll() is None:p.terminate()
    for _,p in reversed(processes):
        try:p.wait(timeout=5)
        except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)
    print('NESTED_KWIN_STOPPED',flush=True)
