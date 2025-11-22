# Project Milestones

Any item with a (L) tag is a contribution which will not be live (or requires further decision making) as this is intended to be
developed as a premium or otherwise private feature. These will be few and far between.

### 0.4.3

1) [x] Is the error.log working and the panic hook?
2) [ ] Remove the `.env`, and put instructions in the docs, makes it easier over time.
   1) [x] Actually I think I want to migrate to 1 profile which effectively becomes the .env? Except for db creds, those should be done over an env file. 
      1) [x] On boot check only 1 profile.
      2) [x] The user selects which **implant name** to build in the builder.
      3) [x] `All/all` should be a special name to build all profiles present.

### 0.5

1) [ ] DLL needs thread start in `DLLMain`
2) [ ] Create custom DLL exports when staging a DLL, which would make it useful for DLL SOH/side loading
   1) [ ] Maybe this should be configurable by the TOML, so each payload can have its own name can can be built individually (or all payloads can be built at once)
   2) [ ] Can use a macro to write the function exporting the function name
      1) [ ] Option per function for it to run Wyrm, or alternatively it is a junk function (not sure how to macroise that for random things? Maybe thats a bad idea)
3) [ ] Build implant as svc
4) [ ] Multiple profiles for listener and implant
5) [ ] Admin URI can be configured specifically in the profile - this will help defeat a little CTI
6) [ ] Number of downloads for staged resources
7) [ ] String stomping options on built binary 
8) [ ] Final OPSEC review on binary indicators to make sure nothing is introduced in this version.

### 0.5.1

1) [ ] Periodically delete items from browser store not in connected agents if not open in a tab.
2) [ ] Save chat from browser store to disk on ka & tab close? Or command to restore history to console.
3) [ ] Create multiple users

### 0.6

1) [ ] Max upload size set on C2
2) [ ] `pull_stream` - Pulls a file as a stream (where the file to exfil is larger than the available RAM)
3) [ ] Link additional modules at comptime into the C2 or agent (via profiles), e.g. to enable NGPB or other custom toolkits.
4) [ ] Consider deprecating the sleep in `listener` and moving it to the `implant` section instead
   1) [ ] Or, keep `listener` but the implant options you can have an array of listener key names to include in the build
5) [ ] Multiple URLs / IPs for C2
6) [ ] Separate URIs for POST and GET
7) [ ] Round robin and different styles for URI & URL rotation
8) [ ] Final OPSEC review on binary indicators to make sure nothing is introduced in this version.
9) [ ] Ps should show parent pids with children in a hierarchy
10) [ ] Logrotate setup
 
### v1.0 - Whelpfire

1) [ ] NG Proxy Bypass (NGPB) (L)
2) [ ] Internal proxy resolution for HTTP requests
3) [ ] Auto poll notifications across all active tabs
4) [ ] `execute-assembly`
   1) [ ] AMSI patching option in profile 
5) [ ] `jump psexec`
6) [ ] Build all Windows payloads (exe, dll, svc), staging should happen via file upload - not by creating an individual payload (at least for now)
7) [ ] Create a "weaponisation" section; which can house various tools to automate weaponisation of certain features. 
   1) [ ] First feature to create here is a stage zero shortcut creator for downloading, moving and executing a payload
8) [ ] Website docs, maybe gitbook or smth, im liking the look of https://github.com/redimp/otterwiki
9) [ ] Stop bcrypt'ing on each admin control / auth event, use tokens. It is needlessly inefficient currently.
10) [ ] Support domain fronting through HTTP headers in malleable profile (check in comms code `.with_header("Host", host)`)
11) [ ] Final OPSEC review on binary indicators to make sure nothing is introduced in this version.

### v1.0.1

1) [ ] Consider unit tests.

### v1.1

These are to be split out further as required for more manageable releases.

1) [ ] Stager & reflective DLL injector (L).
   1) [ ] The base payload should move from an exe/dll to a RDLL or similar so the entry becomes a bootstrapper
   2) [ ] Don't forget ETW & anti_sandbox strategies here - they are in the 2nd stage but may need moving to the 1st stage (aka the bootstrapper)
2) [ ] Long running tasks which have a specified integrity level, so any task set under this scheme can execute at a given integrity level for that machine
3) [ ] Selectable listener profiles, which will feed into things such as spawn, or other things where a selectable listener is required
4) [ ] `spawn`
5) [ ] `inject`
6) [ ] Killing the agent should support from thread as well as from process (in the case of an injected process).
7) [ ] Clone a webpage and automatic download of implant at a staged location (separate wizard needed).
8) [ ] Agent & C2 supports multiple endpoints (selectable in build process from cli) / c2 profiles
   1) This needs to be implemented in the wizard also
9)  [ ] `zip` command to natively zip a folder
10) [ ] Improve pillage function
11) [ ] Concurrent removable media scanner - runs when main thread is sleeping between calls and looks for a removable disk being added. Auto-pillage.
   1)  [ ] The auto pillage file extensions should be specified in the profile toml
12) [ ] Auto Escalator (this could be done a separate project that can be used by others, but also compiles into this):
   1) [ ] Local user -> Local Admin
   2) [ ] Local Admin -> SYSTEM
13) [ ] Improved anti-sandbox checks
14) [ ] Lateral movement:
   1) [ ] PsExec
15) [ ] C2 junk padding response size (needs to play nice with NGPB)
16) [ ] Export agent db info for reporting
17) [ ] Read users clipboard continuously and upload to C2
18) [ ] Multiple C2 implementations on the agent. This could be a task which orders the creation on the implant itself.
19) [ ] More lateral movement techniques
20) [ ] Capture screenshots
21) [ ] Autoloot:
    1)  [ ] SSH keys
    2)  [ ] Image hashes (L)
    3)  [ ] Filenames of office docs, .pdf, .jpg, .mov, .kdbx
22) [ ] Builds agent that can use APIs via hells/halos gate, etc.
23) [ ] Pool Party
24) [ ] C2 rotation strategy from profile
25) [ ] `cat`
26) [ ] `tasks` and `task_kill`
27) [ ] SOCKS proxy
28) [ ] Shellcode loader
29) [ ] C2 configurable so it is hosted on TOR, with C2 fronted redirectors into the TOR network
30) [ ] Profile option for mutex
31) [ ] `drives` search for additional drive volumes
32) [ ] Scope / date / time checks
33) [ ] Add a note to an implant

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
8) [ ] Runtime obfuscation and sleep masking
9)  [ ] 'Overwatch' system on the C2
10) [ ] TOPT
11) [ ] Add ability to protect staged downloads with a header `key=value`, to try prevent mass downloading of an agent in cases where the operator wants it behind a check
12) [ ] Post Quantum Encryption for below TLS implant comms

### Ashen Crown - v3.0

1) [ ] Wyrm Rootkit release
2) [ ] Wyrm rootkit loader

### Ghostscale - v4.0

Nothing planned yet.