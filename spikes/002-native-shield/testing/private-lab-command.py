import os,pathlib,json,subprocess,sys,socket
root=pathlib.Path('/home/erick/Projects/Swatch_Board_App');base=root/'target/isolated-native-lab-1'
env=json.loads((base/'session-env.json').read_text())
assert env['XDG_RUNTIME_DIR']==str(base/'runtime')
assert env['DBUS_SESSION_BUS_ADDRESS'].startswith('unix:path='+str(base/'runtime/bus'))
if sys.argv[1:]==['--status'] or sys.argv[1:]==['--stop']:
    with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as s:
        s.connect(str(base/'runtime/control'));s.sendall(b'stop' if sys.argv[1]=='--stop' else b'status');print(s.recv(10000).decode())
else:
    # Every supplied test command receives the private bus/runtime and private HOME.
    result=subprocess.run(sys.argv[1:],env=env,cwd=root)
    raise SystemExit(result.returncode)
