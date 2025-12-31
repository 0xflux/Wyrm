# Project Milestones

Any item with a (L) tag is a contribution which will not be live (or requires further decision making) as this is intended to be
developed as a premium or otherwise private feature. These will be few and far between.

## (L) Features (locked currently for public consumption)

1) [ ] NG Proxy Bypass (NGPB).
2) [ ] Additional loaders / start from RDLL - configurable, maybe things like early bird, syscalls, etc.
3) [ ] Image hashes in autoloot.
4) [ ] Runtime obfuscation, sleep masking - should spawn from the RDI bootstrap? Shellcode? Where and how? The RDI alloc for the actual DLL can just be encrypted?
5) [ ] **Entire** website clone, and serve download from named page.
6) [ ] Ransomware **SIMULATION** for Business
7) [ ] Execute dotnet in sacrificial process

### v0.7.2

1) [x] Finish implementation of `spawn`
   1) [x] `spawn` basic implementation
   2) [x] Option for spawning as image from profile
   3) [x] Documentation for this feature
2) [x] Reflective injector should dispatch the ETW bypass too I think if set? Or separate profile option?
3) [x] dbgprint needs hooking in the printinfo etc
4) [x] VEH amsi bypass
5) [ ] Docs
      1)  [ ] `spawn`
      2)  [ ] Updated amsi bypass technique
6) [x] Comptime BOF C/C++
   1) [x] Could potensh include object files
7) [ ] `inject` + malleable options (malleable options for it to inject on spawn from the default loader)

### v0.7.3

1) [ ] `can_hijack`
   1) [ ] Specify a path to the image, and Wyrm tells you if you can SOH - this would be great for process injection without risking process injection triggers that an EDR could pick up
   2) [ ] Docs
2) [ ] `ps` needs testing in an AD lab; as well as anything else which may rely on kerb / AD config (e.g. the hostname/domain or smth?)
3) [ ] Position independent shellcode stub for the DLL which allows it to be injected into a foreign process
    1)  [ ] This can be a 4th 'deliverable' maybe called (profile)_shellcode.bin which the user uploads
    2)  [ ] I'm not sure this is necessary given the export..?
4) [ ] `kill_thread` command (useful for sideloaded DLLs)
5) [ ] Validate `dotex` actually unloads the memory
6) [ ] Check for xor key in use? Turn that off, not needed with network encryption?
7) [ ] Comptime BOF Rust
8) [ ] Runtime BOF 
9) [ ] Static link bof at comptime
10) [ ] The loader should inherit option for ETW bypass

### v1.0 - Whelpfire

1) [ ] `jump psexec`
2) [ ] Final OPSEC review on binary indicators to make sure nothing is introduced in this version.
3) [ ] Max upload size set on C2 from profile
4) [ ] Logrotate setup &/ cargo clean?
5) [ ] Link additional modules at comptime into the C2 or agent (via profiles), e.g. to enable NGPB or other custom toolkits.
6) [ ] Separate URIs for POST and GET
7) [ ] Multiple URLs / IPs for C2
8) [ ] Round robin and different styles for URI & URL rotation
9)  [ ] Can I tidy wyrm.rs, maybe dynamic dispatch and traits for main dispatch fn?
10) [ ] Loaders should stomp the MZ and "this program.."
11) [ ] Support domain fronting through HTTP headers in malleable profile (check in comms code `.with_header("Host", host)`)
12) [ ] Staging the encrypted payload as opposed to a stageless only build
13) [ ] When sideloaded no console output coming through
14) [ ] EDR shim removal? https://malwaretech.com/2024/02/bypassing-edrs-with-edr-preload.html
15) [ ] Can I make it x86?
16) [ ] Consider a javascript scripting kit (look at nuclei) (suggestion by @sindhwadrikunj)
17) [ ] Other spawn / inject options
18) [ ] WOF API's
    1)  [ ] C2 download file
    2)  [ ] C2 print info / print fail

### v1.1

These are to be split out further as required for more manageable releases.

1) [ ] Long running tasks which have a specified integrity level, so any task set under this scheme can execute at a given integrity level for that machine
2) [ ] Killing the agent should support from thread as well as from process (in the case of an injected process).
3) [ ] Agent & C2 supports multiple endpoints (selectable in build process from cli) / c2 profiles
   1) This needs to be implemented in the wizard also
4)  [ ] `zip` command to natively zip a folder
5)  [ ] Improve pillage function
6)  [ ] Concurrent removable media scanner - runs when main thread is sleeping between calls and looks for a removable disk being added. Auto-pillage.
   1)  [ ] The auto pillage file extensions should be specified in the profile toml
7)  [ ] Auto Escalator (this could be done a separate project that can be used by others, but also compiles into this):
    1)  [ ] User -> SYSTEM (service paths etc)
    2)  [ ] Local user -> Local Admin
    3)  [ ] Local Admin -> SYSTEM
8)  [ ] Improved anti-sandbox checks
9)  [ ] Additional lateral movement options
10) [ ] C2 junk padding response size (needs to play nice with NGPB)
11) [ ] Export agent db info for reporting
12) [ ] Read users clipboard continuously and upload to C2
13) [ ] Multiple C2 implementations on the agent. This could be a task which orders the creation on the implant itself.
14) [ ] Capture screenshots
15) [ ] Autoloot:
    1)  [ ] SSH keys
    2)  [ ] Filenames of office docs, .pdf, .jpg, .mov, .kdbx
16) [ ] Builds agent that can use APIs via hells/halos gate, etc.
    1)  [ ] Look at FreshyCalls as an alternate
17) [ ] Pool Party
18) [ ] C2 rotation strategy from profile
19) [ ] `cat`
20) [ ] `tasks` and `task_kill`
21) [ ] SOCKS proxy
22) [ ] Shellcode loader
23) [ ] C2 configurable so it is hosted on TOR, with C2 fronted redirectors into the TOR network
24) [ ] `drives` search for additional drive volumes
25) [ ] Scope / date / time checks
26) [ ] Add a note to an implant
27) [ ] Some UAC bypasses?
28) [ ] Specify specific proxy for agent to use

### Voidheart - v2.0

These are to be split out further as required for more manageable releases.

1) [ ] Run tools in memory and send output back to operator
2) [ ] C2 over DNS / DOH
3) [ ] SMB agents
4) [ ] Allow multiplayer
5) [ ] Time-stomping for builds & also agent can stomp files on target
6) [ ] Any inspiration from [trustedsec's BOFs](https://github.com/trustedsec/CS-Situational-Awareness-BOF) around some sitrep stuff this can do?
   1)  [ ] `ldapsearch`
7) [ ] 'Overwatch' system on the C2
8) [ ] TOPT
9)  [ ] Add ability to protect staged downloads with a header `key=value`, to try prevent mass downloading of an agent in cases where the operator wants it behind a check
10) [ ] Post Quantum Encryption for below TLS implant comms
11) [ ] Create multiple users 
    1)  [ ] Make implant multiplayer - this may need a bit of rearchitecting

### Ashen Crown - v3.0

1) [ ] Wyrm Rootkit release
2) [ ] Wyrm rootkit loader

### Ghostscale - v4.0

Nothing planned yet.