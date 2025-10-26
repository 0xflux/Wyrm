# C2

Before using the C2, you **SHOULD** change the default admin token and database creds found in the `../.env` for security purposes.

## TLDR

- As above, edit the `../.env` file to use your own creds - this is for security purposes.
- To run the C2, from the root directory (`../`) run `docker compose up -d --build c2`. On first run this may take a few minutes.
- To connect to the C2, you should use the client which can be run via: `docker compose up -d --build client` and is served on port 4040 by default.
- The C2 uses a docker volume `/data` to store loot as well as other persistent files.

## Info

The C2 module handles only the command and control server implementation and does not deal with showing a GUI as output.
That is handled by the `client` crate which you can run via docker.

The C2 has logging for API endpoint access attempts, errors, and login's. **Note** there is no in-built log rotation, so you may wish to use
the linux `logrotate` application to manage these.

- `Logins`
  - This log file is managed in such a way repeat successful logins will not be recorded by an IP, only the first successful login
  - This will log all cases where an IP makes repeated failed logins
  - This log can be disabled via the `.env` file, adding: `DISABLE_ACCESS_LOG=1`.
  - The log file will show (by default) the plaintext password of **unsuccessful logins** for intelligence gain, this is entirely dependant upon your threat model. To turn this feature off, add `DISABLE_PLAINTXT_PW_BAD_LOGIN=1` to your `.env`.
- `Access`
  - This log could get unwieldy and it can be disabled through the C2 `.env` file, by adding `DISABLE_ACCESS_LOG=1`. This will record all visits to endpoint URI's and record if the access was legitimate (from an agent) or not (scanners, researchers, etc). It is enabled by default and you should consider manually pruning the log, or automating with `logrotate`
- `Error`
  - A simple log file which shows C2 error messages to assist in bug reporting / debugging
  - This log file cannot be disabled
