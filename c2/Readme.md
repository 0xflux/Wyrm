# C2

Before using the C2, ensure the `.env` file is present, and the **working directory** of the C2 is set to the `/c2` directory.
The shell install script will set this automatically for you when creating the service, but if running from Windows, or manual
deployment, the C2's working directory MUST be the `/c2` folder, as it uses relative pathing.

The C2 module handles only the command and control server implementation and does not deal with showing a GUI as output.
That is handled by the `client` crate.

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

## Some handy notes to self

- Ensure you have sqlx: `cargo install sqlx-cli`

## Postgres stuff

Cos im not a frequent web app / db dev, some helpful reminders for using psql:

- Log in: `sudo -u postgres psql wyrm`
- Display tables: `\dt`
- Show data: `SELECT * FROM agents;`
- Show table schema: `\d+ tasks`