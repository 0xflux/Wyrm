# Release Notes

Anything found labelled with a '&#128679;' indicates a possible breaking change to a profile which you will need to adjust from
the `default.example.profile` found in `/c2/profiles/`. This is done especially as to not overwrite your custom profiles when
pulling updates.

## &#128679; v 0.4

### &#128679; Breaking changes

- Docker build pipeline for client now moved to workspace root rather than from within the `/client` directory. To build the client, now run (from the workspace root): `docker compose up -d --build client`

### Non breaking changes

- OPSEC improvement with removing an artifact from the binary related to a struct name
- Improve docker build process for the client through [cargo chef](https://lpalmieri.com/posts/fast-rust-docker-builds/).

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