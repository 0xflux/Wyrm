# Client 

This is the web application interface for operating the Wym C2.

The runtime must be able to find the `.env` file, so this should be in the working directory of where the web app is 
run from, and it must contain a matching `ADMIN_TOKEN` key for the C2, in order to authorise all commands.

If you are logging into the C2 for the first time, note that whatever creds you login with will become the operator's
credentials.

Future updates will support multi-player / multi-accounts.