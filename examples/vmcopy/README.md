# VMCopy

A tool to send a file to a VM.

This supports Hyper-V, VirtualBox and VMware.

# Usage

An example of copying a file from a guest to the host.

```
> vmcopy.exe -e "C:\Program Files (x86)\VMware\VMware Player\vmrun.exe" --copy-from-guest --tool vmware --dst "C:\Users\user\Desktop" --player

Tool: VMware

id: name
-------
0: VM1
1: VM2
2: VM3
-------
id: 0
Guest username: user
Guest password:
Source path: /home/user/test.txt

Executable path: Some("C:\\Program Files (x86)\\VMware\\VMware Player\\vmrun.exe")
Copy from: Guest
VM name: VM1
Source path: /home/user/test.txt
Destination path: C:\Users\user\Desktop

OK?[Y/N]
success!
Press enter!
```

# License

MIT or Apache-2.0 License.
