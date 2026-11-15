"""Only operates on the project's isolated, disposable KWin session."""
import json, pathlib, subprocess, sys, time
root=pathlib.Path(__file__).resolve().parent.parent
assert root == pathlib.Path('/home/erick/Projects/Swatch_Board_App')
env=json.loads((root/'target/isolated-native-lab-1/session-env.json').read_text())
assert env['XDG_RUNTIME_DIR']==str(root/'target/isolated-native-lab-1/runtime')
assert env['WAYLAND_DISPLAY']=='swatch-test'
assert env['DBUS_SESSION_BUS_ADDRESS'].startswith('unix:path='+str(root/'target/isolated-native-lab-1/runtime/bus'))
name=sys.argv[1]
assert name.startswith('private-shield-live-') and name.removeprefix('private-shield-live-').isdigit()
log=root/f'target/{name}-probe.log'
log.unlink(missing_ok=True)
with (root/f'target/{name}-driver.log').open('w') as out:
    p=subprocess.Popen([sys.executable,str(root/f'target/run-{name}.py')],cwd=root,stdout=out,stderr=subprocess.STDOUT)
    try:
        deadline=time.monotonic()+15
        while time.monotonic()<deadline:
            if log.exists() and 'Starting the authorized ScreenCast streams' in log.read_text():break
            if p.poll() is not None:raise RuntimeError('Driver stopped before chooser')
            time.sleep(.1)
        else:raise RuntimeError('No chooser stage')
        time.sleep(1)
        subprocess.run(['/usr/bin/spectacle','--background','--nonotify','--fullscreen','--output',str(root/f'target/{name}-chooser.png')],env=env,check=True,timeout=8)
        cmd=[str(root/'target/release/examples/phase1a_input')]
        # Fixed fixture coordinates: two 1024x768 virtual outputs, private chooser.
        for x,y in [(332,300),(690,300),(732,734)]:
            subprocess.run(cmd+['move',str(x),str(y)],env=env,check=True,stdout=subprocess.DEVNULL)
            subprocess.run(cmd+['left'],env=env,check=True,stdout=subprocess.DEVNULL)
            time.sleep(.1)
        print('Driver exit:',p.wait(timeout=30))
        print((root/f'target/{name}-result.json').read_text())
    finally:
        if p.poll() is None:p.terminate();p.wait(timeout=5)
