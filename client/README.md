# Wyrm Client

This is the web application interface for operating the Wym C2; the C2 component can be found [here on GitHub](https://github.com/0xflux/Wyrm/).

This GUI is super simple to use via `docker`. The only change you need to make before running is in the `docker-compose.yml`,
editing the token value in `ADMIN_TOKEN=fdgiyh%^l!udjfh78364LU7&%df!!` to be whatever you set on the C2. Note: for security
purposes you should not use the default token I provided here.

If you are using this with the Wyrm C2 running locally; rather than connecting via the interface to the typical loopback address 
(127.0.0.1), you will have to enter the IP address of your host, such as 192.168.0.123 (a constraint of docker).

# Usage

To run - simply install docker and: `docker compose up -d`

If any local edits are required during development / debugging, you can rebuild only the changes with: `docker compose up -d --build`

**IMPORTANT**: If you are logging into the C2 for the first time, note that whatever creds you login with will become the operator's
credentials.

You can then access the GUI via: http://127.0.0.1:4040.

## Environment variables

The following environment variables can be controlled through the `docker-compose.yml`:

- `ADMIN_TOKEN`: As discussed above

## Compatibility

To ensure compatibility between versions always ensure both the GUI and C2 are up to date.

This build is compatible with v0.3 of Wyrm C2.

# Legal notice

By using Wyrm C2 you understand that you do so at your own risk. You may only use this for authorised, legal, red team or penetration testing
activity where you have the **full** consent of the asset owner for whom you target with this C2.

I cannot be held responsible for any misuse of this, it is a legitimate security testing tool.