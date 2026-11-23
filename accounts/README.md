# Shifu account pilot

Open **Shifu → My Macs…** in a pilot build. Name this Mac and sign in with Google in the system browser. The first time a Google account joins, Shifu asks for a single-use invite. That account can sign in on another Mac without another invite. Each person sees their own Mac list. Removing a Mac revokes its account session. Terminal access uses Shifu’s existing [remote pairing](../docs/REMOTE-RELAY.en.md#relay-and-pairing).

## Try the local pilot

From the repository root, run `npm --prefix app ci`, `npm --prefix accounts ci`, then `npm --prefix accounts run preview`. Open the loopback URL it prints. Choose a sample identity and Mac, open **My Macs**, and use one of the printed invites. Choose the same identity on the second Mac to try a returning sign-in. This rehearsal uses the desktop account panel and account API with sample identities and a separate in-memory database. Stopping it clears the sample accounts.

## Google setup

1. Create or select a Shifu project owned by your team in Google Cloud. In **Google Auth Platform**, configure Branding and Audience. Choose **External** for downloaders outside your organization and follow the console’s publishing instructions for your intended audience.
2. In **Clients**, create an OAuth client of type **Desktop app**. Configure its client ID and secret in the account server environment. The system browser returns to a loopback port on the Mac. The service requests `openid email`, exchanges the authorization code with PKCE, and verifies the signed ID token and its nonce.
3. Choose an HTTPS hostname your team controls for the account API. A single host with persistent disk can serve this pilot. Hosting the API and creating Google credentials are independent.

[Google project setup](https://developers.google.com/workspace/guides/create-project) · [Desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app)

## Run the account service

Use Node 22.13 or later. From this directory, install the locked dependencies and run the checks.

```sh
npm ci --ignore-scripts
npm test
```

Supply `SHIFU_GOOGLE_CLIENT_ID` and `SHIFU_GOOGLE_CLIENT_SECRET` through the service environment. Choose a durable path for `SHIFU_ACCOUNTS_DB` (default `data/accounts.sqlite` under the current working directory). Start with `npm start`. The service listens on loopback port 3845 by default. Set `PORT` to change it. A request to `GET /v1/me` without a Shifu session should return HTTP 401.

### Host it on Google Compute Engine

A small Linux VM with a persistent disk can run Node, SQLite and an HTTPS reverse proxy. Select the owning project and its billing account before creating the VM. Point a hostname at its reserved external IP. Allow the proxy to receive HTTPS and its certificate validation traffic. The Node port stays on loopback. [Compute Engine disk options](https://cloud.google.com/compute/docs/disks/persistent-disks)

Create the system user and group with `sudo useradd --system --user-group --home-dir /var/lib/shifu-accounts --shell /usr/sbin/nologin shifu-accounts`. Place this directory under `/opt/shifu-accounts`, run `npm ci --omit=dev --ignore-scripts` there, and set the Google variables in a root-readable `/etc/shifu-accounts.env` with mode 0600. The supplied [systemd unit](deploy/shifu-accounts.service) expects Node 22.13+ at `/usr/bin/node` and keeps SQLite under `/var/lib/shifu-accounts`. Install it under `/etc/systemd/system/`, run `sudo systemctl daemon-reload` and `sudo systemctl enable --now shifu-accounts`. Proxy the chosen account hostname to `127.0.0.1:3845`. For example, in the existing Caddy configuration:

```caddyfile
accounts.example.com {
    reverse_proxy 127.0.0.1:3845
}
```

Back up the database using SQLite’s online backup or a consistent disk snapshot. Redeemed invitations, memberships and revocations live in that database.

## Invite the first users

Run the invite command **against the same database as the running service**. For a local instance started from this directory with the default database:

```sh
npm run --silent invite
```

For the systemd installation, use the service identity and its database:

```sh
sudo -u shifu-accounts env SHIFU_ACCOUNTS_DB=/var/lib/shifu-accounts/accounts.sqlite /usr/bin/node /opt/shifu-accounts/invite.mjs
```

Each invocation prints a fresh code for one Google account, valid for seven days. Give it privately to one tester. Issuing and redeeming codes works across service restarts. Only a digest of the code is stored. A missing, invalid, expired or previously redeemed code leaves a new user outside the pilot.

## Connect the desktop app

Set the public account origin when building the pilot app, replacing the example hostname with your own:

```sh
cd ../app
npm ci
SHIFU_ACCOUNTS_URL=https://accounts.example.com npm run build:desktop
```

For a local development run, start the service with your Desktop OAuth credentials, then run `SHIFU_ACCOUNTS_URL=http://127.0.0.1:3845 npm run tauri dev` from `app/`. With the variable unset at both build and launch, ordinary builds open the local workspace directly and omit the pilot menu.

The Mac keeps its installation ID and Shifu session in Keychain, scoped to the app and account-service origin. The service keeps the Google subject ID, verified email, and the display name and random ID of each registered Mac. The desktop app uploads these only during account use. Projects, transcripts, agent credentials and pairing keys stay on the Mac. A Shifu session expires after 30 days. A returning member can then sign in with Google again.
