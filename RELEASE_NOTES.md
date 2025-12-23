# Release Notes

Anything found labelled with a '&#128679;' indicates a possible breaking change to a profile which you will need to adjust from
the `default.example.profile` found in `/c2/profiles/`. This is done especially as to not overwrite your custom profiles when
pulling updates.

**IN ANY CASE ALWAYS BACKUP YOUR PROFILES BEFORE UPDATING!!!!**

## v0.7.1

- Bug fix for the reflective DLL - it was not fully reflective in v0.7, I left some of the logic in the injector which has been migrated to the rDLL bootstrap mechanism. The rDLL should now be reflective from external tooling (so long as you start execution at the `Load` export).
- Introduces an **early preview** of the `spawn` command - you can spawn a new Wyrm agent impersonating `svchost`. To use this you must have either the loader or the raw payload (DLL version) on disk (on the target) and you can run it via: `spawn "path/to/dll"`. Bundling this in as there was the above critical update to the rDLL. It is **NOT** recommended you use this as I am still building it, if you want to, feel free - but it may break or trigger AV right now.

## v0.7

- Wyrm now builds as a reflective DLL, supplying you with a loader DLL, exe and svc in place of the previous raw binary. Meaning in your build, for each profile you now get
  - Raw binaries for when you wish to use them with your own loaders / toolsets (exe, svc and dll) where the DLL version is set up for **reflective** loading via the `Load` export. See the [docs](https://docs.wyrm-c2.com/implant/rdll.html) for more info on how to use the reflective loader export.
  - A loader using the reflective injector of the DLL, giving you an exe, svc and dll - all which load the rDLL into the **current** process. Support for process injection coming later. This is XOR 'encrypted' in the .text section of the loader.
- `pull` command now does so buffered in memory, preventing resource exhaustion from the implant.
- Native support for running `whoami` without needing to touch powershell. Run `whoami` to get info on the domain, user, SID and what privileges are assigned.
- Implant is now **proxy aware**! This means it will attempt to use a corporate proxy if set up for making connections. If none exists, then none will be used! This is done per request to ensure the correct proxy settings are applied to the correct C2 address if using multiple.
- Binary size of the postex payload almost HALVED! Down to about 800 kb!
- Fix logging on C2 to log correct IP with NGINX X-Forwarded-For header.
- Moved implant to reqwest crate for networking from minreq, no real impact on agent size and provides more functionality.
- Fix bug where implant tried to register a mutex when not specified.
- Fix bug in file upload via GUI to the C2 in that it happens much faster.
- Improve how the C2 handles panics and unwraps using `catch_panic`, the  C2 should no longer become unresponsive during panics. Using panics and unwraps was by design, so this should add stability.
- Improved stability with the automatic DLL proxying for search order hijacking.

### Known Issues

- When the DLL is loaded via sideloading, no debug prints or console prints from dotnet tooling are captured.

## v0.6

- AMSI patching available in the implant via the malleable profile (only runs in the agent when necessary).
- You can now execute dotnet programs remotely in the agent, all in memory - does not write anything to disk! Simply run `dotex` and pass your args after, e.g. `dotex Rubeus.exe klist` (see below point as to how to get the binary sent to the agent)!
- This update introduces the `c2_transfer` dir in the root which is used for staging files to be internally used by the C2 during operations such as `dotex` where the payload is sent as bytes to the agent through C2. This folder is a bind mount meaning you can drop files in ad-hoc whilst the server is running and it should be able to read them. If you drop tools in here in a folder, make sure you include that in the path to the tool.
- Agent prints get sent to the server - meaning if you build in debug mode you can see the debug output in the terminal on the c2. This is mainly due to now removing the console window from the application.
- The CRT (C Runtime) is now statically linked into the binary so it can run on machines without the MSVCRT DLLs.
- Some nice UI changes
- Bug fix with parsing config on C2, some options were being left out under certain conditions.

## v 0.5.3

- Potential bug fix for the UI very occasionally not showing messages in the UI. Seems to be fixed.. but the bug happens so little it can be hard to diagnose.

## v 0.5.2

- DLL internals now allow for a better loading mechanism which ensures if run via rundll32, and from DLL Search Order Hijacking, without early termination.
- Malleable profile now provides support for fully fledged DLL Search Order Hijacking attacks! See docs for more info.
- Malleable profile now includes the ability to create a global mutex so you can ensure only one implant (profile) can run on the system, this could be useful for DLL sideloading / search order hijacking if the target is extremely noisy in terms of lots of subprocesses loading in the binary. You can of course have this applied to one profile, but not another, as it is fully optional.
- Improves the output of the `ps` and `reg query` commands.
- Added additional deserialisation option for output of `reg query` such that the `REG_BINARY` type gets decoded.

### Issues under investigation

There is still a very rare, small case where the first few instructions get dispatched and sent to the client, but don't appear in the console. They are logged in the browser store temporarily, but I think the bug is still here.. under investigation - extremely rare which is making it difficult to determine if it is still an issue.

## v 0.5.1

- Improved GUI updates! The dashboard message panel now looks much better, with newlines appearing properly, and spacing kept from the raw output. Colours have also been improved making it much easier to distinguish between message sections!
- Improved UI printing of the `ls` command.

## &#128679; v 0.5

### &#128679; Breaking changes

- Introduced the .svc binary which builds as part of your build package from the C2. There is a new required field in the profile, which is **svc_name**. Read more in the Wyrm Docs profile section as to how to use this field. In short, the value of this field (required) is passed to the Windows Service Control Manager when the service binary is run.

### Non-breaking changes

- Introduced the **string scrubber**!
  - The string scrubber automatically scrubs 'implant.dll' from the export name of the Wyrm DLL.
  - The string scrubber allows through a malleable profile the ability to scrub certain strings from the binary. **Warning:** this interprets bytes like for like and either allows you to replace them, or zero them out. This could lead to accidental pattern collisions with machine code / other artifacts, so if you are using this feature, be sure to test the binary before deployment on a red team op!
- Added download counter for staged resources (visible in new log file, and on the staged resources GUI page).
- Fixed bug (again..) that was preventing messages showing in the GUI, even though they were processed by the client. Hopefully that is the end of that bug!

## v 0.4.4

- Introduces the profile options to build custom DLL export names, as well as define custom machine code to run at an export. This could be used for DLL Sideloading (better support for that coming later, but it should still work in some cases), OPSEC, or just causing a bit of mayhem for a blue teamer.

## &#128679; v 0.4.3

- Investigated whether error logging was happening (the C2 hasn't generated an error in a long time) - confirmed error handling works as expected. This is good.
- Fixes bug which caused some results not to print to an agents console.
- Fixes bugs with file drop via the implant; now correctly drops a file in the 'in memory' working directory of the beacon.

### &#128679; Breaking changes

- Removed most of the environment variable requirements (see docs for instructions).
- This update brings a change to profiles! You now have one profile, and only one, which exists in the `c2/profiles/*.toml` file. You now specify multiple implants by key to build, or alternatively you can build all implant profiles by typing 'all' on the profile builder. See the [docs](https://docs.wyrm-c2.com/) for how to set the profile up, example is provided.

## v 0.4.2

- Fixes bug which prevented user logging into C2 for the first time if no user is created.

## &#128679; v 0.4.1

### &#128679; Breaking changes

- The C2 now uses nginx as part of the docker stack to serve the C2 over TLS. This was an important design decision whilst re-working the server; we are moving away from the previous method of authentication (which re-authenticates each time and will be more CPU intensive than required). Now, we use HTTPS secure cookies to enable the login sessions. Because of this change, you now need to generate a certificate and its private key, and they need to be placed into `/nginx/certs/` named `cert.pem` and `key.pem` respectively. For localhost testing, see my guide on [creating trusted certificates](https://fluxsec.red/wyrm-c2-localhost-self-signed-certificate-windows) locally - failing to do this will result in no connectivity on **localhost**. For prod, create a cert as you see fit (`certbot` / purchased certificates / from a CA, etc..) and add them to the `nginx/certs` dir, updating the `/nginx/nginx.conf` as necessary.
- As Wyrm now uses nginx via Docker, you need to configure the configuration file in `/nginx/nginx.conf`. This file is provided for you in git tracking. **Note:** when v0.4.1 is pushed, I will not be tracking changes to this file so that it doesn't accidentally break a build.
  - Edit `server_name` as appropriate for both HTTP and HTTPS.
  - Edit other settings as you see fit; note, the CORS stuff is mandatory as the GUI is separate from the server.
- You now log into the C2 entering the address of: https://localhost into the login panel (at http://localhost:3000)

### Non-breaking changes

- We now use a better, more efficient, and more secure authentication method of using actual auth HTTPS only tokens with a lifetime of **12 hrs** before you need to log in again to get a new token.
- Fix bug which caused tasks on implant to be dispatched out of order.
- Fixed bug causing console output to appear in the wrong order on the GUI.
- C2 now has docs! https://docs.wyrm-c2.com/

## &#128679; v 0.4

### &#128679; Breaking changes

- `.env` migrated from `/c2` to `/` - **THIS MAY AFFECT YOUR ADMIN TOKEN AND OTHER ENVIRONMENT SETTINGS, PLEASE BACK-UP BEFORE UPDATING**.
- Docker build pipeline for client now moved to workspace root rather than from within the `/client` directory. To build the client, now run (from the workspace root): `docker compose up -d --build client`.
- No more `install.sh`! You run the C2 via docker, simply with: `docker compose up -d --build  c2` from the root directory. This means you can run both the client and c2 via docker.
  - Client: `docker compose up -d --build client`.
  - C2: `docker compose up -d --build c2`.
- Loot, staged resources, and logs can be found in the docker volume /data.

### Non breaking changes

- OPSEC improvement with removing static artifacts from the binary.
- Introduces timestomping for the compile date on built implants - see `profile.example.toml` for full docs, but this optional profile option allows you to select a date in **British format** which is stamped into the binary as the compile date, aiding advanced OPSEC.
- Introduces the ability to export the completed tasks of the agent to a json file (for ingesting into ELK etc) by running the `export_db` command on an agent.
- Completed tasks now mapped to MITRE ATT&CK!
- Introduces the registry manipulation features with `reg query`, `reg delete` and `reg add` commands.
- Improve docker build process for the client through [cargo chef](https://lpalmieri.com/posts/fast-rust-docker-builds/).
- Implant supports `rm` to remove a file, and `rm_d` to remove a directory (and all its children).
- Adds user name who is running processes, as well as the ability to show processes running at a higher privilege (if running with high integrity).
- Improved how the system records time an action was completed, now properly represents the time the agent actually did the work, vs what was in place which was when the result was posted to the server and processed by the database.
- Improved HTTP packet ordering to be more concise and clear, using repr(C) to ensure consistent ordering under the new packet layout.

## v 0.3

This release introduces the new GUI, which is a web based UI used to interact with the Wyrm C2.

- New web based GUI!
- Docker is used to build and deploy the GUI, making it really straightforward.
- Building payloads now downloads as a 7zip archive through the browser.
  - Install `sh` script updated to include 7z dependencies, if manually updating through a pull; make sure you have 7zip installed and available on PATH.

## v 0.2

- Wyrm C2 now uses profiles to build agents with fully customisable configurations.
- IOCs are encrypted at compile time in the payload.
- Events Tracing for Windows (ETW) patching support via customisable profile.
- Profile options to determine log fidelity of the C2.
- Jitter supported in profile, as a percentage of the maximum sleep value time in seconds.
- Investigated apparent bug where results of running tasks appear out of order. The agent does not execute them out of order, this is a GUI display bug. Not fixing at this moment in time as I am building a new GUI for this in an upcoming pre-release version.