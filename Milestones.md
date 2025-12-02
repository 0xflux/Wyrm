# Project Milestones

Any item with a (L) tag is a contribution which will not be live (or requires further decision making) as this is intended to be
developed as a premium or otherwise private feature. These will be few and far between.

### 0.5.2

1) [x] Experiment with proxified DLL SOH
   1) [x] DllMain loader
   2) [x] Write it up on the docs
   3) [x] Add a global mutex
2) [ ] Ps should show parent pids with children in a hierarchy

### 0.6

1) [ ] `pull_stream` - Pulls a file as a stream (where the file to exfil is larger than the available RAM)
2) [ ] Link additional modules at comptime into the C2 or agent (via profiles), e.g. to enable NGPB or other custom toolkits.
3) [ ] Multiple URLs / IPs for C2
4) [ ] Separate URIs for POST and GET
5) [ ] Round robin and different styles for URI & URL rotation
6) [ ] Logrotate setup
7) [ ] Native `whoami` command should output more than just the username, should include GUID and privs natively.
8) [ ] Max upload size set on C2 from profile
9) [ ] Implant should dispatch task BEFORE checking back in / sleep, rather than on next wake up?
 
### v1.0 - Whelpfire

1) [ ] Move to new basic model with stagers, the exports, etc can also apply to those, but we would produce:
   1) [ ] DLL, Exe, Svc of the 'virgin' payload
   2) [ ] NoStd loader (stageless) - encrypted 2nd stage shipped in binary
   3) [ ] NoStd loader (staged) - encrypted 2nd stage
   4) [ ] The NoStds should produce:
      1) [ ] DLL
      2) [ ] Exe
      3) [ ] Svc
   5) [ ] No fancy techniques under the hood, that will come with a (L) version
      1) [ ] Need to think about what the L version will include and look like
      2) [ ] Maybe early bird and other similar techniques, syscalls, etc?
   6) [ ] Malleable encryption byte in profile
2) [ ] NG Proxy Bypass (NGPB) (L)
3) [ ] Internal proxy resolution for HTTP requests
4) [ ] `execute-assembly`
   1) [ ] AMSI patching option in profile 
5) [ ] `jump psexec`
6) [ ] Support domain fronting through HTTP headers in malleable profile (check in comms code `.with_header("Host", host)`)
7) [ ] Profile option for mutex
8) [ ] Final OPSEC review on binary indicators to make sure nothing is introduced in this version.
9) [ ] `ps` needs testing in an AD lab; as well as anything else which may rely on kerb / AD config (e.g. the hostname/domain or smth?)

### v1.1

These are to be split out further as required for more manageable releases.

1) [ ] Stager & reflective DLL injector (L).
   1) [ ] The base payload should move from an exe/dll to a RDLL or similar so the entry becomes a bootstrapper
   2) [ ] Don't forget ETW & anti_sandbox strategies here - they are in the 2nd stage but may need moving to the 1st stage (aka the bootstrapper)
2) [ ] Long running tasks which have a specified integrity level, so any task set under this scheme can execute at a given integrity level for that machine
3) [ ] `spawn` + malleable options
4) [ ] `inject` + malleable options
5) [ ] Killing the agent should support from thread as well as from process (in the case of an injected process).
6) [ ] Agent & C2 supports multiple endpoints (selectable in build process from cli) / c2 profiles
   1) This needs to be implemented in the wizard also
7)  [ ] `zip` command to natively zip a folder
8)  [ ] Improve pillage function
9)  [ ] Concurrent removable media scanner - runs when main thread is sleeping between calls and looks for a removable disk being added. Auto-pillage.
   1)  [ ] The auto pillage file extensions should be specified in the profile toml
10) [ ] Auto Escalator (this could be done a separate project that can be used by others, but also compiles into this):
    1)  [ ] User -> SYSTEM (service paths etc)
    2)  [ ] Local user -> Local Admin
    3)  [ ] Local Admin -> SYSTEM
11) [ ] Improved anti-sandbox checks
12) [ ] Additional lateral movement options
13) [ ] C2 junk padding response size (needs to play nice with NGPB)
14) [ ] Export agent db info for reporting
15) [ ] Read users clipboard continuously and upload to C2
16) [ ] Multiple C2 implementations on the agent. This could be a task which orders the creation on the implant itself.
17) [ ] Capture screenshots
18) [ ] Autoloot:
    1)  [ ] SSH keys
    2)  [ ] Image hashes (L)
    3)  [ ] Filenames of office docs, .pdf, .jpg, .mov, .kdbx
19) [ ] Builds agent that can use APIs via hells/halos gate, etc.
    1)  [ ] Look at FreshyCalls as an alternate
20) [ ] Pool Party
21) [ ] C2 rotation strategy from profile
22) [ ] `cat`
23) [ ] `tasks` and `task_kill`
24) [ ] SOCKS proxy
25) [ ] Shellcode loader
26) [ ] C2 configurable so it is hosted on TOR, with C2 fronted redirectors into the TOR network
27) [ ] `drives` search for additional drive volumes
28) [ ] Scope / date / time checks
29) [ ] Add a note to an implant
30) [ ] Runtime obfuscation, sleep masking and covert loading (L?)
31) [ ] Some UAC bypasses?

### Voidheart - v2.0

These are to be split out further as required for more manageable releases.

1) [ ] Run tools in memory and send output back to operator
2) [ ] C2 over DNS / DOH
3) [ ] SMB agents
4) [ ] Spawn to / Spawn as (including from malleable configuration)
5) [ ] Allow multiplayer
6) [ ] Time-stomping for builds & also agent can stomp files on target
7) [ ] Any inspiration from [trustedsec's BOFs](https://github.com/trustedsec/CS-Situational-Awareness-BOF) around some sitrep stuff this can do?
   1)  [ ] `ldapsearch`
8) [ ] 'Overwatch' system on the C2
9) [ ] TOPT
10) [ ] Add ability to protect staged downloads with a header `key=value`, to try prevent mass downloading of an agent in cases where the operator wants it behind a check
11) [ ] Post Quantum Encryption for below TLS implant comms
12) [ ] Create multiple users 
    1)  [ ] Make implant multiplayer - this may need a bit of rearchitecting
13) [ ] **Entire** website clone, and serve download from named page (L).

### Ashen Crown - v3.0

1) [ ] Wyrm Rootkit release
2) [ ] Wyrm rootkit loader

### Ghostscale - v4.0

Nothing planned yet.