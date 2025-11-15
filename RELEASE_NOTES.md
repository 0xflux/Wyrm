# Release Notes

Anything found labelled with a '&#128679;' indicates a possible breaking change to a profile which you will need to adjust from
the `default.example.profile` found in `/c2/profiles/`. This is done especially as to not overwrite your custom profiles when
pulling updates.

**IN ANY CASE ALWAYS BACKUP YOUR PROFILES BEFORE UPDATING!!!!**

## &#128679; v 0.4.1

### &#128679; Breaking changes

- The C2 now uses nginx as part of the docker stack to serve the C2 over TLS. This was an important design decision whilst re-working the server; we are moving away from the previous method of authentication (which re-authenticates each time and will be more CPU intensive than required). Now, we use HTTP secure cookies to enable the login sessions. Because of this change, you now need to generate a certificate and its private key, and they need to be placed into `/nginx/certs/` named `cert.pem` and `key.pem` respectively. For localhost testing, you can use `certbot` / `mkcert localhost 127.0.0.1 ...` etc. For prod, create a cert as you see fit (`certbot` / purchased certificates / from a CA, etc..).
- As Wyrm now uses nginx via Docker, you need to configure the configuration file in `/nginx/nginx.conf`. This file is provided for you in git tracking. **Note:** when v0.4.1 is pushed, I will not be tracking changes to this file so that it doesn't accidentally break a build.
  - Edit `server_name` as appropriate for both HTTP and HTTPS.
  - Edit other settings as you see fit; note, the CORS stuff is mandatory as the GUI is separate from the server.
- You now log into the C2 entering the address of: https://localhost into the login panel (at http://localhost:3000)

### Non-breaking changes

- We now use a better, more efficient, and more secure authentication method of using actual auth HTTPS only tokens with a lifetime of **12 hrs** before you need to log in again to get a new token.

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